package tech.abhashchakraborty.legato

import android.app.Application
import android.content.Context
import android.content.SharedPreferences
import android.os.Build
import android.provider.Settings
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
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
    val state = MutableStateFlow(PhoneState(paired = false, pcName = null, online = false, via = null, pcTranscribes = false, lastSync = null))
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
        // Whatever woke this process (the bubble, a memo, a share) may need the
        // PC, and Android restarts it in the background without the app open.
        if (state.value.paired) run { connect() }
        // A new network (home Wi-Fi again, say) is worth a try at once,
        // rather than after the reconnect backoff.
        runCatching {
            context.getSystemService(android.net.ConnectivityManager::class.java)
                .registerDefaultNetworkCallback(object : android.net.ConnectivityManager.NetworkCallback() {
                    override fun onAvailable(network: android.net.Network) {
                        if (state.value.paired && !state.value.online) runCatching { core.connect() }
                    }
                })
        }
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

    // ---- the shared clipboard -------------------------------------------

    /**
     * Automatic: a copy on the PC lands on this clipboard at once, and what
     * you copied here goes to the PC when you open Legato (Android lets an
     * app read the clipboard only while it's in front). Manual: nothing moves
     * until you send it or pick a clip.
     */
    var autoClipboard: Boolean
        get() = prefs.getBoolean(PREF_AUTO_CLIP, true)
        set(value) { prefs.edit().putBoolean(PREF_AUTO_CLIP, value).apply() }

    /** The last text that went either way, so a clip never bounces back. */
    @Volatile private var lastClip: String? = null
    private val main = android.os.Handler(android.os.Looper.getMainLooper())

    override fun onClip(text: String, fromApp: String?, ageMs: ULong) {
        lastClip = text
        bump()
        // An old clip (the phone was away) waits in Recent clips rather
        // than replacing what you copied since.
        if (!autoClipboard || ageMs > 120_000uL) return
        main.post {
            runCatching {
                context.getSystemService(android.content.ClipboardManager::class.java)
                    .setPrimaryClip(android.content.ClipData.newPlainText("Legato", text))
            }
        }
    }

    /** Sends [text] to the PC unless it just came from there. True when it went. */
    fun sendClip(text: String): Boolean {
        val t = text.trim()
        if (t.isEmpty() || t == lastClip?.trim()) return false
        val sent = runCatching { core.sendClip(t) }.getOrDefault(false)
        if (sent) lastClip = t
        return sent
    }

    /** Called when Legato comes to the front: in Automatic, sends what's new on the clipboard. */
    fun clipboardOnFocus() {
        if (!autoClipboard || !state.value.online) return
        val text = runCatching {
            val clipboard = context.getSystemService(android.content.ClipboardManager::class.java)
            // Password managers mark what they copy; that stays on the phone.
            val extras = clipboard.primaryClipDescription?.extras
            if (extras?.getBoolean("android.content.extra.IS_SENSITIVE") == true) return
            clipboard.primaryClip?.getItemAt(0)?.coerceToText(context)?.toString()
        }.getOrNull() ?: return
        scope.launch { if (sendClip(text)) onLine("Sent your clipboard to ${state.value.pcName ?: "the PC"}") }
    }

    override fun openBluetooth(address: String, token: String): UShort =
        BluetoothLink.open(context, address, token).toUShort()

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

    /** What this device is, for the words on screen. */
    val device: String
        get() = if (context.resources.configuration.smallestScreenWidthDp >= 600) "tablet" else "phone"

    /** Whether to send recordings to the PC's model while connected. */
    var usePcModel: Boolean
        get() = prefs.getBoolean(PREF_PC_MODEL, true)
        set(value) { prefs.edit().putBoolean(PREF_PC_MODEL, value).apply() }

    /** The PC will write up what you say right now. */
    fun pcWillTranscribe(): Boolean = usePcModel && core.state().pcTranscribes

    /** Paired and set to use the PC's model, so it can write up what you say once it's reached. */
    fun pcMayTranscribe(): Boolean = usePcModel && state.value.paired

    /** Something can turn speech into text: the PC, or the model on this device. */
    fun canTranscribe(): Boolean = pcWillTranscribe() || model.value is ModelStatus.Loaded

    /**
     * Waits a moment for a PC that's still reconnecting (Android had stopped
     * Legato), then says whether anything can write up speech. Blocks.
     */
    fun readyToTranscribe(): Boolean {
        val until = System.currentTimeMillis() + 8_000
        while (!canTranscribe() && pcMayTranscribe() && System.currentTimeMillis() < until) Thread.sleep(250)
        return canTranscribe()
    }

    /**
     * Has the PC load its speech model as you start talking, so a model it
     * had put away doesn't hold up the text. Waits a moment for a PC that's
     * still being reached.
     */
    fun warmPc() {
        if (!pcMayTranscribe()) return
        scope.launch(Dispatchers.IO) {
            val until = System.currentTimeMillis() + 8_000
            while (!pcWillTranscribe() && pcMayTranscribe() && System.currentTimeMillis() < until) delay(250)
            if (pcWillTranscribe()) runCatching { core.warmPc() }
        }
    }

    /**
     * Your words as text: by the PC's model while connected to a PC that
     * offers it (bigger and quicker than the one on this device), else by
     * this device's own. With `app`, the package typed into, this device's
     * text gets the Dictionary, snippets, cleanup and Style for that kind of
     * app, as the PC's does. Blocks; call it off the main thread.
     */
    fun transcribe(samples: FloatArray, app: String? = null): String {
        if (!readyToTranscribe() && pcMayTranscribe()) error("Couldn't reach ${state.value.pcName ?: "your PC"} to write this up")
        if (pcWillTranscribe()) {
            try {
                return core.transcribeOnPc(samples.toList(), app ?: "")
            } catch (e: Exception) {
                if (model.value !is ModelStatus.Loaded) throw e
                onLine("The PC couldn't transcribe (${e.message}); used this device's model")
            }
        }
        val heard = core.transcribe(samples.toList())
        // The PC's text comes finished; this device's is finished here, for the app.
        if (app == null || heard.isBlank()) return heard
        return runCatching { core.finishDictation(heard, app) }.getOrElse { e ->
            onLine("Couldn't apply the Dictionary and Style: ${e.message}")
            heard
        }
    }

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
        const val PREF_PC_MODEL = "pc_model"
        const val PREF_AUTO_CLIP = "auto_clipboard"
        const val MODEL_URL = "https://huggingface.co/handy-computer/moonshine-base-gguf/resolve/main/moonshine-base-Q8_0.gguf"
    }
}

/** How the phone reaches the PC right now, in words. */
val PhoneState.connectedText: String
    get() = when (via) {
        "bluetooth" -> "Connected over Bluetooth"
        "netbird" -> "Connected through Netbird"
        else -> "Connected on Wi-Fi"
    }

sealed class ModelStatus {
    data object Missing : ModelStatus()
    data class Downloading(val percent: Int) : ModelStatus()
    data object Downloaded : ModelStatus()
    data class Loaded(val arch: String) : ModelStatus()
}
