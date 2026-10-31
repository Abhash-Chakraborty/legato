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
import kotlinx.coroutines.flow.updateAndGet
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

    override fun onTrimMemory(level: Int) {
        super.onTrimMemory(level)
        legato.trimMemory(level)
    }
}

val Context.legato: Legato get() = (applicationContext as LegatoApplication).legato

/**
 * The Kotlin side of the core: holds it, hears from it and keeps the state
 * the screens read. Everything the core says lands in [lines].
 */
class Legato(private val context: Context) : CoreListener {
    /** The log, kept on the phone across restarts so it still tells what happened after Android stopped Legato. */
    private val logFile = File(context.filesDir, "log.txt")
    val lines = MutableStateFlow(
        runCatching {
            // Builds before this one logged what each clip from the PC said; those lines go.
            logFile.readLines().filterNot { "Clip from the PC: “" in it }.takeLast(LOG_LINES)
        }.getOrDefault(emptyList()),
    )
    val state = MutableStateFlow(PhoneState(paired = false, pcName = null, online = false, via = null, pcTranscribes = false, lastSync = null))
    val model = MutableStateFlow<ModelStatus>(ModelStatus.Missing)
    val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    /** Bumped after every change, local or synced, so lists read again. */
    val version = MutableStateFlow(0)
    /** Failures worth a snackbar, as opposed to the chatter in [lines]. */
    val notices = MutableSharedFlow<String>(extraBufferCapacity = 8)
    val prefs: SharedPreferences = context.getSharedPreferences("legato", Context.MODE_PRIVATE)
    /** API keys the PC shared, sealed by the Android Keystore. Before the core, which may hand some over at once. */
    val vault = Vault(context)

    val core: Core = Core(
        File(context.filesDir, "phone").absolutePath,
        deviceName(),
        this,
    )

    init {
        logCrashes()
        refresh()
        // Before the chooser, Moonshine Base lived under a name of its own.
        File(modelsDir(), "moonshine-base.gguf").takeIf { it.exists() }
            ?.renameTo(File(modelsDir(), PhoneModels.byId("moonshine-base").file))
        // Moonshine Base was the default until Parakeet 110M; whoever has it keeps it.
        if (!prefs.contains(PREF_MODEL) && File(modelsDir(), PhoneModels.byId("moonshine-base").file).exists()) {
            prefs.edit().putString(PREF_MODEL, "moonshine-base").apply()
        }
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
                        wakeLink()
                    }
                })
        }
        // With the screen on, a PC out of reach is tried at once and then every
        // half minute (a laptop waking up is found while the phone is in hand);
        // with it off, every five minutes.
        runCatching {
            val screen = object : android.content.BroadcastReceiver() {
                override fun onReceive(c: Context, intent: android.content.Intent) {
                    core.setScreenOn(intent.action == android.content.Intent.ACTION_SCREEN_ON)
                }
            }
            val filter = android.content.IntentFilter().apply {
                addAction(android.content.Intent.ACTION_SCREEN_ON)
                addAction(android.content.Intent.ACTION_SCREEN_OFF)
            }
            if (android.os.Build.VERSION.SDK_INT >= 33) {
                context.registerReceiver(screen, filter, Context.RECEIVER_NOT_EXPORTED)
            } else {
                context.registerReceiver(screen, filter)
            }
            core.setScreenOn(context.getSystemService(android.os.PowerManager::class.java).isInteractive)
        }
    }

    /** Tries the PC now, if paired and not connected, rather than after the backoff. */
    fun wakeLink() {
        if (state.value.paired && !state.value.online) runCatching { core.connect() }
    }

    override fun onLine(line: String) {
        val stamp = java.text.SimpleDateFormat("MMM d HH:mm:ss", java.util.Locale.US).format(java.util.Date())
        val entry = "$stamp  $line"
        val all = lines.updateAndGet { (it + entry).takeLast(LOG_LINES) }
        // Each line is added to the file; it's cut back to the last lines only
        // once it has grown to twice that, rather than rewritten every time.
        scope.launch(Dispatchers.IO) {
            synchronized(logFile) {
                runCatching {
                    if (++logged > LOG_LINES * 2) {
                        logFile.writeText(all.joinToString("\n", postfix = "\n"))
                        logged = all.size
                    } else {
                        logFile.appendText(entry + "\n")
                    }
                }
            }
        }
        refresh()
        bump()
    }

    /** Lines in the log file, for knowing when to cut it back. Under the file's lock. */
    private var logged = runCatching { logFile.readLines().size }.getOrDefault(0)

    /** A crash goes in the log before Android closes Legato. */
    private fun logCrashes() {
        val previous = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { thread, e ->
            runCatching {
                val stamp = java.text.SimpleDateFormat("MMM d HH:mm:ss", java.util.Locale.US).format(java.util.Date())
                val where = e.stackTrace.take(4).joinToString(" < ") { "${it.fileName}:${it.lineNumber}" }
                synchronized(logFile) { logFile.appendText("$stamp  Crashed on ${thread.name}: $e at $where\n") }
            }
            previous?.uncaughtException(thread, e)
        }
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

    /**
     * When the clip last sent from here was copied, kept across restarts:
     * Android restarts Legato often, and the same old clip mustn't go to the
     * PC again over something copied there since.
     */
    private var clipSent: Long
        get() = prefs.getLong(PREF_CLIP_SENT, 0)
        set(value) { prefs.edit().putLong(PREF_CLIP_SENT, value).apply() }
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
                    .setPrimaryClip(android.content.ClipData.newPlainText(PC_CLIP, text))
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

    /**
     * A copy made in another app, which the bubble's service noticed by
     * Android's clipboard chip. Android hands the clipboard only to the app
     * in front, so the text is what was selected just before; with nothing
     * selected (an app's own Copy button), Legato comes to the front for a
     * moment to read it, unless a keyboard is open: that would take the
     * field you're typing in out of focus. Automatic clipboard only.
     */
    fun copiedElsewhere(selected: String?, keyboardOpen: Boolean) {
        if (!autoClipboard || !state.value.online) return
        if (selected != null) {
            scope.launch { if (sendClip(selected)) onLine("Sent what you copied to ${state.value.pcName ?: "the PC"}") }
            return
        }
        if (keyboardOpen) return
        runCatching {
            context.startActivity(
                android.content.Intent(context, ClipReadActivity::class.java)
                    .addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK or android.content.Intent.FLAG_ACTIVITY_NO_ANIMATION),
            )
        }.onFailure { onLine("Couldn't read what you copied: ${it.message}") }
    }

    /** Called when Legato comes to the front: in Automatic, sends what's new on the clipboard. */
    fun clipboardOnFocus() {
        if (!autoClipboard || !state.value.online) return
        var copied = 0L
        val text = runCatching {
            val clipboard = context.getSystemService(android.content.ClipboardManager::class.java)
            val description = clipboard.primaryClipDescription ?: return
            // Password managers mark what they copy; that stays on the phone.
            if (description.extras?.getBoolean("android.content.extra.IS_SENSITIVE") == true) return
            // A dictation the bubble couldn't type stays on the phone, as the PC
            // keeps its own pastes out of the shared clipboard; a clip from the
            // PC is already there.
            if (description.label == DICTATION_CLIP || description.label == PC_CLIP) return
            // Each copy has its own time; the one already sent doesn't go again.
            copied = description.timestamp
            if (copied != 0L) {
                // The first time after this check arrived, take what's there as sent.
                if (!prefs.contains(PREF_CLIP_SENT)) clipSent = copied
                if (copied == clipSent) return
            }
            clipboard.primaryClip?.getItemAt(0)?.coerceToText(context)?.toString()
        }.getOrNull() ?: return
        scope.launch {
            if (sendClip(text)) onLine("Sent your clipboard to ${state.value.pcName ?: "the PC"}")
            // Sent now or already on the PC; one that didn't go tries again next time.
            if (text.trim() == lastClip?.trim()) clipSent = copied
        }
    }

    override fun openBluetooth(address: String, token: String): UShort =
        BluetoothLink.open(context, address, token).toUShort()

    override fun onUnpaired(why: String) {
        onLine("Unpaired: $why")
        vault.replace(KEY_PREFIX, emptyMap())
        refresh()
    }

    // ---- transforms ------------------------------------------------------

    override fun onAiKeys(keys: List<tech.abhashchakraborty.legato.core.ProviderKey>) {
        vault.replace(KEY_PREFIX, keys.mapNotNull { k -> k.key?.let { KEY_PREFIX + k.providerId to it } }.toMap())
        bump()
    }

    // This phone's TLS key, sealed by the Keystore so a copy of the files can't pose as it.
    override fun seal(plain: ByteArray): ByteArray? = vault.sealBytes(plain)

    override fun unseal(sealed: ByteArray): ByteArray? = vault.openBytes(sealed)

    /** Whether a key for `providerId` is kept here (never what it is). */
    fun hasKey(providerId: String): Boolean = vault.has(KEY_PREFIX + providerId)

    /** The transform run on every dictation before it's typed, if any. */
    var afterDictation: String?
        get() = prefs.getString(PREF_AFTER, null)
        set(value) { prefs.edit().apply { if (value == null) remove(PREF_AFTER) else putString(PREF_AFTER, value) }.apply() }

    /**
     * Runs transform `id` on `text`, for the app `app`: on the PC while
     * connected (its key never leaves it), else here with the provider and
     * key it shared. Null when the model had nothing to change. Blocks.
     */
    fun transform(id: String, text: String, app: String? = null): String? {
        if (core.pcTransforms()) {
            try {
                return core.transformOnPc(id, text, app ?: "")
            } catch (e: Exception) {
                if (core.transformRequest(id, app ?: "", null) == null) throw e
                onLine("${state.value.pcName ?: "The PC"} couldn't run the transform (${e.message}); ran it here")
            }
        }
        val request = core.transformRequest(id, app ?: "", null)
            ?: error("Connect to ${state.value.pcName ?: "your PC"} to run this, or turn on Share AI keys with your phone there")
        val answer = LlmClient.complete(request, vault.get(KEY_PREFIX + request.providerId), text)
        return core.readTransformAnswer(answer, text)
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

    /**
     * Whether a dictation keeps listening when the keyboard closes (switching
     * apps, the notification shade, the screen timing out). Off: closing
     * the keyboard ends it, and the words are written up.
     */
    var listenThroughKeyboardClose: Boolean
        get() = prefs.getBoolean(PREF_KEEP_LISTENING, true)
        set(value) { prefs.edit().putBoolean(PREF_KEEP_LISTENING, value).apply() }

    // ---- a cancelled dictation --------------------------------------------

    /** The audio of the last dictation cancelled before its words were written up. */
    private val cancelledFile = File(context.filesDir, "cancelled.pcm")

    /** How long the dictation waiting in [cancelledFile] is, in seconds; null when none waits. */
    val cancelled = MutableStateFlow(
        cancelledFile.takeIf { it.exists() }?.let { (it.length() / 32_000L).toInt() },
    )

    /**
     * Keeps a cancelled dictation (✕, or Cancel in the notification) so its
     * words aren't lost to a slip. Written up at once when the PC does it
     * (cheap for the phone); otherwise its audio waits for Home's Write it up,
     * as loading this device's model for it would cost more than a tap.
     */
    fun keepCancelled(samples: FloatArray) {
        // Under a second is a slip of the finger, not words.
        if (samples.size < 16_000) return
        scope.launch(Dispatchers.IO) {
            runCatching { cancelledFile.writeBytes(pcm16(samples)) }
                .onFailure { onLine("Couldn't keep the cancelled dictation: ${it.message}"); return@launch }
            cancelled.value = samples.size / 16_000
            onLine("Kept the cancelled dictation")
            if (pcWillTranscribe()) writeUpCancelled()
        }
    }

    /** Writes up the cancelled dictation into Home. Blocks; call it off the main thread. */
    fun writeUpCancelled(): Boolean {
        val bytes = runCatching { cancelledFile.readBytes() }.getOrNull() ?: return false
        val samples = FloatArray(bytes.size / 2) { i ->
            ((bytes[2 * i].toInt() and 0xff) or (bytes[2 * i + 1].toInt() shl 8)).toShort() / 32768f
        }
        val text = runCatching { transcribe(samples, null) }.getOrElse { e ->
            onLine("Couldn't write up the cancelled dictation: ${e.message}")
            return false
        }
        if (text.isNotBlank()) runCatching { core.keepCancelled(text) }
        discardCancelled()
        bump()
        return text.isNotBlank()
    }

    fun discardCancelled() {
        cancelledFile.delete()
        cancelled.value = null
    }

    /** Whether to send recordings to the PC's model while connected. */
    var usePcModel: Boolean
        get() = prefs.getBoolean(PREF_PC_MODEL, true)
        set(value) { prefs.edit().putBoolean(PREF_PC_MODEL, value).apply() }

    /** The PC will write up what you say right now. */
    fun pcWillTranscribe(): Boolean = usePcModel && core.state().pcTranscribes

    /** Paired and set to use the PC's model, so it can write up what you say once it's reached. */
    fun pcMayTranscribe(): Boolean = usePcModel && state.value.paired

    /**
     * Something can turn speech into text: the PC, or the model on this
     * device (loaded, or put away while unused and loaded again for it).
     */
    fun canTranscribe(): Boolean = pcWillTranscribe() || model.value is ModelStatus.Loaded ||
        (model.value is ModelStatus.Downloaded && modelFile().exists())

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
        val pcm = pcm16(samples)
        val seconds = "%.1f".format(samples.size / 16_000f)
        val started = System.currentTimeMillis()
        fun took() = "%.1f".format((System.currentTimeMillis() - started) / 1000f)
        if (pcWillTranscribe()) {
            try {
                return core.transcribeOnPc(pcm, app ?: "").also {
                    onLine("${state.value.pcName ?: "The PC"} wrote up $seconds s in ${took()} s" + if (it.isBlank()) ", and heard no words" else "")
                }
            } catch (e: Exception) {
                if (!modelFile().exists()) throw e
                onLine("The PC couldn't transcribe (${e.message}); used this device's model")
            }
        }
        val heard = synchronized(modelLock) {
            ensureModel()
            try { core.transcribe(pcm, selectedModel.language()) } finally { idleUnload() }
        }
        onLine("${selectedModel.name} wrote up $seconds s in ${took()} s" + if (heard.isBlank()) ", and heard no words" else "")
        // The PC's text comes finished; this device's is finished here, for the app.
        if (app == null || heard.isBlank()) return heard
        return runCatching { core.finishDictation(heard, app) }.getOrElse { e ->
            onLine("Couldn't apply the Dictionary and Style: ${e.message}")
            heard
        }
    }

    private fun modelsDir(): File = File(context.filesDir, "models").apply { mkdirs() }

    /** The model this device writes up with when the PC doesn't. */
    val selectedModel: PhoneModel get() = PhoneModels.byId(prefs.getString(PREF_MODEL, null))

    fun modelFile(m: PhoneModel = selectedModel): File = File(modelsDir(), m.file)

    fun isDownloaded(m: PhoneModel): Boolean = modelFile(m).exists()

    /**
     * Makes `m` the model this device uses. The one in memory goes; `m`
     * loads when you next talk, or downloads now if it isn't here yet.
     */
    fun selectModel(m: PhoneModel) {
        if (m.id == selectedModel.id || model.value is ModelStatus.Downloading) return
        prefs.edit().putString(PREF_MODEL, m.id).apply()
        scope.launch {
            synchronized(modelLock) { if (core.modelLoaded()) core.unloadModel() }
            model.value = if (isDownloaded(m)) ModelStatus.Downloaded else ModelStatus.Missing
            if (isDownloaded(m)) loadModel() else fetchModel()
        }
    }

    /** Deletes a model that isn't the one in use, for the space. */
    fun deleteModel(m: PhoneModel) {
        if (m.id == selectedModel.id) return
        modelFile(m).delete()
        bump()
    }

    /** Downloads the chosen model once, then loads it. Progress goes to [model]. */
    fun fetchModel() {
        if (model.value is ModelStatus.Downloading) return
        val m = selectedModel
        scope.launch {
            try {
                val file = modelFile(m)
                if (!file.exists()) download(m, file)
                if (m.id != selectedModel.id) return@launch
                model.value = ModelStatus.Downloaded
                loadModel()
            } catch (e: Exception) {
                model.value = ModelStatus.Missing
                onLine("Model download failed: ${e.message}")
                notice("${m.name} didn't download: ${e.message}")
            }
        }
    }

    fun loadModel() {
        scope.launch {
            try {
                synchronized(modelLock) { ensureModel() }
                idleUnload()
            } catch (e: Exception) {
                onLine(e.message ?: e.toString())
            }
        }
    }

    private val modelLock = Any()
    private var unloadJob: kotlinx.coroutines.Job? = null

    /** Loads the model if it isn't in memory. Blocks; hold [modelLock]. */
    private fun ensureModel() {
        if (core.modelLoaded()) return
        if (!modelFile().exists()) error("No speech model on this $device yet")
        val arch = core.loadModel(modelFile().absolutePath)
        model.value = ModelStatus.Loaded(arch)
    }

    /**
     * Android is short of memory: the model goes now rather than in five
     * minutes, as soon as no dictation is using it. Leaving Legato's screens
     * alone (the bubble may be tapped any second) keeps it.
     */
    fun trimMemory(level: Int) {
        val short = level == android.content.ComponentCallbacks2.TRIM_MEMORY_RUNNING_LOW ||
            level == android.content.ComponentCallbacks2.TRIM_MEMORY_RUNNING_CRITICAL ||
            level >= android.content.ComponentCallbacks2.TRIM_MEMORY_BACKGROUND
        if (!short) return
        scope.launch {
            synchronized(modelLock) {
                if (core.modelLoaded()) {
                    core.unloadModel()
                    model.value = ModelStatus.Downloaded
                    onLine("Android is short of memory: let go of the speech model")
                }
            }
        }
    }

    /**
     * Lets go of the model after five minutes unused: it holds hundreds of
     * megabytes, and loading it again takes a second or two.
     */
    private fun idleUnload() {
        unloadJob?.cancel()
        unloadJob = scope.launch {
            delay(IDLE_UNLOAD_MS)
            synchronized(modelLock) {
                if (core.modelLoaded()) {
                    core.unloadModel()
                    model.value = ModelStatus.Downloaded
                }
            }
        }
    }

    /**
     * Downloads `m` from its pinned revision over HTTPS and keeps it only if
     * its SHA-256 matches the catalog's: a changed or damaged file never loads.
     */
    private suspend fun download(m: PhoneModel, file: File) = withContext(Dispatchers.IO) {
        val tmp = File(file.path + ".part")
        val conn = (URL(m.url).openConnection() as HttpURLConnection).apply {
            instanceFollowRedirects = true
            connectTimeout = 15_000
            readTimeout = 30_000
        }
        try {
            if (conn.responseCode !in 200..299) error("HTTP ${conn.responseCode}")
            val total = conn.contentLengthLong.takeIf { it > 0 } ?: m.bytes
            val digest = java.security.MessageDigest.getInstance("SHA-256")
            conn.inputStream.use { input ->
                tmp.outputStream().use { out ->
                    val buf = ByteArray(256 * 1024)
                    var done = 0L
                    var shown = -1
                    while (true) {
                        val n = input.read(buf)
                        if (n < 0) break
                        out.write(buf, 0, n)
                        digest.update(buf, 0, n)
                        done += n
                        val percent = (done * 100 / total).toInt().coerceAtMost(100)
                        if (percent != shown) { shown = percent; model.value = ModelStatus.Downloading(percent) }
                    }
                }
            }
            val sum = digest.digest().joinToString("") { "%02x".format(it) }
            if (!sum.equals(m.sha256, ignoreCase = true)) error("the file didn't match its checksum, so it was thrown away")
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
        const val PREF_PC_MODEL = "pc_model"
        const val PREF_AFTER = "after_dictation_transform"
        private const val KEY_PREFIX = "ai:"
        const val PREF_MODEL = "speech_model"
        const val PREF_AUTO_CLIP = "auto_clipboard"
        const val PREF_KEEP_LISTENING = "keep_listening_without_keyboard"
        private const val PREF_CLIP_SENT = "clip_sent_copied_at"
        /** The label on clips that came from the PC, so they aren't sent back. */
        private const val PC_CLIP = "Legato from the PC"
        /** The label on clips the bubble writes, so they aren't sent to the PC. */
        const val DICTATION_CLIP = "Legato dictation"
        private const val IDLE_UNLOAD_MS = 5 * 60 * 1000L
        private const val LOG_LINES = 300

        /** A recording as signed 16-bit little-endian PCM, which crosses to the core in one copy. */
        fun pcm16(samples: FloatArray): ByteArray {
            val out = ByteArray(samples.size * 2)
            for (i in samples.indices) {
                val v = (samples[i].coerceIn(-1f, 1f) * 32767f).toInt()
                out[2 * i] = v.toByte()
                out[2 * i + 1] = (v shr 8).toByte()
            }
            return out
        }
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
