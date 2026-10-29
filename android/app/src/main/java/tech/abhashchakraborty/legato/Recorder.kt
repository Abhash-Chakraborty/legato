package tech.abhashchakraborty.legato

import android.annotation.SuppressLint
import android.media.AudioFormat
import android.media.AudioRecord
import android.media.MediaRecorder
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.concurrent.thread

/**
 * The microphone as 16 kHz mono float samples, which is what the speech
 * model takes. Start, talk, stop; stop returns everything heard.
 */
class Recorder {
    /** One recording: its own flag, thread and audio, so the next can start before this one is collected. */
    class Take internal constructor() {
        internal val running = AtomicBoolean(true)
        internal val chunks = ArrayList<ShortArray>()
        internal var worker: Thread? = null

        /** What it heard, once the microphone has let go, as -1..1 floats at 16 kHz. Blocks briefly. */
        fun collect(): FloatArray {
            worker?.join(1500)
            return synchronized(chunks) {
                val out = FloatArray(chunks.sumOf { it.size })
                var at = 0
                for (c in chunks) { for (s in c) { out[at++] = s / 32768f } }
                chunks.clear()
                out
            }
        }
    }

    @Volatile private var current: Take? = null

    /** Peak level of the last chunk, 0..1, for a meter. */
    @Volatile var level: Float = 0f
        private set

    val isRecording: Boolean get() = current?.running?.get() == true

    @SuppressLint("MissingPermission") // asked for before start() is reachable
    @Synchronized
    fun start() {
        if (isRecording) return
        val take = Take()
        val minBuf = AudioRecord.getMinBufferSize(SAMPLE_RATE, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT)
        val record = AudioRecord(
            MediaRecorder.AudioSource.VOICE_RECOGNITION,
            SAMPLE_RATE,
            AudioFormat.CHANNEL_IN_MONO,
            AudioFormat.ENCODING_PCM_16BIT,
            maxOf(minBuf, SAMPLE_RATE / 2),
        )
        current = take
        take.worker = thread(name = "legato-mic") {
            record.startRecording()
            val buf = ShortArray(SAMPLE_RATE / 10) // 100 ms
            while (take.running.get()) {
                val n = record.read(buf, 0, buf.size)
                if (n > 0) {
                    synchronized(take.chunks) { take.chunks.add(buf.copyOf(n)) }
                    var peak = 0
                    for (i in 0 until n) { val a = kotlin.math.abs(buf[i].toInt()); if (a > peak) peak = a }
                    level = peak / 32768f
                }
            }
            record.stop()
            record.release()
        }
    }

    /** Stops and returns the recording as -1..1 floats at 16 kHz. */
    fun stop(): FloatArray = halt()?.collect() ?: FloatArray(0)

    /** Stops listening at once, without waiting for the microphone; the take collects what it heard. */
    @Synchronized
    fun halt(): Take? {
        val take = current ?: return null
        if (!take.running.compareAndSet(true, false)) return null
        current = null
        level = 0f
        return take
    }

    companion object {
        const val SAMPLE_RATE = 16_000
    }
}
