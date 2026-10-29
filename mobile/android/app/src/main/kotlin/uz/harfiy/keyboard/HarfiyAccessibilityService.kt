package uz.harfiy.keyboard

import android.accessibilityservice.AccessibilityService
import android.accessibilityservice.InputMethod
import android.annotation.SuppressLint
import android.content.Context
import android.content.Intent
import android.content.SharedPreferences
import android.content.pm.ApplicationInfo
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.text.TextUtils
import android.view.accessibility.AccessibilityEvent
import android.view.inputmethod.EditorInfo
import androidx.core.content.edit

/**
 * Harfiy Accessibility Service.
 *
 * Runs seamlessly in the background and intercepts typing across ALL Android apps
 * while the user uses their default system keyboard (Gboard, Samsung, SwiftKey, etc.).
 *
 * Single Source of Truth:
 * - State is completely tied to whether this AccessibilityService is enabled in Android.
 * - Supports disableSelf() for direct deactivation.
 *
 * Text is edited through the accessibility InputConnection (API 33+), the same
 * channel the keyboard types through, so only the two trigger characters change and
 * the rest of the field (formatting, mentions, custom emoji) is left untouched.
 * Every edit is checked against the editor's live state first; on any mismatch the
 * field is left alone rather than risk corrupting it.
 *
 * Keystrokes that convert nothing cost no IPC: the trigger is read from the event
 * itself and planned by the engine before anything talks to the editor.
 */
class HarfiyAccessibilityService : AccessibilityService() {

    companion object {
        const val PREFS_NAME = "harfiy_prefs"
        const val KEY_ENABLE_NG = "harfiy_enable_ng"
        const val KEY_INSTANT_UNDO = "harfiy_instant_undo"
        const val KEY_GAME_PAUSE = "harfiy_game_pause"
        const val KEY_EXCLUDED_APPS = "harfiy_excluded_apps"
        const val KEY_IS_PAUSED = "harfiy_is_paused"

        // Text events caused by our own edit arrive shortly after it; within this window
        // they must not cancel the pending instant undo.
        private const val ECHO_WINDOW_MS = 150L

        // The selection snapshot from onUpdateSelection can lag the text event slightly.
        private const val STALE_RETRY_DELAY_MS = 40L

        // Inserts up to this long are treated as typed words (glide typing, autocorrect);
        // longer ones are pastes and are left as pasted.
        private const val MAX_BULK_INSERT = 48

        @Volatile
        var instance: HarfiyAccessibilityService? = null
            private set

        fun isPaused(context: Context): Boolean {
            val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
            return prefs.getBoolean(KEY_IS_PAUSED, false)
        }

        @SuppressLint("ApplySharedPref")
        fun setPaused(context: Context, paused: Boolean) {
            val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
            prefs.edit(commit = true) { putBoolean(KEY_IS_PAUSED, paused) }
            HarfiyTileService.requestTileUpdate(context)
        }
    }

    private data class UndoState(
        val original: String,
        val replacementIndex: Int,
        val createdAt: Long
    )

    private enum class EditResult { DONE, STALE, MISMATCH }

    /**
     * Tracks the editor's selection and the keyboard's composing region, both needed
     * to place an InputConnection edit exactly on the trigger characters.
     */
    private class TrackingInputMethod(service: AccessibilityService) : InputMethod(service) {
        var selStart = -1
        var selEnd = -1
        var composingStart = -1
        var composingEnd = -1

        override fun onStartInput(attribute: EditorInfo, restarting: Boolean) {
            super.onStartInput(attribute, restarting)
            selStart = attribute.initialSelStart
            selEnd = attribute.initialSelEnd
            composingStart = -1
            composingEnd = -1
        }

        override fun onFinishInput() {
            super.onFinishInput()
            selStart = -1
            selEnd = -1
            composingStart = -1
            composingEnd = -1
        }

        override fun onUpdateSelection(
            oldSelStart: Int, oldSelEnd: Int,
            newSelStart: Int, newSelEnd: Int,
            candidatesStart: Int, candidatesEnd: Int
        ) {
            super.onUpdateSelection(oldSelStart, oldSelEnd, newSelStart, newSelEnd, candidatesStart, candidatesEnd)
            selStart = newSelStart
            selEnd = newSelEnd
            composingStart = minOf(candidatesStart, candidatesEnd)
            composingEnd = maxOf(candidatesStart, candidatesEnd)
        }
    }

    private val mainHandler = Handler(Looper.getMainLooper())
    private var lastUndo: UndoState? = null

    // End offset of a trigger pair we just restored via instant undo, so its echo
    // event isn't converted straight back.
    private var suppressTriggerEnd = -1

    private lateinit var prefs: SharedPreferences

    // Settings mirrored into fields so the typing path never reads preferences.
    private var paused = false
    private var enableNg = false
    private var instantUndo = true
    private var gamePause = true
    private var excludedApps: Set<String> = emptySet()

    // App categories don't change while installed; avoids a PackageManager IPC per event.
    private val gamePackages = HashMap<String, Boolean>()

    // Held in a field: SharedPreferences keeps listeners weakly.
    private val prefsListener = SharedPreferences.OnSharedPreferenceChangeListener { _, _ -> loadSettings() }

    override fun onCreate() {
        super.onCreate()
        prefs = getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
        loadSettings()
        prefs.registerOnSharedPreferenceChangeListener(prefsListener)
    }

    private fun loadSettings() {
        paused = prefs.getBoolean(KEY_IS_PAUSED, false)
        enableNg = prefs.getBoolean(KEY_ENABLE_NG, false)
        instantUndo = prefs.getBoolean(KEY_INSTANT_UNDO, true)
        gamePause = prefs.getBoolean(KEY_GAME_PAUSE, true)
        excludedApps = prefs.getStringSet(KEY_EXCLUDED_APPS, emptySet())?.toSet() ?: emptySet()
    }

    override fun onCreateInputMethod(): InputMethod = TrackingInputMethod(this)

    override fun onServiceConnected() {
        super.onServiceConnected()
        instance = this
    }

    /**
     * Programmatically disables this accessibility service in system settings.
     */
    fun disableService() {
        disableSelf()
        instance = null
    }

    override fun onAccessibilityEvent(event: AccessibilityEvent?) {
        if (event == null) return

        when (event.eventType) {
            AccessibilityEvent.TYPE_VIEW_TEXT_CHANGED -> {}
            AccessibilityEvent.TYPE_VIEW_FOCUSED -> {
                // Focus shifted to another input field - reset undo state
                lastUndo = null
                suppressTriggerEnd = -1
                return
            }
            else -> return
        }

        // Temporary pause (from Quick Settings Tile or in-app toggle)
        if (paused) return

        val pkgName = event.packageName?.toString() ?: ""
        if (pkgName.isNotEmpty() && pkgName in excludedApps) return
        if (gamePause && isGamePackage(pkgName)) return

        handleTextChanged(event, pkgName)
    }

    private fun isGamePackage(pkg: String): Boolean {
        if (pkg.isEmpty()) return false
        return gamePackages.getOrPut(pkg) {
            try {
                packageManager.getApplicationInfo(pkg, 0).category == ApplicationInfo.CATEGORY_GAME
            } catch (_: Exception) {
                false
            }
        }
    }

    private fun handleTextChanged(event: AccessibilityEvent, pkgName: String) {
        if (event.isPassword) return

        val fromIndex = event.fromIndex
        val addedCount = event.addedCount
        val removedCount = event.removedCount

        // Case 1: Instant Undo check (User pressed Backspace right after our transformation)
        val undo = lastUndo
        if (instantUndo && undo != null && addedCount == 0 && removedCount == 1 &&
            fromIndex == undo.replacementIndex
        ) {
            lastUndo = null
            if (replaceBeforeCursor(pkgName, "", undo.original, undo.replacementIndex) == EditResult.DONE) {
                suppressTriggerEnd = undo.replacementIndex + undo.original.length
            }
            return
        }

        // Echoes of our own edit start at or before the replaced pair; anything else
        // means the user moved on, which cancels the undo.
        if (undo != null) {
            val isEcho = fromIndex <= undo.replacementIndex &&
                SystemClock.uptimeMillis() - undo.createdAt < ECHO_WINDOW_MS
            if (!isEcho) lastUndo = null
        }

        // Case 2: New character(s) typed
        if (addedCount <= 0) return

        val insertEnd = fromIndex + addedCount
        val suppressEnd = suppressTriggerEnd
        suppressTriggerEnd = -1
        if (insertEnd == suppressEnd) return

        if (addedCount == 1) {
            val tail = textBefore(event, pkgName, insertEnd, 2) ?: return
            val currChar = tail[tail.length - 1]
            val prevChar = if (tail.length >= 2) tail[tail.length - 2] else null

            val edit = HarfiyEngine.planTail(prevChar, currChar, enableNg) ?: return

            val original = tail.subSequence(tail.length - edit.deleteCount, tail.length).toString()
            convert(pkgName, original, edit.replacement, listOf(edit.kind), insertEnd, undoable = true)
        } else if (addedCount <= MAX_BULK_INSERT) {
            convertInsert(event, pkgName, addedCount, insertEnd)
        }
        // Larger inserts are pastes: rewriting them could drop the pasted formatting.
    }

    /**
     * Converts a multi-character insert (glide typing, autocorrect, suggestion pick),
     * where checking only the last two characters would miss pairs inside the word.
     * The character before the insert is included so a pair split across it still matches.
     */
    private fun convertInsert(event: AccessibilityEvent, pkgName: String, addedCount: Int, insertEnd: Int) {
        val text = textBefore(event, pkgName, insertEnd, addedCount + 1)?.toString() ?: return
        val edits = HarfiyEngine.planEdits(text, enableNg)
        if (edits.isEmpty()) return

        // One replacement from the first edit to the cursor keeps the IC work to a
        // single verified edit, the same shape as the keyboard committing a word.
        val first = edits[0].start
        val converted = buildString {
            var pos = first
            for (edit in edits) {
                append(text, pos, edit.start)
                append(edit.replacement)
                pos = edit.end
            }
            append(text, pos, text.length)
        }
        convert(
            pkgName, text.substring(first), converted, edits.mapNotNull { it.kind }, insertEnd,
            undoable = false
        )
    }

    /**
     * The (up to) [count] characters ending at [end]. Taken from the event's own text,
     * which is already in this process; only if an app leaves it empty does this ask the
     * editor over IPC. Either way [replaceBeforeCursor] re-verifies before editing.
     */
    private fun textBefore(event: AccessibilityEvent, pkgName: String, end: Int, count: Int): CharSequence? {
        if (end < 1) return null
        val eventText = event.text.firstOrNull()
        if (eventText != null && end <= eventText.length) {
            return eventText.subSequence(maxOf(0, end - count), end)
        }

        val im = inputMethod as? TrackingInputMethod ?: return null
        if (im.currentInputEditorInfo?.packageName != pkgName) return null
        val surrounding = im.currentInputConnection?.getSurroundingText(count, 0, 0) ?: return null
        val sel = surrounding.selectionStart
        if (sel != surrounding.selectionEnd || sel < 1) return null
        if (surrounding.offset >= 0 && surrounding.offset + sel != end) return null
        return surrounding.text.subSequence(maxOf(0, sel - count), sel)
    }

    /**
     * Applies one verified replacement before the cursor. Only single-trigger edits are
     * [undoable]: Backspace right after a word-sized conversion should just delete.
     */
    private fun convert(
        pkgName: String,
        original: String,
        replacement: String,
        kinds: List<HarfiyEngine.RuleKind>,
        cursor: Int,
        undoable: Boolean,
        retryIfStale: Boolean = true
    ) {
        when (replaceBeforeCursor(pkgName, original, replacement, cursor)) {
            EditResult.DONE -> {
                if (undoable) {
                    lastUndo = UndoState(
                        original = original,
                        replacementIndex = cursor - original.length,
                        createdAt = SystemClock.uptimeMillis()
                    )
                }
                for (kind in kinds) HarfiyStats.record(this, kind)
            }
            EditResult.STALE -> if (retryIfStale) {
                mainHandler.postDelayed(
                    { convert(pkgName, original, replacement, kinds, cursor, undoable, retryIfStale = false) },
                    STALE_RETRY_DELAY_MS
                )
            }
            EditResult.MISMATCH -> {}
        }
    }

    /**
     * Replaces [expected], which must sit right before a collapsed cursor at [cursor],
     * with [replacement] through the accessibility InputConnection.
     *
     * The keyboard's composing region matters here: commitText replaces it when one
     * exists, and deleteSurroundingText skips over it, so both are planned around it.
     * Returns STALE when our selection snapshot lags the editor (worth one retry) and
     * MISMATCH when the field no longer looks like we expect; neither touches the text.
     */
    private fun replaceBeforeCursor(pkgName: String, expected: String, replacement: String, cursor: Int): EditResult {
        val im = inputMethod as? TrackingInputMethod ?: return EditResult.MISMATCH
        // The event must come from the app whose editor we're connected to.
        if (im.currentInputEditorInfo?.packageName != pkgName) return EditResult.MISMATCH
        val ic = im.currentInputConnection ?: return EditResult.MISMATCH

        val cs = im.composingStart
        val ce = im.composingEnd
        val hasComposing = cs in 0 until ce
        val before = if (hasComposing) maxOf(expected.length, cursor - cs) else expected.length

        val surrounding = ic.getSurroundingText(before, 0, 0) ?: return EditResult.MISMATCH
        val text = surrounding.text
        val sel = surrounding.selectionStart
        if (sel != surrounding.selectionEnd || sel < before) return EditResult.MISMATCH
        if (surrounding.offset >= 0 && surrounding.offset + sel != cursor) return EditResult.MISMATCH
        if (!TextUtils.equals(text.subSequence(sel - expected.length, sel), expected)) return EditResult.MISMATCH

        // The composing region is only trustworthy if the snapshot it came with matches
        // the live cursor.
        if (im.selStart != cursor || im.selEnd != cursor) return EditResult.STALE

        if (!hasComposing) {
            if (expected.isNotEmpty()) ic.deleteSurroundingText(expected.length, 0)
            ic.commitText(replacement, 1, null)
            return EditResult.DONE
        }

        // Composing text that doesn't end at the cursor is an unusual layout; don't guess.
        if (ce != cursor) return EditResult.MISMATCH

        val start = cursor - expected.length
        val composingText = text.subSequence(sel - (cursor - cs), sel)
        if (cs <= start) {
            // The pair is inside the composing word: commitText swaps exactly that word.
            ic.commitText(TextUtils.concat(composingText.subSequence(0, start - cs), replacement), 1, null)
        } else {
            // The pair straddles the composing start: settle the composing text in place,
            // then edit with no composing region left.
            ic.commitText(composingText, 1, null)
            ic.deleteSurroundingText(expected.length, 0)
            ic.commitText(replacement, 1, null)
        }
        return EditResult.DONE
    }

    override fun onInterrupt() {
        lastUndo = null
    }

    override fun onTaskRemoved(rootIntent: Intent?) {
        super.onTaskRemoved(rootIntent)
        // Keep service alive across UI task swipes
    }

    override fun onUnbind(intent: Intent?): Boolean {
        HarfiyStats.flush()
        return super.onUnbind(intent)
    }

    override fun onDestroy() {
        super.onDestroy()
        HarfiyStats.flush()
        mainHandler.removeCallbacksAndMessages(null)
        prefs.unregisterOnSharedPreferenceChangeListener(prefsListener)
        if (instance == this) {
            instance = null
        }
        lastUndo = null
    }
}
