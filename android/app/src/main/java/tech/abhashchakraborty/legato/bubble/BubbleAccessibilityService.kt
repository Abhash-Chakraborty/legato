package tech.abhashchakraborty.legato.bubble

import android.accessibilityservice.AccessibilityService
import android.content.ClipData
import android.content.ClipboardManager
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.text.InputType
import android.view.accessibility.AccessibilityEvent
import android.view.accessibility.AccessibilityNodeInfo
import android.view.accessibility.AccessibilityWindowInfo
import kotlinx.coroutines.flow.MutableStateFlow

/**
 * The eyes and hands of the bubble: notices when a keyboard is open over an
 * editable field, shows the bubble then, and types dictated text into the
 * field. It reads only the focused field, never the rest of the screen.
 */
class BubbleAccessibilityService : AccessibilityService() {
    private val main = Handler(Looper.getMainLooper())
    private var overlay: BubbleOverlay? = null
    private val recheck = Runnable { refresh() }

    override fun onServiceConnected() {
        super.onServiceConnected()
        instance = this
        running.value = true
        overlay = BubbleOverlay(this)
        refresh()
    }

    override fun onAccessibilityEvent(event: AccessibilityEvent) {
        // Windows settle a moment after the event; look once they have.
        main.removeCallbacks(recheck)
        main.postDelayed(recheck, 120)
    }

    override fun onInterrupt() {}

    override fun onDestroy() {
        overlay?.hide()
        overlay = null
        if (instance === this) instance = null
        running.value = false
        super.onDestroy()
    }

    /** Shows the bubble when a keyboard is open over a field it may type into. */
    private fun refresh() {
        val keyboard = windows.any { it.type == AccessibilityWindowInfo.TYPE_INPUT_METHOD }
        val field = if (keyboard) focusedField() else null
        val allowed = field != null && !field.password && !field.numeric && field.app !in Excluded.packages
        if (allowed) overlay?.show() else overlay?.hide()
    }

    private data class Field(val app: String, val password: Boolean, val numeric: Boolean)

    private fun focusedField(): Field? {
        val node = findFocus(AccessibilityNodeInfo.FOCUS_INPUT) ?: return null
        try {
            if (!node.isEditable) return null
            val numeric = Build.VERSION.SDK_INT >= Build.VERSION_CODES.R &&
                (node.inputType and InputType.TYPE_MASK_CLASS).let {
                    it == InputType.TYPE_CLASS_NUMBER || it == InputType.TYPE_CLASS_PHONE
                }
            return Field(node.packageName?.toString() ?: "", node.isPassword, numeric)
        } finally {
            if (Build.VERSION.SDK_INT < 33) @Suppress("DEPRECATION") node.recycle()
        }
    }

    /**
     * Puts `text` at the cursor of the focused field (replacing a selection),
     * with a space before it when the text before the cursor needs one.
     * Falls back to a paste through the clipboard when the field refuses.
     */
    fun insert(text: String): Boolean {
        val node = findFocus(AccessibilityNodeInfo.FOCUS_INPUT) ?: return false
        if (!node.isEditable) return false
        val current = node.text?.toString().orEmpty()
        val start = node.textSelectionStart.takeIf { it >= 0 } ?: current.length
        val end = node.textSelectionEnd.takeIf { it >= 0 } ?: start
        val before = current.substring(0, minOf(start, current.length))
        val after = current.substring(minOf(end, current.length))
        val lead = if (before.isNotEmpty() && !before.last().isWhitespace()) " " else ""
        val piece = lead + text
        val args = Bundle().apply {
            putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, before + piece + after)
        }
        if (node.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, args)) {
            val caret = before.length + piece.length
            node.performAction(
                AccessibilityNodeInfo.ACTION_SET_SELECTION,
                Bundle().apply {
                    putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, caret)
                    putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, caret)
                },
            )
            return true
        }
        // Some apps (rich editors, web views) ignore set-text; paste instead.
        val cm = getSystemService(ClipboardManager::class.java)
        val keep = cm.primaryClip
        cm.setPrimaryClip(ClipData.newPlainText("Legato", piece))
        val pasted = node.performAction(AccessibilityNodeInfo.ACTION_PASTE)
        main.postDelayed({ keep?.let { cm.setPrimaryClip(it) } }, 400)
        return pasted
    }

    companion object {
        @Volatile var instance: BubbleAccessibilityService? = null
            private set

        /** True while the user has the service switched on. */
        val running = MutableStateFlow(false)
    }
}

/** Apps the bubble stays out of, on top of password and number fields. */
object Excluded {
    val packages: Set<String> = setOf(
        // Banking and payments: never dictate into these.
        "com.google.android.apps.walletnfcrel",
        "com.paypal.android.p2pmobile",
        "de.comdirect.android",
        "com.db.pbc.mibanco",
        "de.dkb.portalapp",
        "de.ingdiba.bankingapp",
        "com.n26",
        "de.number26.android",
        "com.revolut.revolut",
        "de.commerzbanking.mobil",
        "de.postbank.finanzassistent",
        "com.starfinanz.smob.android.sfinanzstatus",
        // Password managers.
        "com.x8bit.bitwarden",
        "com.kunzisoft.keepass.free",
        "com.agilebits.onepassword",
    )
}
