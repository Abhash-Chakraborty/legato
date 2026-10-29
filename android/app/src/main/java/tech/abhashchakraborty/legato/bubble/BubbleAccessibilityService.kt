package tech.abhashchakraborty.legato.bubble

import android.accessibilityservice.AccessibilityService
import android.content.ClipData
import android.content.ClipboardManager
import android.graphics.Rect
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.text.InputType
import android.util.Log
import android.view.accessibility.AccessibilityEvent
import android.view.accessibility.AccessibilityNodeInfo
import android.view.accessibility.AccessibilityWindowInfo
import tech.abhashchakraborty.legato.Legato
import tech.abhashchakraborty.legato.legato
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
    private val stopIfClosed = Runnable {
        val closed = windows.none { it.type == AccessibilityWindowInfo.TYPE_INPUT_METHOD }
        if (closed && Dictation.state.value is Dictation.State.Listening && Dictation.target == Dictation.Target.Field) {
            legato.onLine("The keyboard closed, so the dictation stopped")
            Dictation.finish(this)
        }
    }

    override fun onServiceConnected() {
        super.onServiceConnected()
        Log.i(TAG, "Connected")
        instance = this
        running.value = true
        overlay = BubbleOverlay(this)
        refresh()
    }

    override fun onAccessibilityEvent(event: AccessibilityEvent) {
        when (event.eventType) {
            AccessibilityEvent.TYPE_VIEW_TEXT_SELECTION_CHANGED -> noteSelection(event)
            // The chip's views fill in a moment after its window opens.
            AccessibilityEvent.TYPE_WINDOW_STATE_CHANGED -> if (event.packageName == SYSTEM_UI) {
                chipLooks = 0
                main.removeCallbacks(lookForChip)
                main.postDelayed(lookForChip, 150)
            }
        }
        // The cursor moves with every key typed. With the bubble already up
        // the field is known, so skip the search (a walk through the page
        // in Chrome) and spare the battery; focus and window changes still look.
        if (event.eventType == AccessibilityEvent.TYPE_VIEW_TEXT_SELECTION_CHANGED && overlay?.shown == true) return
        // Windows settle a moment after the event; look once they have.
        main.removeCallbacks(recheck)
        main.postDelayed(recheck, 120)
    }

    override fun onInterrupt() {}

    override fun onDestroy() {
        Log.i(TAG, "Destroyed")
        overlay?.hide(force = true)
        overlay = null
        if (instance === this) instance = null
        running.value = false
        super.onDestroy()
    }

    // ---- copies, for the shared clipboard ---------------------------------

    /**
     * The text last selected, kept only in memory to tell what a copy took,
     * and when the selection went away: Copy itself collapses it, so the
     * first collapse keeps it a moment; anything after forgets it.
     */
    private var selected: String? = null
    private var selectedAt = 0L
    private var collapsedAt = 0L
    private var copiedAt = 0L

    /** Follows the selection; password fields are skipped. */
    private fun noteSelection(event: AccessibilityEvent) {
        if (event.isPassword) return
        val from = minOf(event.fromIndex, event.toIndex)
        val to = maxOf(event.fromIndex, event.toIndex)
        val text = event.text.firstOrNull()
        if (from >= 0 && to > from && text != null && to <= text.length) {
            selected = text.subSequence(from, to).toString()
            selectedAt = SystemClock.uptimeMillis()
            collapsedAt = 0L
        } else if (selected != null) {
            if (collapsedAt == 0L) collapsedAt = SystemClock.uptimeMillis() else selected = null
        }
    }

    /**
     * The chip's views can take a moment to appear, so it's looked for a few
     * times. Only its appearing counts: while it stays up, a new selection's
     * menu isn't another copy. Once up, it's watched until it goes.
     */
    private var chipLooks = 0
    private var chipUp = false
    private val lookForChip: Runnable = object : Runnable {
        override fun run() {
            val shown = clipboardChipShown()
            if (shown && !chipUp) copied()
            chipUp = shown
            when {
                shown -> main.postDelayed(this, 1000)
                ++chipLooks < 4 -> main.postDelayed(this, 250)
            }
        }
    }

    /**
     * Whether Android's "copied" chip (Android 13+), which opens on every
     * copy, is on screen. Its window has no title, so it's known by its view.
     */
    private fun clipboardChipShown(): Boolean = windows.any { w ->
        w.type == AccessibilityWindowInfo.TYPE_SYSTEM &&
            w.root?.findAccessibilityNodeInfosByViewId("$SYSTEM_UI:id/clipboard_ui")?.isNotEmpty() == true
    }

    /**
     * Something was copied in another app: the selection made just before
     * is what was copied (Copy in the text menu). An older one may belong to
     * something else, so then the clipboard is read instead.
     */
    private fun copied() {
        val now = SystemClock.uptimeMillis()
        if (now - copiedAt < 1500) return
        copiedAt = now
        // Still selected, or collapsed just now by the copy.
        val text = selected?.takeIf { now - selectedAt < 120_000 && (collapsedAt == 0L || now - collapsedAt < 3000) }
        selected = null
        val keyboard = windows.any { it.type == AccessibilityWindowInfo.TYPE_INPUT_METHOD }
        Log.i(TAG, "Noticed a copy (${if (text != null) "the selection" else "nothing selected"}, keyboard ${if (keyboard) "open" else "closed"})")
        legato.copiedElsewhere(text, keyboard)
    }

    /** Shows the bubble when a keyboard is open over a field it may type into. */
    private fun refresh() {
        val keyboard = windows.firstOrNull { it.type == AccessibilityWindowInfo.TYPE_INPUT_METHOD }
        val field = if (keyboard != null) focusedField() else null
        val allowed = field != null && !field.password && !field.numeric && field.app !in Excluded.packages
        if (allowed) {
            val top = Rect().also { keyboard?.getBoundsInScreen(it) }.top
            // A dictation may follow: the PC gets a head start if it's out of reach.
            if (overlay?.shown != true) legato.wakeLink()
            overlay?.show(top.takeIf { it > 0 })
        } else {
            // The keyboard closed mid-dictation: it listens on, unless set to
            // stop. Moving between fields closes it for a moment, so look again.
            if (keyboard == null && !legato.listenThroughKeyboardClose) {
                main.removeCallbacks(stopIfClosed)
                main.postDelayed(stopIfClosed, 600)
            }
            overlay?.hide()
        }
    }

    private data class Field(val app: String, val password: Boolean, val numeric: Boolean)

    /**
     * The field with the cursor. An input-focus search answers first, but
     * Compose apps (ChatGPT) and search screens often give a container that
     * isn't marked editable, and web content (Chrome) doesn't always answer
     * at all, so the windows on screen are searched for a focused editable
     * node too: the active one first, then the rest (a search page can sit
     * in a window of its own).
     */
    private fun inputNode(): AccessibilityNodeInfo? {
        val focus = findFocus(AccessibilityNodeInfo.FOCUS_INPUT)
        if (focus != null && focus.isEditable) return focus
        focus?.let { search(it) }?.let { return it }
        val roots = buildList {
            rootInActiveWindow?.let { add(it) }
            windows.filter { it.type == AccessibilityWindowInfo.TYPE_APPLICATION && !it.isActive }
                .sortedByDescending { it.isFocused }
                .forEach { w -> w.root?.let { add(it) } }
        }
        for (root in roots) search(root)?.let { return it }
        return null
    }

    /** A focused editable node at or under `root`. */
    private fun search(root: AccessibilityNodeInfo): AccessibilityNodeInfo? {
        val queue = ArrayDeque(listOf(root))
        var seen = 0
        while (queue.isNotEmpty() && seen < 3000) {
            val node = queue.removeFirst()
            seen++
            if (node.isFocused && node.isEditable) return node
            for (i in 0 until node.childCount) node.getChild(i)?.let { queue.addLast(it) }
        }
        return null
    }

    private fun focusedField(): Field? {
        val node = inputNode() ?: return editorField()
        try {
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
     * The field the keyboard is typing into, as the keyboard sees it (Android
     * 13+). Some apps hide their fields from accessibility altogether; the
     * keyboard always knows the app and the kind of field, and the bubble can
     * type through the same connection.
     */
    private fun editorField(): Field? {
        if (Build.VERSION.SDK_INT < 33) return null
        val method = inputMethod ?: return null
        if (!method.currentInputStarted) return null
        val info = method.currentInputEditorInfo ?: return null
        val type = info.inputType
        val kind = type and InputType.TYPE_MASK_CLASS
        val variation = type and InputType.TYPE_MASK_VARIATION
        val password = (kind == InputType.TYPE_CLASS_TEXT && variation in PASSWORD_VARIATIONS) ||
            (kind == InputType.TYPE_CLASS_NUMBER && variation == InputType.TYPE_NUMBER_VARIATION_PASSWORD)
        // A keyboard open with no text field behind it (TYPE_NULL) has nothing to type into.
        if (type == InputType.TYPE_NULL) return null
        val numeric = kind == InputType.TYPE_CLASS_NUMBER || kind == InputType.TYPE_CLASS_PHONE
        return Field(info.packageName ?: "", password, numeric)
    }

    /** The app of the focused field, for Style's kind of app. */
    fun focusedApp(): String? = inputNode()?.packageName?.toString() ?: editorField()?.app

    /**
     * Puts `text` at the cursor of the focused field (replacing a selection),
     * with a space before it when the text before the cursor needs one.
     * Falls back to a paste through the clipboard when the field refuses.
     */
    fun insert(text: String): Boolean {
        val node = inputNode() ?: return typeThroughKeyboard(text)
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
     * Types into a field accessibility can't see, through the keyboard's
     * connection (Android 13+). The text before the cursor decides the space.
     */
    private fun typeThroughKeyboard(text: String): Boolean {
        if (Build.VERSION.SDK_INT < 33) return false.also { Log.w(TAG, "No field has the cursor") }
        val method = inputMethod
        val connection = method?.currentInputConnection
        if (method == null || connection == null || !method.currentInputStarted) {
            return false.also { Log.w(TAG, "No field has the cursor and no keyboard connection is open") }
        }
        val around = connection.getSurroundingText(1, 0, 0)
        val before = around?.let { it.text.subSequence(0, it.selectionStart.coerceIn(0, it.text.length)) }?.toString().orEmpty()
        val lead = if (before.isNotEmpty() && !before.last().isWhitespace()) " " else ""
        connection.commitText(lead + text, 1, null)
        Log.i(TAG, "Typed through the keyboard connection (${method.currentInputEditorInfo?.packageName})")
        return true
    }

    /**
     * Replaces `original` in the focused field with `replacement`, for a
     * transform on selected text whose app dropped the result (Chrome does
     * once the selection is gone). The occurrence around the cursor wins,
     * else the last one. False when the field no longer holds `original`.
     */
    fun replaceText(original: String, replacement: String): Boolean {
        val node = inputNode() ?: return false
        node.refresh()
        if (!node.isEditable) return false
        val current = typedText(node)
        // The app took the result itself.
        if (replacement in current) return false
        val caret = node.textSelectionEnd
        var at = current.indexOf(original)
        var chosen = -1
        while (at >= 0) {
            chosen = at
            if (caret in at..at + original.length) break
            at = current.indexOf(original, at + 1)
        }
        if (chosen < 0) return false
        val args = Bundle().apply {
            putCharSequence(
                AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE,
                current.substring(0, chosen) + replacement + current.substring(chosen + original.length),
            )
        }
        if (node.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, args)) return true
        // Web views refuse set-text: select the old text through the keyboard's connection and type over it.
        if (Build.VERSION.SDK_INT >= 33) {
            val connection = inputMethod?.currentInputConnection ?: return false
            connection.setSelection(chosen, chosen + original.length)
            connection.commitText(replacement, 1, null)
            return true
        }
        return false
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
        private const val SYSTEM_UI = "com.android.systemui"
        @Volatile var instance: BubbleAccessibilityService? = null
            private set

        /** Text field kinds that hold a password. */
        private val PASSWORD_VARIATIONS = setOf(
            InputType.TYPE_TEXT_VARIATION_PASSWORD,
            InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD,
            InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD,
        )

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
