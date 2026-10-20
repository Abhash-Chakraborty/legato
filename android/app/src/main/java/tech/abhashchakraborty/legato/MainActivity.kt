package tech.abhashchakraborty.legato

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import tech.abhashchakraborty.legato.ui.LegatoApp
import tech.abhashchakraborty.legato.ui.LegatoTheme

/** Opens Legato's screens, and hands them any pairing link it was opened with. */
class MainActivity : ComponentActivity() {
    private val recorder = Recorder()
    /** A pairing link this activity was opened with, now or later. */
    private var link by mutableStateOf<String?>(null)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        // Only a fresh start pairs; turning the screen recreates the activity
        // with the same intent, and the link has been used by then.
        if (savedInstanceState == null) link = pairingLink(intent)
        setContent {
            LegatoTheme {
                LegatoApp(recorder, link)
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        pairingLink(intent)?.let { link = it }
    }

    override fun onStart() {
        super.onStart()
        if (legato.state.value.paired) legato.run { connect() }
    }

    // Android hands an app the clipboard only once its window has focus.
    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus) legato.clipboardOnFocus()
    }

    private fun pairingLink(intent: Intent?): String? {
        val uri = intent?.data ?: return null
        return if (uri.scheme == "legato" && uri.host == "pair") uri.toString() else null
    }
}
