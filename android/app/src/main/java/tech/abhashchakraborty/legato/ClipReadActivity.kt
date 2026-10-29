package tech.abhashchakraborty.legato

import android.app.Activity
import android.os.Build

/**
 * Comes to the front for a moment, unseen, to read what was just copied in
 * another app: Android hands the clipboard only to the app with focus. It
 * sends the clip to the PC as Legato's own screens do, then goes.
 */
class ClipReadActivity : Activity() {
    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (!hasFocus) return
        legato.clipboardOnFocus()
        finish()
        if (Build.VERSION.SDK_INT >= 34) {
            overrideActivityTransition(OVERRIDE_TRANSITION_CLOSE, 0, 0)
        } else {
            @Suppress("DEPRECATION") overridePendingTransition(0, 0)
        }
    }
}
