package tech.abhashchakraborty.legato.bubble

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.widget.Toast
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
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

    val state = kotlinx.coroutines.flow.MutableStateFlow<State>(State.Idle)
    val listeners = CopyOnWriteArrayList<(State) -> Unit>()

    internal fun set(s: State) {
        state.value = s
        listeners.forEach { it(s) }
    }

    fun start(context: Context) = send(context, DictationService.ACTION_START)
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
                Dictation.set(Dictation.State.Listening(recorder.level))
                main.postDelayed(this, 60)
            }
        }
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_START -> start()
            ACTION_FINISH -> finish()
            ACTION_CANCEL -> cancel()
        }
        return START_NOT_STICKY
    }

    private fun start() {
        if (recorder.isRecording) return
        val notification = notification()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            startForeground(NOTIFICATION_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE)
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }
        val legato = applicationContext.legato
        if (legato.model.value !is ModelStatus.Loaded) {
            if (legato.modelFile().exists()) legato.loadModel() else {
                Toast.makeText(this, getString(R.string.no_model), Toast.LENGTH_LONG).show()
                stopSelf()
                return
            }
        }
        recorder.start()
        Dictation.set(Dictation.State.Listening(0f))
        main.post(tick)
    }

    private fun finish() {
        if (!recorder.isRecording) { stopSelf(); return }
        main.removeCallbacks(tick)
        val samples = recorder.stop()
        Dictation.set(Dictation.State.Transcribing)
        val legato = applicationContext.legato
        scope.launch {
            val text = runCatching {
                withContext(Dispatchers.Default) { legato.core.transcribe(samples.toList()) }
            }.getOrElse { e -> legato.onLine(e.message ?: "Transcription failed"); "" }
            withContext(Dispatchers.Main) {
                if (text.isNotBlank()) {
                    val typed = BubbleAccessibilityService.instance?.insert(text) ?: false
                    if (!typed) {
                        Toast.makeText(this@DictationService, getString(R.string.not_typed), Toast.LENGTH_SHORT).show()
                        legato.onLine("Couldn't type into this field: $text")
                    }
                    legato.run { dictate(text) }
                }
                Dictation.set(Dictation.State.Idle)
                stopForeground(STOP_FOREGROUND_REMOVE)
                stopSelf()
            }
        }
    }

    private fun cancel() {
        main.removeCallbacks(tick)
        if (recorder.isRecording) recorder.stop()
        Dictation.set(Dictation.State.Idle)
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    override fun onDestroy() {
        main.removeCallbacks(tick)
        if (recorder.isRecording) recorder.stop()
        scope.cancel()
        if (Dictation.state.value !is Dictation.State.Idle) Dictation.set(Dictation.State.Idle)
        super.onDestroy()
    }

    private fun notification(): Notification {
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(
            NotificationChannel(CHANNEL, getString(R.string.notification_channel), NotificationManager.IMPORTANCE_LOW).apply {
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
        return Notification.Builder(this, CHANNEL)
            .setSmallIcon(R.drawable.ic_bubble)
            .setContentTitle(getString(R.string.listening))
            .setContentIntent(open)
            .addAction(Notification.Action.Builder(null, getString(R.string.bubble_cancel), stop).build())
            .setOngoing(true)
            .build()
    }

    companion object {
        const val ACTION_START = "tech.abhashchakraborty.legato.START"
        const val ACTION_FINISH = "tech.abhashchakraborty.legato.FINISH"
        const val ACTION_CANCEL = "tech.abhashchakraborty.legato.CANCEL"
        private const val CHANNEL = "dictation"
        private const val NOTIFICATION_ID = 1
    }
}
