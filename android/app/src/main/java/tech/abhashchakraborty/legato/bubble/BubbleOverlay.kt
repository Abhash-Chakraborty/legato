package tech.abhashchakraborty.legato.bubble

import android.animation.ValueAnimator
import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.Path
import android.graphics.PixelFormat
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import android.util.TypedValue
import android.view.Gravity
import android.view.HapticFeedbackConstants
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import android.view.WindowManager
import android.widget.FrameLayout
import android.widget.ImageButton
import android.widget.ImageView
import android.widget.LinearLayout
import android.view.animation.DecelerateInterpolator
import tech.abhashchakraborty.legato.R
import kotlin.math.abs
import kotlin.math.hypot
import kotlin.math.sin

/**
 * The Legato bubble over the keyboard: a 40 dp teal circle that fades to a
 * small dot when left alone, a slim bar with Cancel, the level and a check
 * while listening. Drag it along either edge; drop it at the bottom to hide
 * it for 10 minutes. Sizes are dp, so it looks the same on any phone.
 */
class BubbleOverlay(private val context: Context) {
    private val wm = context.getSystemService(WindowManager::class.java)
    private val main = Handler(Looper.getMainLooper())
    private val density = context.resources.displayMetrics.density
    private fun dp(v: Float) = (v * density)
    private fun dpi(v: Float) = (v * density).toInt()

    private var root: FrameLayout? = null
    private var params: WindowManager.LayoutParams? = null
    private val bubble = ImageView(context)
    private val bar = LinearLayout(context)
    private val wave = WaveView(context)
    private var snoozedUntil = 0L
    /** Where the bubble rests on its edge; the bar opens from here and the dot returns here. */
    private var dockX = 0
    private var dotted = false
    private var listening = false
    private val toDot = Runnable { if (!listening && !dragging) setDot(true) }
    private var dragging = false

    private val onState: (Dictation.State) -> Unit = { s -> main.post { render(s) } }

    val shown: Boolean get() = root != null

    /**
     * Shows the bubble at its place on the edge, kept above the keyboard
     * when `keyboardTop` (in screen pixels) is known: a landscape tablet's
     * keyboard reaches higher than the default place. A shown bubble moves
     * up when the keyboard grows.
     */
    fun show(keyboardTop: Int? = null) {
        if (System.currentTimeMillis() < snoozedUntil) return
        hideWhenIdle = false
        root?.let { frame ->
            val lp = params ?: return
            if (dragging || listening) return
            // Turned to landscape or back: back to its edge, above the keyboard.
            val x = Position.x(context, dpi(40f))
            val y = above(lp.y, keyboardTop)
            if (x != dockX || y != lp.y) {
                dockX = x
                lp.x = x
                lp.y = y
                runCatching { wm.updateViewLayout(frame, lp) }
            }
            return
        }
        val type = if (Settings.canDrawOverlays(context)) {
            WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY
        } else {
            WindowManager.LayoutParams.TYPE_ACCESSIBILITY_OVERLAY
        }
        val lp = WindowManager.LayoutParams(
            ViewGroup.LayoutParams.WRAP_CONTENT,
            ViewGroup.LayoutParams.WRAP_CONTENT,
            type,
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or
                WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN or
                WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL,
            PixelFormat.TRANSLUCENT,
        ).apply {
            gravity = Gravity.TOP or Gravity.START
            x = Position.x(context, dpi(40f))
            y = above(Position.y(context), keyboardTop)
            noMoveAnimation()
        }
        dockX = lp.x
        // The bubble, bar and wave are kept between shows; free them from the last window.
        (bubble.parent as? ViewGroup)?.removeView(bubble)
        (bar.parent as? ViewGroup)?.removeView(bar)
        bar.removeAllViews()
        val frame = FrameLayout(context)
        buildBubble(frame)
        buildBar(frame)
        frame.alpha = 0f
        frame.scaleX = 0.4f
        frame.scaleY = 0.4f
        wm.addView(frame, lp)
        root = frame
        params = lp
        frame.animate().alpha(1f).scaleX(1f).scaleY(1f).setDuration(220).setInterpolator(easeOut).start()
        Dictation.listeners.add(onState)
        render(Dictation.state.value)
        armDot()
    }

    /** `y`, moved up if the bubble or its bar would cover the keyboard. */
    private fun above(y: Int, keyboardTop: Int?): Int {
        val top = keyboardTop ?: return y
        return y.coerceAtMost(top - dpi(56f)).coerceAtLeast(dpi(80f))
    }

    /**
     * Hides the bubble once nothing is going on. While you talk, or while
     * the text is written up, it stays where it is: switching apps, pulling
     * down the notification shade or the screen timing out closes the
     * keyboard, and none of that should stop a dictation. It goes when the
     * dictation ends, unless a keyboard is open by then.
     */
    fun hide(force: Boolean = false) {
        val frame = root ?: return
        if (!force && Dictation.state.value !is Dictation.State.Idle) {
            hideWhenIdle = true
            return
        }
        hideWhenIdle = false
        Dictation.listeners.remove(onState)
        main.removeCallbacks(toDot)
        runCatching { wm.removeView(frame) }
        root = null
        params = null
        listening = false
        dotted = false
    }

    /** A keyboard closed during the dictation: hide once it's over. */
    private var hideWhenIdle = false

    private fun armDot() {
        main.removeCallbacks(toDot)
        main.postDelayed(toDot, 2000)
    }

    private fun setDot(on: Boolean) {
        if (dotted == on || listening) return
        dotted = on
        val size = if (on) dp(14f) else dp(40f)
        val lp = bubble.layoutParams
        ValueAnimator.ofInt(lp.width, size.toInt()).apply {
            duration = 320
            addUpdateListener {
                lp.width = it.animatedValue as Int
                lp.height = it.animatedValue as Int
                bubble.layoutParams = lp
            }
            start()
        }
        bubble.animate().alpha(if (on) 0.5f else 0.72f).setDuration(320).start()
        bubble.setImageResource(if (on) 0 else R.drawable.ic_bubble)
        bubble.contentDescription = context.getString(R.string.bubble_tap)
    }

    private fun buildBubble(frame: FrameLayout) {
        bubble.setImageResource(R.drawable.ic_bubble)
        bubble.setBackgroundResource(R.drawable.bubble_bg)
        bubble.scaleType = ImageView.ScaleType.CENTER
        bubble.alpha = 0.72f
        bubble.contentDescription = context.getString(R.string.bubble_tap)
        // A 48 dp touch target around a 40 dp mark; the dot keeps the same target.
        val pad = dpi(4f)
        val wrap = FrameLayout(context).apply { setPadding(pad, pad, pad, pad) }
        wrap.addView(bubble, FrameLayout.LayoutParams(dpi(40f), dpi(40f), Gravity.CENTER))
        wrap.minimumWidth = dpi(48f)
        wrap.minimumHeight = dpi(48f)
        frame.addView(wrap, FrameLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT))
        wrap.setOnTouchListener(DragTapListener(wrap))
    }

    private fun buildBar(frame: FrameLayout) {
        bar.orientation = LinearLayout.HORIZONTAL
        bar.gravity = Gravity.CENTER_VERTICAL
        bar.setBackgroundResource(R.drawable.bar_bg)
        bar.alpha = 0.86f
        bar.visibility = View.GONE
        bar.setPadding(dpi(4f), 0, dpi(4f), 0)
        val cancel = ImageButton(context).apply {
            setImageResource(R.drawable.ic_close)
            setBackgroundResource(R.drawable.bar_button_bg)
            contentDescription = context.getString(R.string.bubble_cancel)
            setOnClickListener { Dictation.cancel(context) }
        }
        val check = ImageButton(context).apply {
            setImageResource(R.drawable.ic_check)
            setBackgroundResource(R.drawable.bar_check_bg)
            contentDescription = context.getString(R.string.bubble_finish)
            setOnClickListener { Dictation.finish(context) }
        }
        val size = dpi(36f)
        bar.addView(cancel, LinearLayout.LayoutParams(size, size))
        bar.addView(wave, LinearLayout.LayoutParams(dpi(72f), dpi(24f)).apply { marginStart = dpi(6f); marginEnd = dpi(6f) })
        bar.addView(check, LinearLayout.LayoutParams(size, size))
        frame.addView(bar, FrameLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, dpi(40f)))
    }

    private fun render(state: Dictation.State) {
        val frame = root ?: return
        val nowListening = state is Dictation.State.Listening || state is Dictation.State.Transcribing
        if (nowListening != listening) {
            listening = nowListening
            if (listening) {
                main.removeCallbacks(toDot)
                dotted = false
                // Place the bar before it shows, in one layout: it opens from the
                // bubble towards the middle of the screen and never off the edge.
                bar.measure(View.MeasureSpec.UNSPECIFIED, View.MeasureSpec.UNSPECIFIED)
                val barW = bar.measuredWidth
                val dotW = frame.getChildAt(0).width.takeIf { it > 0 } ?: dpi(48f)
                val screen = context.resources.displayMetrics.widthPixels
                val onLeft = dockX + dotW / 2 < screen / 2
                params?.let { lp ->
                    lp.x = (if (onLeft) dockX else dockX + dotW - barW).coerceIn(dpi(8f), screen - barW - dpi(8f))
                    runCatching { wm.updateViewLayout(frame, lp) }
                }
                frame.getChildAt(0).visibility = View.GONE
                bar.visibility = View.VISIBLE
                bar.pivotX = if (onLeft) 0f else barW.toFloat()
                bar.scaleX = 0.3f
                bar.animate().scaleX(1f).setDuration(220).setInterpolator(easeOut).start()
            } else {
                bar.visibility = View.GONE
                frame.getChildAt(0).visibility = View.VISIBLE
                bubble.alpha = 0.72f
                bubble.layoutParams = bubble.layoutParams.apply { width = dpi(40f); height = dpi(40f) }
                bubble.setImageResource(R.drawable.ic_bubble)
                // Back to the edge it came from.
                params?.let { lp ->
                    lp.x = dockX
                    runCatching { wm.updateViewLayout(frame, lp) }
                }
                armDot()
            }
        }
        if (state is Dictation.State.Listening) wave.level = state.level
        if (state is Dictation.State.Transcribing) wave.busy = true else wave.busy = false
        keepScreenOn(state is Dictation.State.Listening)
        if (state is Dictation.State.Idle && hideWhenIdle) hide()
    }

    /**
     * Holds the screen on while you talk: a paragraph takes longer than most
     * screen timeouts, and nobody touches the phone while speaking.
     */
    private fun keepScreenOn(on: Boolean) {
        val frame = root ?: return
        val lp = params ?: return
        val flag = WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON
        if ((lp.flags and flag != 0) == on) return
        lp.flags = if (on) lp.flags or flag else lp.flags and flag.inv()
        runCatching { wm.updateViewLayout(frame, lp) }
    }

    private val easeOut = DecelerateInterpolator(2f)

    /**
     * Android slides an overlay to each new position over about 300 ms, so the
     * bar opened late and the dot crawled back to its edge. The bubble does its
     * own motion, so turn that off (PRIVATE_FLAG_NO_MOVE_ANIMATION, not public).
     */
    private fun WindowManager.LayoutParams.noMoveAnimation() {
        runCatching {
            val field = WindowManager.LayoutParams::class.java.getField("privateFlags")
            field.setInt(this, field.getInt(this) or 0x40)
        }
    }

    /** Tap starts listening; a long press hides for 10 minutes; a drag moves it. */
    private inner class DragTapListener(private val view: View) : View.OnTouchListener {
        private var downX = 0f
        private var downY = 0f
        private var startX = 0
        private var startY = 0
        private var moved = false
        private var downAt = 0L
        private val longPress = Runnable {
            if (!moved) {
                view.performHapticFeedback(HapticFeedbackConstants.LONG_PRESS)
                snooze()
            }
        }

        override fun onTouch(v: View, e: MotionEvent): Boolean {
            val lp = params ?: return false
            when (e.actionMasked) {
                MotionEvent.ACTION_DOWN -> {
                    downX = e.rawX; downY = e.rawY; startX = lp.x; startY = lp.y
                    moved = false; dragging = false; downAt = System.currentTimeMillis()
                    main.removeCallbacks(toDot)
                    main.postDelayed(longPress, 550)
                    bubble.animate().scaleX(0.94f).scaleY(0.94f).setDuration(100).start()
                }
                MotionEvent.ACTION_MOVE -> {
                    val dx = e.rawX - downX; val dy = e.rawY - downY
                    if (!moved && hypot(dx, dy) > dp(8f)) {
                        moved = true; dragging = true; main.removeCallbacks(longPress)
                        if (dotted) setDot(false)
                    }
                    if (moved) {
                        lp.x = (startX + dx).toInt(); lp.y = (startY + dy).toInt()
                        root?.let { runCatching { wm.updateViewLayout(it, lp) } }
                    }
                }
                MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                    main.removeCallbacks(longPress)
                    bubble.animate().scaleX(1f).scaleY(1f).setDuration(220).start()
                    dragging = false
                    if (moved) {
                        val metrics = context.resources.displayMetrics
                        if (e.rawY > metrics.heightPixels - dp(96f) && abs(e.rawX - metrics.widthPixels / 2f) < dp(96f)) {
                            snooze()
                        } else {
                            // Settle on the nearer edge.
                            val toLeft = e.rawX < metrics.widthPixels / 2f
                            val target = if (toLeft) dpi(8f) else metrics.widthPixels - view.width - dpi(8f)
                            lp.y = lp.y.coerceIn(dpi(80f), metrics.heightPixels - dpi(160f))
                            Position.save(context, toLeft, lp.y)
                            dockX = target
                            animateX(lp, target)
                        }
                    } else if (e.actionMasked == MotionEvent.ACTION_UP && System.currentTimeMillis() - downAt < 550) {
                        view.performHapticFeedback(HapticFeedbackConstants.VIRTUAL_KEY)
                        Dictation.start(context)
                    }
                    armDot()
                }
            }
            return true
        }

        private fun animateX(lp: WindowManager.LayoutParams, target: Int) {
            ValueAnimator.ofInt(lp.x, target).apply {
                duration = 320
                interpolator = easeOut
                addUpdateListener { lp.x = it.animatedValue as Int; root?.let { r -> runCatching { wm.updateViewLayout(r, lp) } } }
                start()
            }
        }
    }

    private fun snooze() {
        snoozedUntil = System.currentTimeMillis() + 10 * 60 * 1000
        hide(force = true)
    }

    /** Remembers which edge and how high the bubble sits. */
    private object Position {
        private const val PREFS = "bubble"
        fun x(context: Context, width: Int): Int {
            val left = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getBoolean("left", false)
            val screen = context.resources.displayMetrics.widthPixels
            val d = context.resources.displayMetrics.density
            return if (left) (8 * d).toInt() else screen - width - (16 * d).toInt()
        }
        fun y(context: Context): Int {
            val screen = context.resources.displayMetrics.heightPixels
            return context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getInt("y", (screen * 0.52f).toInt())
        }
        fun save(context: Context, left: Boolean, y: Int) {
            context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().putBoolean("left", left).putInt("y", y).apply()
        }
    }
}

/** The sound level as a wavy line, Material 3 Expressive style. */
class WaveView(context: Context) : View(context) {
    var level: Float = 0f
        set(v) { field = v; invalidate() }
    var busy: Boolean = false
        set(v) { field = v; invalidate() }
    private var phase = 0f
    private var amp = 2f
    private val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.STROKE
        strokeCap = Paint.Cap.ROUND
        strokeWidth = TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_DIP, 2.5f, context.resources.displayMetrics)
        color = Color.parseColor("#247872")
    }
    private val path = Path()

    override fun onDraw(canvas: Canvas) {
        val w = width.toFloat(); val h = height.toFloat()
        val target = if (busy) 3f else 1.5f + level * (h / 2 - 3f)
        amp += (target - amp) * 0.5f
        phase += if (busy) 0.5f else 0.9f
        val wl = TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_DIP, 14f, resources.displayMetrics)
        path.reset()
        var x = 0f
        while (x <= w) {
            val y = h / 2 + sin((x / wl) * 2 * Math.PI + phase).toFloat() * amp
            if (x == 0f) path.moveTo(x, y) else path.lineTo(x, y)
            x += 2f
        }
        canvas.drawPath(path, paint)
        postInvalidateOnAnimation()
    }
}
