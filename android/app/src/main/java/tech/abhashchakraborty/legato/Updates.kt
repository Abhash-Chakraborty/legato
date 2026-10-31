package tech.abhashchakraborty.legato

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.Settings
import androidx.core.content.FileProvider
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
        data class Failed(val message: String) : State
    }

    private val _state = MutableStateFlow<State>(State.Idle)
    val state: StateFlow<State> = _state.asStateFlow()

    private const val MANIFEST = "https://github.com/Abhash-Chakraborty/legato/releases/latest/download/legato-android.json"
    private const val EVERY_MS = 6 * 60 * 60 * 1000L
    const val PREF_AUTO = "update_check"
    private const val PREF_LAST = "update_checked_at"
    private const val PREF_OFFERED = "update_offered"

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
        if (s is State.Checking || s is State.Downloading) return
        _state.value = State.Checking
        val legato = context.legato
        legato.scope.launch(Dispatchers.IO) {
            _state.value = try {
                val json = JSONObject(get(MANIFEST).decodeToString())
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
     * Hands the APK to Android's installer. The first time, Android asks to
     * let Legato install apps; that opens its page, and Install again
     * carries on from there.
     */
    fun install(context: Context) {
        val r = _state.value as? State.Ready ?: return
        if (!context.packageManager.canRequestPackageInstalls()) {
            context.startActivity(
                Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:${context.packageName}"))
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
            )
            return
        }
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.updates", r.file)
        context.startActivity(
            Intent(Intent.ACTION_VIEW)
                .setDataAndType(uri, "application/vnd.android.package-archive")
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK),
        )
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
