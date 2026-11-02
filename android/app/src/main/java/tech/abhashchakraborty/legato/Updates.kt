package tech.abhashchakraborty.legato

import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageInstaller
import android.os.Build
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import org.json.JSONObject
import java.io.File
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest

/**
 * Updates from GitHub Releases, where each release carries the APK and
 * `legato-android.json` (its version, address and checksum). Checks when
 * Legato opens, at most every few hours, or from Settings › About; the APK
 * downloads into the cache, is checked against the release's checksum, and
 * Android's installer asks you to confirm. No server of Legato's own.
 */
object Updates {
    sealed interface State {
        data object Idle : State
        data object Checking : State
        data object UpToDate : State
        data class Available(val version: String, val code: Int, val apk: String, val sha256: String, val size: Long) : State
        data class Downloading(val version: String, val progress: Float) : State
        data class Ready(val version: String, val file: File) : State
        /** Copying into Android's installer, then waiting on its confirmation. */
        data class Installing(val version: String, val file: File) : State
        data class Failed(val message: String) : State
    }

    private val _state = MutableStateFlow<State>(State.Idle)
    val state: StateFlow<State> = _state.asStateFlow()

    private const val MANIFEST = "https://github.com/Abhash-Chakraborty/legato/releases/latest/download/legato-android.json"
    private const val EVERY_MS = 6 * 60 * 60 * 1000L
    const val PREF_AUTO = "update_check"
    private const val PREF_LAST = "update_checked_at"
    private const val PREF_OFFERED = "update_offered"

    /** Debug builds can check another address, to try an update before it's released. */
    private fun manifest(context: Context): String =
        if (BuildConfig.DEBUG) context.legato.prefs.getString("update_manifest", null) ?: MANIFEST else MANIFEST

    fun autoCheck(context: Context): Boolean = context.legato.prefs.getBoolean(PREF_AUTO, true)

    fun lastChecked(context: Context): Long = context.legato.prefs.getLong(PREF_LAST, 0L)

    /** On opening Legato: checks when automatic checks are on and the last was a while ago. */
    fun maybeCheck(context: Context) {
        if (!autoCheck(context)) return
        if (System.currentTimeMillis() - lastChecked(context) < EVERY_MS) return
        check(context)
    }

    fun check(context: Context) {
        val s = _state.value
        if (s is State.Checking || s is State.Downloading || s is State.Installing) return
        _state.value = State.Checking
        val legato = context.legato
        legato.scope.launch(Dispatchers.IO) {
            _state.value = try {
                val json = JSONObject(get(manifest(context)).decodeToString())
                legato.prefs.edit().putLong(PREF_LAST, System.currentTimeMillis()).apply()
                val code = json.getInt("versionCode")
                if (code <= BuildConfig.VERSION_CODE) State.UpToDate
                else State.Available(
                    version = json.getString("versionName"),
                    code = code,
                    apk = json.getString("apk"),
                    sha256 = json.getString("sha256").lowercase(),
                    size = json.optLong("size", 0L),
                )
            } catch (e: Exception) {
                State.Failed(if (e is java.io.FileNotFoundException) "No Android release yet" else "Couldn't check for updates (${e.message ?: "no connection"})")
            }
        }
    }

    /** Whether to offer [version] unasked: once per version, after an automatic check. */
    fun shouldOffer(context: Context, version: String): Boolean =
        context.legato.prefs.getString(PREF_OFFERED, null) != version

    fun offered(context: Context, version: String) {
        context.legato.prefs.edit().putString(PREF_OFFERED, version).apply()
    }

    fun download(context: Context) {
        val a = _state.value as? State.Available ?: return
        val legato = context.legato
        _state.value = State.Downloading(a.version, 0f)
        legato.scope.launch(Dispatchers.IO) {
            _state.value = try {
                val dir = File(context.cacheDir, "updates").apply { mkdirs() }
                dir.listFiles()?.forEach { it.delete() }
                val file = File(dir, "legato-${a.version}.apk")
                val digest = MessageDigest.getInstance("SHA-256")
                open(a.apk).use { conn ->
                    val total = conn.contentLengthLong.takeIf { it > 0 } ?: a.size
                    var done = 0L
                    conn.inputStream.use { input ->
                        file.outputStream().use { out ->
                            val buf = ByteArray(64 * 1024)
                            while (true) {
                                val n = input.read(buf)
                                if (n < 0) break
                                out.write(buf, 0, n)
                                digest.update(buf, 0, n)
                                done += n
                                if (total > 0) _state.value = State.Downloading(a.version, done.toFloat() / total)
                            }
                        }
                    }
                }
                val sum = digest.digest().joinToString("") { "%02x".format(it) }
                if (sum != a.sha256) {
                    file.delete()
                    error("the download didn't match the release; try again")
                }
                legato.onLine("Downloaded Legato ${a.version} for Android")
                State.Ready(a.version, file)
            } catch (e: Exception) {
                State.Failed("Couldn't download ${a.version}: ${e.message ?: "no connection"}")
            }
        }
    }

    /**
     * Hands the APK to Android's installer in a session, which asks you to
     * confirm (and, the first time, to let Legato install apps). An update
     * installed this way keeps its accessibility service on: Android
     * restricts apps opened from a downloaded file, and turned the bubble
     * off after an update that went that way.
     */
    fun install(context: Context) {
        val r = _state.value as? State.Ready ?: return
        val legato = context.legato
        // Copying a large APK takes a moment; a second tap would open a second confirmation.
        _state.value = State.Installing(r.version, r.file)
        legato.scope.launch(Dispatchers.IO) {
            try {
                val installer = context.packageManager.packageInstaller
                val params = PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL).apply {
                    setAppPackageName(context.packageName)
                    setSize(r.file.length())
                    // Once Legato installed itself, later updates need no second confirmation.
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) setRequireUserAction(PackageInstaller.SessionParams.USER_ACTION_NOT_REQUIRED)
                }
                val id = installer.createSession(params)
                installer.openSession(id).use { session ->
                    session.openWrite("legato.apk", 0, r.file.length()).use { out ->
                        r.file.inputStream().use { it.copyTo(out) }
                        session.fsync(out)
                    }
                    val flags = PendingIntent.FLAG_UPDATE_CURRENT or
                        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) PendingIntent.FLAG_MUTABLE else 0
                    val done = PendingIntent.getBroadcast(context, id, Intent(context, UpdateReceiver::class.java), flags)
                    session.commit(done.intentSender)
                }
                legato.onLine("Handed Legato ${r.version} to Android's installer")
            } catch (e: Exception) {
                _state.value = State.Failed("Couldn't install ${r.version}: ${e.message ?: "the installer refused it"}")
            }
        }
    }

    /** What the installer said about the session [install] committed. */
    internal fun installerSaid(context: Context, intent: Intent) {
        when (val status = intent.getIntExtra(PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE)) {
            PackageInstaller.STATUS_PENDING_USER_ACTION -> {
                @Suppress("DEPRECATION")
                val confirm = intent.getParcelableExtra<Intent>(Intent.EXTRA_INTENT) ?: return
                context.startActivity(confirm.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
            }
            // Android closes Legato to replace it; the bubble's service comes back on its own.
            PackageInstaller.STATUS_SUCCESS -> {}
            // Cancelled in the confirmation: Install stays there for later.
            PackageInstaller.STATUS_FAILURE_ABORTED -> {
                val s = _state.value
                if (s is State.Installing) _state.value = State.Ready(s.version, s.file)
            }
            else -> {
                val why = intent.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE) ?: "status $status"
                context.legato.onLine("The update didn't install: $why")
                _state.value = State.Failed("The update didn't install ($why)")
            }
        }
    }

    private class Conn(val c: HttpURLConnection) : AutoCloseable {
        val contentLengthLong get() = c.contentLengthLong
        val inputStream get() = c.inputStream
        override fun close() = c.disconnect()
    }

    /** GitHub answers release downloads with a redirect to its file host. */
    private fun open(address: String): Conn {
        var url = URL(address)
        repeat(5) {
            val c = (url.openConnection() as HttpURLConnection).apply {
                instanceFollowRedirects = false
                connectTimeout = 15_000
                readTimeout = 30_000
                setRequestProperty("User-Agent", "Legato-Android/${BuildConfig.VERSION_NAME}")
            }
            when (c.responseCode) {
                in 300..399 -> {
                    val next = c.getHeaderField("Location") ?: error("redirect without an address")
                    c.disconnect()
                    url = URL(url, next)
                    if (url.protocol != "https") error("not https")
                }
                HttpURLConnection.HTTP_NOT_FOUND -> { c.disconnect(); throw java.io.FileNotFoundException(address) }
                in 200..299 -> return Conn(c)
                else -> { val code = c.responseCode; c.disconnect(); error("HTTP $code") }
            }
        }
        error("too many redirects")
    }

    private fun get(address: String): ByteArray = open(address).use { it.inputStream.use { s -> s.readBytes() } }
}

/** Receives the installer's answers for [Updates.install]. */
class UpdateReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) = Updates.installerSaid(context, intent)
}
