package io.gannyu.input

import android.content.Context
import android.util.AttributeSet
import android.view.MotionEvent
import android.view.View
import android.view.ViewConfiguration
import android.widget.Button
import android.widget.LinearLayout

/**
 * A keyboard row with separate visual and logical key bounds.
 *
 * AOSP LatinIME models a key's visible bounds independently from its hit box, so the visual
 * horizontal/vertical gaps still belong to a key. This row keeps the existing child layout and
 * routes each touch cell to its closest visible Button.
 *
 * Reference: AOSP LatinIME Key.java (mWidth/mHeight versus mHitBox) and KeyDetector.java.
 */
internal class KeyboardRowLayout @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
) : LinearLayout(context, attrs) {
    private val touchSlop = ViewConfiguration.get(context).scaledTouchSlop
    private var activeKey: Button? = null

    override fun dispatchTouchEvent(event: MotionEvent): Boolean {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                activeKey = nearestKey(event.x.toInt()) ?: return super.dispatchTouchEvent(event)
                return dispatchInside(activeKey!!, event)
            }

            MotionEvent.ACTION_MOVE -> {
                val target = activeKey ?: return super.dispatchTouchEvent(event)
                val outsideVertically = event.y < -touchSlop || event.y >= height + touchSlop
                if (outsideVertically || nearestKey(event.x.toInt()) !== target) {
                    cancel(target, event)
                    activeKey = null
                    return true
                }
                return dispatchInside(target, event)
            }

            MotionEvent.ACTION_UP -> {
                val target = activeKey ?: return super.dispatchTouchEvent(event)
                activeKey = null
                if (nearestKey(event.x.toInt()) !== target) {
                    cancel(target, event)
                    return true
                }
                return dispatchInside(target, event)
            }

            MotionEvent.ACTION_CANCEL -> {
                val target = activeKey ?: return super.dispatchTouchEvent(event)
                activeKey = null
                cancel(target, event)
                return true
            }

            else -> {
                val target = activeKey ?: return super.dispatchTouchEvent(event)
                return dispatchInside(target, event)
            }
        }
    }

    private fun nearestKey(x: Int): Button? {
        val keys = ArrayList<Button>(childCount)
        for (index in 0 until childCount) {
            val child = getChildAt(index)
            if (child is Button && child.visibility == View.VISIBLE && child.isEnabled) {
                keys += child
            }
        }
        val lefts = IntArray(keys.size) { keys[it].left }
        val rights = IntArray(keys.size) { keys[it].right }
        val nearestIndex = HorizontalKeyHitResolver.nearestKeyIndex(x, lefts, rights)
        if (nearestIndex < 0) return null
        return keys[nearestIndex]
    }

    private fun dispatchInside(target: Button, source: MotionEvent): Boolean {
        val event = MotionEvent.obtain(source)
        val maxX = (target.width - 1).coerceAtLeast(0).toFloat()
        val maxY = (target.height - 1).coerceAtLeast(0).toFloat()
        event.setLocation(
            (source.x - target.left).coerceIn(0f, maxX),
            (source.y - target.top).coerceIn(0f, maxY),
        )
        return try {
            target.dispatchTouchEvent(event)
        } finally {
            event.recycle()
        }
    }

    private fun cancel(target: Button, source: MotionEvent) {
        val event = MotionEvent.obtain(source)
        event.action = MotionEvent.ACTION_CANCEL
        event.offsetLocation(-target.left.toFloat(), -target.top.toFloat())
        try {
            target.dispatchTouchEvent(event)
        } finally {
            event.recycle()
        }
    }
}
