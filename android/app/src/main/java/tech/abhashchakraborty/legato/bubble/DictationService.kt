package tech.abhashchakraborty.legato.bubble

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.PowerManager
import android.os.SystemClock
import android.util.Log
import android.widget.Toast
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import tech.abhashchakraborty.legato.Legato
import tech.abhashchakraborty.legato.MainActivity
import tech.abhashchakraborty.legato.ModelStatus
import tech.abhashchakraborty.legato.R
import tech.abhashchakraborty.legato.Recorder
import tech.abhashchakraborty.legato.legato
import java.util.concurrent.CopyOnWriteArrayList

/** What the bubble shows: idle, listening with a level, or working on the text. */
object Dictation {
    sealed class State {
        data object Idle : State()
        data class Listening(val level: Float) : State()
        data object Transcribing : State()
    }

    /** Where the words go: typed into the field by the bubble, or handed to a Legato screen. */
    enum class Target { Field, Screen }

    val state = kotlinx.coroutines.flow.MutableStateFlow<State>(State.Idle)
    /** Words for the Legato screen that asked for them, waiting until it takes them. */
    val forScreen = kotlinx.coroutines.flow.MutableStateFlow<String?>(null)
    /** Where the dictation under way goes. */
    @Volatile var target = Target.Field
        internal set
    val listeners = CopyOnWriteArrayList<(State) -> Unit>()

    internal fun set(s: State) {
        state.value = s
        listeners.forEach { it(s) }
    }

    fun start(context: Context, target: Target = Target.Field) =
        context.startForegroundService(
            Intent(context, DictationService::class.java)
                .setAction(DictationService.ACTION_START)
                .putExtra(DictationService.EXTRA_TARGET, target.name),
        )
    fun finish(context: Context) = send(context, DictationService.ACTION_FINISH)
    fun cancel(context: Context) = send(context, DictationService.ACTION_CANCEL)

    private fun send(context: Context, action: String) {
        val intent = Intent(context, DictationService::class.java).setAction(action)
        if (action == DictationService.ACTION_START) context.startForegroundService(intent) else context.startService(intent)
    }
}

/**
 * Holds the microphone while you talk, as the foreground service Android
 * asks for, then turns the recording into text and types it into the field
 * through the accessibility service.
 */
class DictationService : Service() {
    private val recorder = Recorder()
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val main = Handler(Looper.getMainLooper())
    private val tick = object : Runnable {
        override fun run() {
            if (recorder.isRecording) {
                // A dictation left listening (the bubble forgotten, the phone
                // put away) is written up rather than recording all evening.
                val now = SystemClock.elapsedRealtime()
                if (recorder.level > SPEECH_LEVEL) heardAt = now
                val why = when {
                    now - heardAt > QUIET_LIMIT_MS -> "Stopped listening after 2 minutes without speech"
                    now - startedAt > LONGEST_MS -> "Stopped listening at 20 minutes"
                    else -> null
                }
                if (why != null) {
                    applicationContext.legato.onLine(why)
                    finish()
                    return
                }
                Dictation.set(Dictation.State.Listening(recorder.level))
                main.postDelayed(this, 60)
            }
        }
    }

    // Main thread only. A recording that's still being written up when the
    // next starts, or when ✕ is tapped, mustn't stop or type over that one.
    private var session = 0
    private var startedAt = 0L
    private var heardAt = 0L
    private var cancelled = 0
    private var pending = 0

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_START -> start(
                runCatching { Dictation.Target.valueOf(intent.getStringExtra(EXTRA_TARGET) ?: "") }.getOrDefault(Dictation.Target.Field),
            )
            ACTION_FINISH -> finish()
            ACTION_CANCEL -> cancel()
        }
        return START_NOT_STICKY
    }

    private fun start(target: Dictation.Target) {
        if (recorder.isRecording) return
        val notification = notification()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            startForeground(NOTIFICATION_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE)
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }
        val legato = applicationContext.legato
        if (!legato.pcWillTranscribe()) {
            when {
                // Loads while you talk (it's let go after a while unused).
                legato.modelFile().exists() -> legato.loadModel()
                // The PC is being reached while you talk; transcribe() waits for it.
                legato.pcMayTranscribe() -> legato.run { connect() }
                else -> {
                    Toast.makeText(this, getString(R.string.no_model), Toast.LENGTH_LONG).show()
                    stopSelf()
                    return
                }
            }
        }
        recorder.start()
        startedAt = SystemClock.elapsedRealtime()
        heardAt = startedAt
        session++
        Dictation.target = target
        legato.warmPc()
        Dictation.set(Dictation.State.Listening(0f))
        main.post(tick)
    }

    private fun finish() {
        // Done again while the text is written up mustn't end the service,
        // which would throw that text away.
        if (!recorder.isRecording) {
            if (pending == 0) stopSelf()
            return
        }
        main.removeCallbacks(tick)
        // The bar answers the tap at once; the microphone lets go off this thread.
        val take = recorder.halt()
        Dictation.set(Dictation.State.Transcribing)
        getSystemService(NotificationManager::class.java).notify(NOTIFICATION_ID, notification(listening = false))
        val legato = applicationContext.legato
        val mine = session
        val target = Dictation.target
        // A Legato screen takes the words as they were said, with no app's Style.
        val app = if (target == Dictation.Target.Screen) null else BubbleAccessibilityService.instance?.focusedApp() ?: ""
        pending++
        // The screen may be off by now; keep the CPU up until the text is written.
        val wake = awake()
        scope.launch {
            val samples = take?.collect() ?: FloatArray(0)
            val text = runCatching {
                withContext(Dispatchers.Default) {
                    val heard = legato.transcribe(samples, app)
                    // The transform picked for every dictation; the words as heard when it can't run.
                    val after = legato.afterDictation?.takeIf { heard.isNotBlank() && target == Dictation.Target.Field }
                    if (after == null) heard
                    else runCatching { legato.transform(after, heard, app) ?: heard }.getOrElse { e ->
                        legato.onLine("The transform after dictation didn't run: ${e.message}")
                        heard
                    }
                }
            }.getOrElse { e ->
                val why = e.message ?: "Transcription failed"
                legato.onLine(why)
                withContext(Dispatchers.Main) { Toast.makeText(this@DictationService, why, Toast.LENGTH_LONG).show() }
                ""
            }
            withContext(Dispatchers.Main) {
                if (wake.isHeld) wake.release()
                pending--
                // Cancelled while it was written up: the words stay in Home.
                if (text.isNotBlank() && mine <= cancelled) runCatching { legato.core.keepCancelled(text) }
                if (text.isNotBlank() && mine > cancelled && target == Dictation.Target.Screen) {
                    Dictation.forScreen.value = listOfNotNull(Dictation.forScreen.value, text).joinToString(" ")
                } else if (text.isNotBlank() && mine > cancelled) {
                    val service = BubbleAccessibilityService.instance
                    if (service == null) Log.w(BubbleAccessibilityService.TAG, "The bubble's service isn't connected")
                    val typed = service?.insert(text) ?: false
                    if (!typed) {
                        // The field went away while the text was written up; keep it.
                        getSystemService(ClipboardManager::class.java)
                            .setPrimaryClip(ClipData.newPlainText(Legato.DICTATION_CLIP, text))
                        Toast.makeText(this@DictationService, getString(R.string.not_typed), Toast.LENGTH_LONG).show()
                        legato.onLine("Couldn't type into this field, so the text is on the clipboard")
                    }
                    legato.run { dictate(text) }
                }
                if (!recorder.isRecording) {
                    Dictation.set(Dictation.State.Idle)
                    if (pending == 0) {
                        stopForeground(STOP_FOREGROUND_REMOVE)
                        stopSelf()
                    }
                }
            }
        }
    }

    private fun awake(): PowerManager.WakeLock =
        getSystemService(PowerManager::class.java)
            .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "legato:dictation")
            .apply { acquire(5 * 60_000L) }

    private fun cancel() {
        main.removeCallbacks(tick)
        cancelled = session
        if (recorder.isRecording) {
            val heard = recorder.stop()
            applicationContext.legato.onLine("Dictation cancelled")
            // Kept, in case ✕ was a slip.
            applicationContext.legato.keepCancelled(heard)
        }
        Dictation.set(Dictation.State.Idle)
        // One being written up finishes first, to keep its words in Home.
        if (pending == 0) {
            stopForeground(STOP_FOREGROUND_REMOVE)
            stopSelf()
        }
    }

    override fun onDestroy() {
        main.removeCallbacks(tick)
        if (recorder.isRecording) {
            recorder.stop()
            applicationContext.legato.onLine("Dictation stopped: Android ended the recording")
        }
        scope.cancel()
        if (Dictation.state.value !is Dictation.State.Idle) Dictation.set(Dictation.State.Idle)
        super.onDestroy()
    }

    /**
     * The ongoing notification: Done and Cancel while listening, so a
     * dictation started in one app can be finished from anywhere, the
     * notification shade included.
     */
    private fun notification(listening: Boolean = true): Notification {
        val nm = getSystemService(NotificationManager::class.java)
        // Default importance, still silent: some phones (Nothing OS) fold low ones
        // into a row of icons, hiding the buttons. The old low channel goes.
        nm.deleteNotificationChannel(OLD_CHANNEL)
        nm.createNotificationChannel(
            NotificationChannel(CHANNEL, getString(R.string.notification_channel), NotificationManager.IMPORTANCE_DEFAULT).apply {
                setSound(null, null)
                enableVibration(false)
            },
        )
        val open = PendingIntent.getActivity(
            this, 0, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE,
        )
        val stop = PendingIntent.getService(
            this, 1, Intent(this, DictationService::class.java).setAction(ACTION_CANCEL), PendingIntent.FLAG_IMMUTABLE,
        )
        val done = PendingIntent.getService(
            this, 3, Intent(this, DictationService::class.java).setAction(ACTION_FINISH), PendingIntent.FLAG_IMMUTABLE,
        )
        val builder = Notification.Builder(this, CHANNEL)
            .setSmallIcon(R.drawable.ic_bubble)
            .setContentIntent(open)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
        if (!listening) return builder.setContentTitle(getString(R.string.writing_up)).build()
        return builder
            .setContentTitle(getString(R.string.listening))
            .setContentText(getString(R.string.listening_hint))
            .setUsesChronometer(true)
            .setWhen(System.currentTimeMillis())
            .addAction(Notification.Action.Builder(null, getString(R.string.notification_done), done).build())
            .addAction(Notification.Action.Builder(null, getString(R.string.bubble_cancel), stop).build())
            .build()
    }

    companion object {
        private const val SPEECH_LEVEL = 0.03f
        private const val QUIET_LIMIT_MS = 2 * 60_000L
        private const val LONGEST_MS = 20 * 60_000L
        const val ACTION_START = "tech.abhashchakraborty.legato.START"
        const val ACTION_FINISH = "tech.abhashchakraborty.legato.FINISH"
        const val ACTION_CANCEL = "tech.abhashchakraborty.legato.CANCEL"
        const val EXTRA_TARGET = "target"
        private const val CHANNEL = "dictation_shown"
        private const val OLD_CHANNEL = "dictation"
        private const val NOTIFICATION_ID = 1
    }
}
