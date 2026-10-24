package tech.abhashchakraborty.legato.bubble

import android.accessibilityservice.AccessibilityService
import android.content.ClipData
import android.content.ClipboardManager
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.text.InputType
import android.util.Log
import android.view.accessibility.AccessibilityEvent
import android.view.accessibility.AccessibilityNodeInfo
import android.view.accessibility.AccessibilityWindowInfo
import tech.abhashchakraborty.legato.Legato
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
        Log.i(TAG, "Connected")
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
        Log.i(TAG, "Destroyed")
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

    /**
     * The field with the cursor. Web content (Chrome) doesn't always answer
     * an input-focus search, so the window in front is searched for its
     * focused editable node as well.
     */
    private fun inputNode(): AccessibilityNodeInfo? {
        findFocus(AccessibilityNodeInfo.FOCUS_INPUT)?.let { return it }
        val root = rootInActiveWindow ?: return null
        val queue = ArrayDeque(listOf(root))
        var seen = 0
        while (queue.isNotEmpty() && seen < 2000) {
            val node = queue.removeFirst()
            seen++
            if (node.isFocused && node.isEditable) return node
            for (i in 0 until node.childCount) node.getChild(i)?.let { queue.addLast(it) }
        }
        return null
    }

    private fun focusedField(): Field? {
        val node = inputNode() ?: return null
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

    /** The app of the focused field, for Style's kind of app. */
    fun focusedApp(): String? = inputNode()?.packageName?.toString()

    /**
     * Puts `text` at the cursor of the focused field (replacing a selection),
     * with a space before it when the text before the cursor needs one.
     * Falls back to a paste through the clipboard when the field refuses.
     */
    fun insert(text: String): Boolean {
        val node = inputNode() ?: return false.also { Log.w(TAG, "No field has the cursor") }
        // The cached node can be a moment old; ask the app for it afresh.
        node.refresh()
        if (!node.isEditable) return false.also { Log.w(TAG, "The focused node isn't editable (${node.packageName})") }
        val current = typedText(node)
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
        // Web views (Chrome) refuse set-text. From Android 13 the service can
        // type through the keyboard's connection to the field, as a keyboard does.
        if (Build.VERSION.SDK_INT >= 33) {
            val connection = inputMethod?.currentInputConnection
            if (connection != null) {
                connection.commitText(piece, 1, null)
                return true
            }
            Log.w(TAG, "${node.packageName} refused set-text and no keyboard connection is open")
        }
        // Older Android: paste through the clipboard instead.
        val cm = getSystemService(ClipboardManager::class.java)
        // In the background this reads as nothing (Android 10+), so the
        // dictation is cleared afterwards rather than left on the clipboard.
        val keep = cm.primaryClip
        cm.setPrimaryClip(ClipData.newPlainText(Legato.DICTATION_CLIP, piece))
        val pasted = node.performAction(AccessibilityNodeInfo.ACTION_PASTE)
        if (pasted) {
            main.postDelayed({ if (keep != null) cm.setPrimaryClip(keep) else cm.clearPrimaryClip() }, 400)
        } else {
            Log.w(TAG, "${node.packageName} refused both set-text and paste")
        }
        return pasted
    }

    /**
     * What's typed in the field. An empty field reports its grey hint ("Ask
     * Google about this…") as its text, which would end up in front of the
     * dictation, so a hint counts as nothing typed.
     */
    private fun typedText(node: AccessibilityNodeInfo): String {
        val text = node.text?.toString().orEmpty()
        if (node.isShowingHintText) return ""
        val hint = node.hintText?.toString()
        return if (!hint.isNullOrEmpty() && text == hint) "" else text
    }

    companion object {
        const val TAG = "LegatoBubble"
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
