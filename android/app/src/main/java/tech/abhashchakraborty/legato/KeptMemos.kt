package tech.abhashchakraborty.legato

import android.content.Context
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import org.json.JSONArray
import tech.abhashchakraborty.legato.core.MemoLine
import java.io.File
import java.io.RandomAccessFile

/**
 * The audio of memo parts that weren't written up (no PC, no model, a
 * dropped link), kept beside the saved memo until they are. One file of
 * 16-bit audio per memo holds just those parts, and a list says where each
 * one sits in the memo. Written up when the PC can do it (cheap for the
 * device) or from the memo's Try again; deleting the memo drops them.
 */
object KeptMemos {
    /** A part's place in the memo and in the kept file, in samples. */
    private data class Part(val atMs: Long, val endMs: Long, val offset: Long, val samples: Int)

    private val _pending = MutableStateFlow<Map<Long, Int>>(emptyMap())
    /** Memo id to the seconds of it still waiting to be written up. */
    val pending: StateFlow<Map<Long, Int>> = _pending.asStateFlow()

    private val busy = mutableSetOf<Long>()

    private fun dir(context: Context) = File(context.filesDir, "kept-memos").apply { mkdirs() }
    private fun audio(context: Context, id: Long) = File(dir(context), "$id.pcm")
    private fun parts(context: Context, id: Long) = File(dir(context), "$id.json")

    fun load(context: Context) {
        val found = dir(context).listFiles { f -> f.name.endsWith(".json") }.orEmpty().mapNotNull { f ->
            val id = f.nameWithoutExtension.toLongOrNull() ?: return@mapNotNull null
            id to readParts(context, id).sumOf { it.samples } / Recorder.SAMPLE_RATE
        }
        _pending.value = found.toMap()
    }

    /** Keeps `spans` (sample ranges of `samples`) of memo `id` to write up later. */
    fun keep(context: Context, id: Long, samples: FloatArray, spans: List<Pair<Int, Int>>) {
        if (spans.isEmpty()) return
        val list = JSONArray()
        var offset = 0L
        audio(context, id).outputStream().buffered().use { out ->
            for ((from, to) in spans) {
                val bytes = Legato.pcm16(samples.copyOfRange(from, to))
                out.write(bytes)
                list.put(JSONArray().put(from / 16L).put(to / 16L).put(offset).put(to - from))
                offset += to - from
            }
        }
        parts(context, id).writeText(list.toString())
        _pending.value = _pending.value + (id to spans.sumOf { it.second - it.first } / Recorder.SAMPLE_RATE)
    }

    fun forget(context: Context, id: Long) {
        audio(context, id).delete()
        parts(context, id).delete()
        _pending.value = _pending.value - id
    }

    private fun readParts(context: Context, id: Long): List<Part> = runCatching {
        val a = JSONArray(parts(context, id).readText())
        (0 until a.length()).map { i ->
            val p = a.getJSONArray(i)
            Part(p.getLong(0), p.getLong(1), p.getLong(2), p.getInt(3))
        }
    }.getOrDefault(emptyList())

    /**
     * Writes up what's kept of memo `id` and adds it to the memo. Parts that
     * fail again stay for next time. Returns how many parts were written up.
     * Blocks; call it off the main thread.
     */
    fun writeUp(context: Context, id: Long): Int {
        synchronized(busy) { if (!busy.add(id)) return 0 }
        try {
            val legato = context.legato
            val all = readParts(context, id)
            if (all.isEmpty()) { forget(context, id); return 0 }
            val lines = ArrayList<MemoLine>()
            val left = ArrayList<Part>()
            RandomAccessFile(audio(context, id), "r").use { file ->
                for (part in all) {
                    val bytes = ByteArray(part.samples * 2)
                    file.seek(part.offset * 2)
                    file.readFully(bytes)
                    val samples = FloatArray(part.samples) { i ->
                        ((bytes[2 * i].toInt() and 0xff) or (bytes[2 * i + 1].toInt() shl 8)).toShort() / 32768f
                    }
                    val text = runCatching { legato.transcribe(samples, Legato.MEMO) }.getOrElse { e ->
                        legato.onLine("A memo part still wasn't written up: ${e.message}")
                        left += part
                        null
                    }?.trim()
                    if (!text.isNullOrEmpty()) lines += MemoLine(atMs = part.atMs, endMs = part.endMs, text = text)
                }
            }
            if (lines.isNotEmpty() && !legato.core.addMemoLines(id, lines)) {
                // The memo was deleted meanwhile.
                forget(context, id)
                return 0
            }
            if (left.isEmpty()) forget(context, id)
            else {
                // What's left keeps its place in the file; only the list shrinks.
                val list = JSONArray()
                left.forEach { list.put(JSONArray().put(it.atMs).put(it.endMs).put(it.offset).put(it.samples)) }
                parts(context, id).writeText(list.toString())
                _pending.value = _pending.value + (id to left.sumOf { it.samples } / Recorder.SAMPLE_RATE)
            }
            if (lines.isNotEmpty()) {
                legato.onLine("Wrote up ${lines.size} kept part${if (lines.size == 1) "" else "s"} of a memo")
                legato.bump()
            }
            return all.size - left.size
        } finally {
            synchronized(busy) { busy.remove(id) }
        }
    }

    /** Writes up every kept memo, when the PC can (the device's model would cost more than a tap). */
    fun writeUpAllOnPc(context: Context) {
        val legato = context.legato
        if (!legato.pcWillTranscribe()) return
        for (id in _pending.value.keys) writeUp(context, id)
    }
}
