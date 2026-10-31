package tech.abhashchakraborty.legato

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.PowerManager
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import tech.abhashchakraborty.legato.bubble.Dictation
import tech.abhashchakraborty.legato.core.MemoLine
import java.text.DateFormat
import java.util.Date

/** A voice memo as the Notes tab shows it: idle, recording, or being written up. */
object Memo {
    data class State(
        val recording: Boolean = false,
        val startedAt: Long = 0,
        val level: Float = 0f,
        /** "Writing up" while the text is on its way, else null. */
        val writing: String? = null,
        /** What's written up so far while it records, half a minute at a time. */
        val heard: String = "",
    )

    val state = MutableStateFlow(State())

    fun start(context: Context) {
        context.startForegroundService(Intent(context, MemoService::class.java).setAction(MemoService.ACTION_START))
    }

    fun finish(context: Context) {
        context.startService(Intent(context, MemoService::class.java).setAction(MemoService.ACTION_FINISH))
    }
}

/**
 * Records a memo as the foreground service Android asks for, so it keeps
 * hearing you with the screen off or another app in front, then writes it
 * up: by the PC's model while connected, else this device's. Long memos go
 * in pieces of about half a minute, cut where it's quietest, each a line of
 * the transcript, written up while you're still talking, so Stop leaves
 * only the last piece. The note syncs to the PC like any other.
 */
class MemoService : Service() {
    private val recorder = Recorder()
    private val main = Handler(Looper.getMainLooper())
    /** Lines written up while recording, and the sample they reach. */
    private val lines = ArrayList<MemoLine>()
    @Volatile private var writtenTo = 0
    private var live: Job? = null
    private val tick = object : Runnable {
        override fun run() {
            if (recorder.isRecording) {
                Memo.state.update { it.copy(level = recorder.level) }
                main.postDelayed(this, 60)
            }
        }
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_START -> start()
            ACTION_FINISH -> finish()
        }
        return START_NOT_STICKY
    }

    private fun start() {
        startForeground(NOTIFICATION_ID, notification(), ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE)
        // The bubble's dictation and a memo can't share the microphone.
        if (recorder.isRecording) return
        if (Dictation.state.value !is Dictation.State.Idle) {
            applicationContext.legato.notice("Finish the dictation first.")
            done()
            return
        }
        recorder.start()
        Memo.state.value = Memo.State(recording = true, startedAt = System.currentTimeMillis())
        main.post(tick)
        synchronized(lines) { lines.clear() }
        writtenTo = 0
        val take = recorder.take
        live = take?.let { applicationContext.legato.scope.launch(Dispatchers.IO) { writeAsYouGo(it) } }
    }

    /**
     * Writes up each half minute once there's more than that past the last
     * cut, while the recording goes on. A piece that fails, or no model to
     * write with, leaves the rest for Stop.
     */
    private suspend fun writeAsYouGo(take: Recorder.Take) {
        val legato = applicationContext.legato
        val rate = Recorder.SAMPLE_RATE
        while (take.running.get()) {
            delay(2_000)
            if (take.heard - writtenTo <= PIECE * rate || !legato.canTranscribe()) continue
            val window = take.slice(writtenTo, writtenTo + PIECE * rate)
            val end = quietest(window, 0)
            if (!quiet(window, 0, end)) {
                val text = try {
                    // Nobody waits on these, so the PC doesn't race the
                    // device's model, which would only take the CPU from
                    // the recording.
                    legato.transcribe(window.copyOfRange(0, end), Legato.MEMO, race = false).trim()
                } catch (e: Exception) {
                    legato.onLine("The memo will be written up when it stops (${e.message ?: "transcription failed"})")
                    return
                }
                if (text.isNotEmpty()) {
                    val line = MemoLine(atMs = writtenTo / 16L, endMs = (writtenTo + end) / 16L, text = text)
                    val heard = synchronized(lines) { lines += line; lines.joinToString(" ") { it.text } }
                    Memo.state.update { if (it.recording) it.copy(heard = heard) else it }
                }
            }
            writtenTo += end
        }
    }

    private fun finish() {
        if (!recorder.isRecording) { done(); return }
        main.removeCallbacks(tick)
        val started = Memo.state.value.startedAt
        val samples = recorder.stop()
        Memo.state.value = Memo.State(writing = "Writing up")
        val legato = applicationContext.legato
        // A long memo takes a while to write up, often with the screen off.
        val wake = getSystemService(PowerManager::class.java)
            .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "legato:memo")
            .apply { acquire(15 * 60_000L) }
        legato.scope.launch {
            try {
                // A piece being written up as it stopped finishes first (a
                // write-up isn't interrupted, only the wait for the next).
                live?.let { it.cancel(); it.join() }
                val lines = synchronized(this@MemoService.lines) { ArrayList(this@MemoService.lines) }
                val pieces = cut(samples, writtenTo)
                if (pieces.isNotEmpty() && !legato.readyToTranscribe()) {
                    legato.notice("Couldn't reach ${legato.state.value.pcName ?: "your PC"}, so the memo wasn't written up.")
                    return@launch
                }
                for ((n, piece) in pieces.withIndex()) {
                    if (pieces.size > 1) Memo.state.value = Memo.State(writing = "Writing up · ${n + 1} of ${pieces.size}")
                    val (from, to) = piece
                    if (quiet(samples, from, to)) continue
                    val text = runCatching { legato.transcribe(samples.copyOfRange(from, to), Legato.MEMO) }.getOrElse { e ->
                        legato.onLine(e.message ?: "Transcription failed"); ""
                    }.trim()
                    if (text.isNotEmpty()) lines += MemoLine(atMs = from / 16L, endMs = to / 16L, text = text)
                }
                if (lines.isEmpty()) {
                    legato.notice("Legato heard no words, so the memo wasn't saved.")
                } else {
                    val title = "Voice memo · " + DateFormat.getTimeInstance(DateFormat.SHORT).format(Date(started))
                    legato.core.addMemo(title, started / 1000, samples.size / 16L, lines)
                    legato.bump()
                    val pc = legato.state.value
                    legato.notice(if (pc.paired) "Memo saved; it syncs to ${pc.pcName ?: "your PC"}." else "Memo saved.")
                }
            } catch (e: Exception) {
                legato.notice(e.message ?: "Couldn't save the memo")
            } finally {
                if (wake.isHeld) wake.release()
                main.post { done() }
            }
        }
    }

    private fun done() {
        Memo.state.value = Memo.State()
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    override fun onDestroy() {
        main.removeCallbacks(tick)
        live?.cancel()
        if (recorder.isRecording) recorder.stop()
        if (Memo.state.value.recording) Memo.state.value = Memo.State()
        super.onDestroy()
    }

    /** Pieces of at most 30 s from sample `start` on, each cut at the quietest 100 ms after the 20 s mark. */
    private fun cut(samples: FloatArray, start: Int = 0): List<Pair<Int, Int>> {
        val rate = Recorder.SAMPLE_RATE
        val out = ArrayList<Pair<Int, Int>>()
        var from = start
        while (from < samples.size) {
            if (samples.size - from <= PIECE * rate) { out += from to samples.size; break }
            val best = quietest(samples, from)
            out += from to best
            from = best
        }
        return out
    }

    /** Where to end a piece starting at `from`: the middle of the quietest 100 ms between 20 and 30 s in. */
    private fun quietest(samples: FloatArray, from: Int): Int {
        val rate = Recorder.SAMPLE_RATE
        var best = from + PIECE * rate
        var bestEnergy = Double.MAX_VALUE
        var at = from + 20 * rate
        while (at + rate / 10 <= minOf(from + PIECE * rate, samples.size)) {
            var e = 0.0
            for (i in at until at + rate / 10) e += samples[i] * samples[i]
            if (e < bestEnergy) { bestEnergy = e; best = at + rate / 20 }
            at += rate / 10
        }
        return minOf(best, samples.size)
    }

    /** A stretch with no speech in it; the models make words up from silence. */
    private fun quiet(samples: FloatArray, from: Int, to: Int): Boolean {
        var peak = 0f
        for (i in from until to) { val a = kotlin.math.abs(samples[i]); if (a > peak) peak = a }
        return peak < 0.02f
    }

    private fun notification(): Notification {
        val nm = getSystemService(NotificationManager::class.java)
        // Default importance, still silent: some phones (Nothing OS) fold low ones
        // into a row of icons, hiding the buttons. The old low channel goes.
        nm.deleteNotificationChannel(OLD_CHANNEL)
        nm.createNotificationChannel(
            NotificationChannel(CHANNEL, getString(R.string.memo_channel), NotificationManager.IMPORTANCE_DEFAULT).apply {
                setSound(null, null)
                enableVibration(false)
            },
        )
        val open = PendingIntent.getActivity(this, 0, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE)
        val stop = PendingIntent.getService(
            this, 2, Intent(this, MemoService::class.java).setAction(ACTION_FINISH), PendingIntent.FLAG_IMMUTABLE,
        )
        return Notification.Builder(this, CHANNEL)
            .setSmallIcon(R.drawable.ic_bubble)
            .setContentTitle(getString(R.string.memo_recording))
            .setUsesChronometer(true)
            .setWhen(System.currentTimeMillis())
            .setContentIntent(open)
            .addAction(Notification.Action.Builder(null, getString(R.string.memo_stop), stop).build())
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .build()
    }

    companion object {
        const val ACTION_START = "tech.abhashchakraborty.legato.MEMO_START"
        const val ACTION_FINISH = "tech.abhashchakraborty.legato.MEMO_FINISH"
        private const val CHANNEL = "memo_shown"
        private const val OLD_CHANNEL = "memo"
        private const val NOTIFICATION_ID = 2
        /** The longest piece written up at once, in seconds. */
        private const val PIECE = 30
    }
}
