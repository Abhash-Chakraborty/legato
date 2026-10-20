package tech.abhashchakraborty.legato

import android.app.Application
import android.content.Context
import android.content.SharedPreferences
import android.os.Build
import android.provider.Settings
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import tech.abhashchakraborty.legato.core.Core
import tech.abhashchakraborty.legato.core.CoreListener
import tech.abhashchakraborty.legato.core.PhoneState
import java.io.File
import java.net.HttpURLConnection
import java.net.URL

/** One core per process, created when the app starts. */
class LegatoApplication : Application() {
    lateinit var legato: Legato
        private set

    override fun onCreate() {
        super.onCreate()
        legato = Legato(this)
    }
}

val Context.legato: Legato get() = (applicationContext as LegatoApplication).legato

/**
 * The Kotlin side of the core: holds it, hears from it and keeps the state
 * the screens read. Everything the core says lands in [lines].
 */
class Legato(private val context: Context) : CoreListener {
    val lines = MutableStateFlow<List<String>>(emptyList())
    val state = MutableStateFlow(PhoneState(paired = false, pcName = null, online = false, lastSync = null))
    val model = MutableStateFlow<ModelStatus>(ModelStatus.Missing)
    val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    /** Bumped after every change, local or synced, so lists read again. */
    val version = MutableStateFlow(0)
    /** Failures worth a snackbar, as opposed to the chatter in [lines]. */
    val notices = MutableSharedFlow<String>(extraBufferCapacity = 8)
    val prefs: SharedPreferences = context.getSharedPreferences("legato", Context.MODE_PRIVATE)

    val core: Core = Core(
        File(context.filesDir, "phone").absolutePath,
        deviceName(),
        this,
    )

    init {
        refresh()
        if (modelFile().exists()) model.value = ModelStatus.Downloaded
    }

    override fun onLine(line: String) {
        lines.update { (it + line).takeLast(60) }
        refresh()
        bump()
    }

    fun bump() {
        version.update { it + 1 }
    }

    /** Shows [text] in a snackbar on whatever screen is open. */
    fun notice(text: String) {
        notices.tryEmit(text)
    }

    override fun onUnpaired(why: String) {
        onLine("Unpaired: $why")
        refresh()
    }

    fun refresh() {
        state.value = core.state()
    }

    /** Runs a core call off the main thread and reports a failure as a line. */
    fun run(block: Core.() -> Unit) {
        scope.launch {
            try {
                core.block()
            } catch (e: Exception) {
                val why = e.message ?: e.toString()
                notice(why)
                onLine(why)
            }
            refresh()
            bump()
        }
    }

    // ---- the speech model ----------------------------------------------

    fun modelFile(): File = File(File(context.filesDir, "models").apply { mkdirs() }, MODEL_FILE)

    /** Downloads the model once, then loads it. Progress goes to [model]. */
    fun fetchModel() {
        if (model.value is ModelStatus.Downloading) return
        scope.launch {
            try {
                val file = modelFile()
                if (!file.exists()) download(file)
                model.value = ModelStatus.Downloaded
                loadModel()
            } catch (e: Exception) {
                model.value = ModelStatus.Missing
                onLine("Model download failed: ${e.message}")
            }
        }
    }

    fun loadModel() {
        scope.launch {
            try {
                val arch = core.loadModel(modelFile().absolutePath)
                model.value = ModelStatus.Loaded(arch)
                onLine("Speech model loaded ($arch)")
            } catch (e: Exception) {
                onLine(e.message ?: e.toString())
            }
        }
    }

    private suspend fun download(file: File) = withContext(Dispatchers.IO) {
        val tmp = File(file.path + ".part")
        val conn = (URL(MODEL_URL).openConnection() as HttpURLConnection).apply {
            instanceFollowRedirects = true
            connectTimeout = 15_000
            readTimeout = 30_000
        }
        try {
            if (conn.responseCode !in 200..299) error("HTTP ${conn.responseCode}")
            val total = conn.contentLengthLong
            conn.inputStream.use { input ->
                tmp.outputStream().use { out ->
                    val buf = ByteArray(256 * 1024)
                    var done = 0L
                    while (true) {
                        val n = input.read(buf)
                        if (n < 0) break
                        out.write(buf, 0, n)
                        done += n
                        model.value = ModelStatus.Downloading(if (total > 0) (done * 100 / total).toInt() else -1)
                    }
                }
            }
            if (!tmp.renameTo(file)) error("couldn't move the model into place")
        } finally {
            conn.disconnect()
            tmp.delete()
        }
    }

    private fun deviceName(): String =
        Settings.Global.getString(context.contentResolver, Settings.Global.DEVICE_NAME)
            ?: "${Build.MANUFACTURER} ${Build.MODEL}".trim()

    companion object {
        /** Moonshine Base: small and quick on a phone, English. Swapped for a chooser later. */
        const val MODEL_FILE = "moonshine-base.gguf"
        const val MODEL_URL = "https://huggingface.co/handy-computer/moonshine-base-gguf/resolve/main/moonshine-base-Q8_0.gguf"
    }
}

sealed class ModelStatus {
    data object Missing : ModelStatus()
    data class Downloading(val percent: Int) : ModelStatus()
    data object Downloaded : ModelStatus()
    data class Loaded(val arch: String) : ModelStatus()
}
