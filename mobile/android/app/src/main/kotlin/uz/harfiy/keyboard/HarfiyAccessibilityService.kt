package uz.harfiy.keyboard

import android.accessibilityservice.AccessibilityService
import android.content.Context
import android.content.SharedPreferences
import android.content.pm.ApplicationInfo
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
 * Single Source of Truth:
 * - State is completely tied to whether this AccessibilityService is enabled in Android.
 * - Supports disableSelf() for direct deactivation.
 */
class HarfiyAccessibilityService : AccessibilityService() {

    companion object {
        const val PREFS_NAME = "harfiy_prefs"
        const val KEY_ENABLE_NG = "harfiy_enable_ng"
        const val KEY_INSTANT_UNDO = "harfiy_instant_undo"
        const val KEY_GAME_PAUSE = "harfiy_game_pause"
        const val KEY_EXCLUDED_APPS = "harfiy_excluded_apps"

        // Stats keys
        const val KEY_STATS_TOTAL = "stats_total"
        const val KEY_STATS_SH = "stats_sh"
        const val KEY_STATS_CH = "stats_ch"
        const val KEY_STATS_O = "stats_o"
        const val KEY_STATS_G = "stats_g"
        const val KEY_STATS_NG = "stats_ng"

        @Volatile
        var instance: HarfiyAccessibilityService? = null
            private set

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

    override fun onServiceConnected() {
        super.onServiceConnected()
        instance = this
    }

    /**
     * Programmatically disables this accessibility service in system settings.
     * Available on Android 7.0 (API 24) and above.
     */
    fun disableService() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
            disableSelf()
        }
        instance = null
    }

    override fun onAccessibilityEvent(event: AccessibilityEvent?) {
        if (event == null) return

        // 1. Excluded apps check
        val pkgName = event.packageName?.toString() ?: ""
        val excludedApps = prefs.getStringSet(KEY_EXCLUDED_APPS, emptySet()) ?: emptySet()
        if (pkgName.isNotEmpty() && excludedApps.contains(pkgName)) {
            return
        }

        // 2. Game mode auto-pause check
        if (prefs.getBoolean(KEY_GAME_PAUSE, true) && isGamePackage(pkgName)) {
            return
        }

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

    private fun isGamePackage(pkg: String): Boolean {
        if (pkg.isEmpty()) return false
        return try {
            val appInfo = packageManager.getApplicationInfo(pkg, 0)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                appInfo.category == ApplicationInfo.CATEGORY_GAME
            } else {
                false
            }
        } catch (_: Exception) {
            false
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
                // Restore the exact pre-replacement string (never touches the hint/placeholder)
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
        val match = matchRule(prevChar, currChar, enableNg) ?: return

        val replacement = match.replacement

        // Perform replacement of the 2-character trigger
        val prefix = currentText.substring(0, insertEnd - 2)
        val suffix = currentText.substring(insertEnd)
        val newText = prefix + replacement + suffix
        val newCursor = prefix.length + replacement.length

        // Save pristine full text for instant undo
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
        recordStat(match.statKey)
    }

    private data class RuleMatch(val replacement: String, val statKey: String)

    private fun matchRule(prev: Char, curr: Char, enableNg: Boolean): RuleMatch? {
        return when {
            // sh -> ş
            prev == 's' && (curr == 'h' || curr == 'H') -> RuleMatch("ş", KEY_STATS_SH)
            prev == 'S' && (curr == 'h' || curr == 'H') -> RuleMatch("Ş", KEY_STATS_SH)

            // ch -> ç
            prev == 'c' && (curr == 'h' || curr == 'H') -> RuleMatch("ç", KEY_STATS_CH)
            prev == 'C' && (curr == 'h' || curr == 'H') -> RuleMatch("Ç", KEY_STATS_CH)

            // o' -> ö
            prev == 'o' && isApostrophe(curr) -> RuleMatch("ö", KEY_STATS_O)
            prev == 'O' && isApostrophe(curr) -> RuleMatch("Ö", KEY_STATS_O)

            // g' -> ğ
            prev == 'g' && isApostrophe(curr) -> RuleMatch("ğ", KEY_STATS_G)
            prev == 'G' && isApostrophe(curr) -> RuleMatch("Ğ", KEY_STATS_G)

            // ng -> ñ
            enableNg && prev == 'n' && (curr == 'g' || curr == 'G') -> RuleMatch("ñ", KEY_STATS_NG)
            enableNg && prev == 'N' && (curr == 'g' || curr == 'G') -> RuleMatch("Ñ", KEY_STATS_NG)

            else -> null
        }
    }

    private fun recordStat(statKey: String) {
        val total = prefs.getLong(KEY_STATS_TOTAL, 0L) + 1
        val typeCount = prefs.getLong(statKey, 0L) + 1
        prefs.edit()
            .putLong(KEY_STATS_TOTAL, total)
            .putLong(statKey, typeCount)
            .apply()
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
        if (instance == this) {
            instance = null
        }
        lastUndo = null
    }
}
