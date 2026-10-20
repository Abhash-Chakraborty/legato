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
    private val running = AtomicBoolean(false)
    private var worker: Thread? = null
    private val chunks = ArrayList<ShortArray>()

    /** Peak level of the last chunk, 0..1, for a meter. */
    @Volatile var level: Float = 0f
        private set

    val isRecording: Boolean get() = running.get()

    @SuppressLint("MissingPermission") // asked for before start() is reachable
    fun start() {
        if (!running.compareAndSet(false, true)) return
        chunks.clear()
        val minBuf = AudioRecord.getMinBufferSize(SAMPLE_RATE, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT)
        val record = AudioRecord(
            MediaRecorder.AudioSource.VOICE_RECOGNITION,
            SAMPLE_RATE,
            AudioFormat.CHANNEL_IN_MONO,
            AudioFormat.ENCODING_PCM_16BIT,
            maxOf(minBuf, SAMPLE_RATE / 2),
        )
        worker = thread(name = "legato-mic") {
            record.startRecording()
            val buf = ShortArray(SAMPLE_RATE / 10) // 100 ms
            while (running.get()) {
                val n = record.read(buf, 0, buf.size)
                if (n > 0) {
                    synchronized(chunks) { chunks.add(buf.copyOf(n)) }
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
    fun stop(): FloatArray {
        if (!running.compareAndSet(true, false)) return FloatArray(0)
        worker?.join(1500)
        worker = null
        level = 0f
        val total = synchronized(chunks) { chunks.sumOf { it.size } }
        val out = FloatArray(total)
        var at = 0
        synchronized(chunks) {
            for (c in chunks) { for (s in c) { out[at++] = s / 32768f } }
            chunks.clear()
        }
        return out
    }

    companion object {
        const val SAMPLE_RATE = 16_000
    }
}
