package uz.harfiy.keyboard

import android.accessibilityservice.AccessibilityService
import android.content.Context
import android.content.SharedPreferences
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.view.accessibility.AccessibilityEvent
import android.view.accessibility.AccessibilityNodeInfo

/**
 * Harfiy Accessibility Service.
 *
 * Runs seamlessly in the background and intercepts typing across ALL Android apps
 * while the user uses their default system keyboard (Gboard, Samsung, SwiftKey, etc.).
 *
 * Automatically converts Uzbek phonetic pairs in real time:
 *   sh -> ş, ch -> ç, o' -> ö, g' -> ğ (and optionally ng -> ñ)
 *
 * Supports Harfiy's signature Instant Undo: pressing Backspace immediately after
 * conversion restores the exact pre-conversion text without leaking hints/placeholders.
 */
class HarfiyAccessibilityService : AccessibilityService() {

    companion object {
        const val PREFS_NAME = "harfiy_prefs"
        const val KEY_ENABLED = "harfiy_enabled"
        const val KEY_ENABLE_NG = "harfiy_enable_ng"
        const val KEY_INSTANT_UNDO = "harfiy_instant_undo"

        private val APOSTROPHES = charArrayOf('\'', '`', '‘', '’', 'ʻ', 'ʼ', '´')

        private fun isApostrophe(c: Char): Boolean {
            for (apo in APOSTROPHES) {
                if (c == apo) return true
            }
            return false
        }
    }

    private data class UndoState(
        val nodeHashCode: Int,
        val restoredFullText: String,
        val restoredCursor: Int,
        val replacementIndex: Int
    )

    private val mainHandler = Handler(Looper.getMainLooper())
    private var isSelfUpdating = false
    private var lastUndo: UndoState? = null

    private lateinit var prefs: SharedPreferences

    override fun onCreate() {
        super.onCreate()
        prefs = getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
    }

    override fun onAccessibilityEvent(event: AccessibilityEvent?) {
        if (event == null) return
        if (!prefs.getBoolean(KEY_ENABLED, true)) return

        when (event.eventType) {
            AccessibilityEvent.TYPE_VIEW_TEXT_CHANGED -> {
                handleTextChanged(event)
            }
            AccessibilityEvent.TYPE_VIEW_FOCUSED -> {
                // Focus shifted to another input field - reset undo state
                lastUndo = null
            }
        }
    }

    /**
     * Extracts actual user-typed text from the node, safely stripping any
     * placeholder / hintText that Android framework might return when the field is empty.
     */
    private fun getActualText(source: AccessibilityNodeInfo): String {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            if (source.isShowingHintText) {
                return ""
            }
        }
        val text = source.text?.toString() ?: ""
        val hint = source.hintText?.toString()
        if (!hint.isNullOrEmpty() && text == hint) {
            return ""
        }
        return text
    }

    private fun handleTextChanged(event: AccessibilityEvent) {
        if (isSelfUpdating) return

        val source = event.source ?: return
        if (!source.isEditable || source.isPassword) return

        val fromIndex = event.fromIndex
        val addedCount = event.addedCount
        val removedCount = event.removedCount
        val currentText = getActualText(source)

        // Case 1: Instant Undo check (User pressed Backspace right after our transformation)
        val instantUndoEnabled = prefs.getBoolean(KEY_INSTANT_UNDO, true)
        val undo = lastUndo
        if (instantUndoEnabled && undo != null && addedCount == 0 && removedCount == 1) {
            if (fromIndex == undo.replacementIndex) {
                // The user just deleted our replacement character.
                // Restore the exact pre-replacement string (never touches the hint/placeholder).
                applyTextChange(source, undo.restoredFullText, undo.restoredCursor)
                lastUndo = null
                return
            }
        }

        // Reset undo state if user typed or deleted elsewhere
        lastUndo = null

        // Case 2: New character(s) typed
        if (addedCount <= 0) return

        val insertEnd = fromIndex + addedCount
        if (insertEnd < 2 || insertEnd > currentText.length) return

        val prevChar = currentText[insertEnd - 2]
        val currChar = currentText[insertEnd - 1]

        val enableNg = prefs.getBoolean(KEY_ENABLE_NG, false)
        val replacement = matchRule(prevChar, currChar, enableNg) ?: return

        // Perform replacement of the 2-character trigger
        val prefix = currentText.substring(0, insertEnd - 2)
        val suffix = currentText.substring(insertEnd)
        val newText = prefix + replacement + suffix
        val newCursor = prefix.length + replacement.length

        // Save pristine full text for instant undo (independent of future node hint states)
        val originalPair = "$prevChar$currChar"
        val restoredFullText = prefix + originalPair + suffix
        val restoredCursor = prefix.length + originalPair.length

        lastUndo = UndoState(
            nodeHashCode = source.hashCode(),
            restoredFullText = restoredFullText,
            restoredCursor = restoredCursor,
            replacementIndex = prefix.length
        )

        applyTextChange(source, newText, newCursor)
    }

    private fun matchRule(prev: Char, curr: Char, enableNg: Boolean): String? {
        return when {
            // sh -> ş
            prev == 's' && (curr == 'h' || curr == 'H') -> "ş"
            prev == 'S' && (curr == 'h' || curr == 'H') -> "Ş"

            // ch -> ç
            prev == 'c' && (curr == 'h' || curr == 'H') -> "ç"
            prev == 'C' && (curr == 'h' || curr == 'H') -> "Ç"

            // o' -> ö
            prev == 'o' && isApostrophe(curr) -> "ö"
            prev == 'O' && isApostrophe(curr) -> "Ö"

            // g' -> ğ
            prev == 'g' && isApostrophe(curr) -> "ğ"
            prev == 'G' && isApostrophe(curr) -> "Ğ"

            // ng -> ñ
            enableNg && prev == 'n' && (curr == 'g' || curr == 'G') -> "ñ"
            enableNg && prev == 'N' && (curr == 'g' || curr == 'G') -> "Ñ"

            else -> null
        }
    }

    private fun applyTextChange(source: AccessibilityNodeInfo, newText: String, newCursor: Int) {
        isSelfUpdating = true
        try {
            val setArgs = Bundle().apply {
                putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, newText)
            }
            val success = source.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, setArgs)

            if (success) {
                val selArgs = Bundle().apply {
                    putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, newCursor)
                    putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, newCursor)
                }
                source.performAction(AccessibilityNodeInfo.ACTION_SET_SELECTION, selArgs)
            }
        } finally {
            mainHandler.postDelayed({
                isSelfUpdating = false
            }, 30)
        }
    }

    override fun onInterrupt() {
        lastUndo = null
    }

    override fun onDestroy() {
        super.onDestroy()
        lastUndo = null
    }
}
