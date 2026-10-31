package tech.abhashchakraborty.legato

import android.annotation.SuppressLint
import android.media.AudioFormat
import android.media.AudioRecord
import android.media.MediaRecorder
import android.os.Process
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

        /** How many samples it has heard so far. */
        @Volatile var heard: Int = 0
            internal set

        /** Samples `from` until `to` heard so far, as -1..1 floats, keeping the recording whole. */
        fun slice(from: Int, to: Int): FloatArray = synchronized(chunks) {
            val end = minOf(to, heard)
            val out = FloatArray(maxOf(0, end - from))
            var at = 0
            for (c in chunks) {
                if (at + c.size > from && at < end) {
                    for (i in maxOf(0, from - at) until minOf(c.size, end - at)) out[at + i - from] = c[i] / 32768f
                }
                at += c.size
                if (at >= end) break
            }
            out
        }

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

    /** The recording under way, for reading what it has heard so far. */
    val take: Take? get() = current

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
            // Two seconds of room, so a busy processor (a memo written up
            // while it records) doesn't lose what the microphone heard.
            maxOf(minBuf, SAMPLE_RATE * 4),
        )
        current = take
        take.worker = thread(name = "legato-mic") {
            Process.setThreadPriority(Process.THREAD_PRIORITY_URGENT_AUDIO)
            record.startRecording()
            val buf = ShortArray(SAMPLE_RATE / 10) // 100 ms
            while (take.running.get()) {
                val n = record.read(buf, 0, buf.size)
                if (n > 0) {
                    synchronized(take.chunks) {
                        take.chunks.add(buf.copyOf(n))
                        take.heard += n
                    }
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
