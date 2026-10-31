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
import android.text.InputType
import android.text.TextUtils
import android.util.Log
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
 * A word the keyboard is still composing is never edited: the keyboard keeps its own
 * copy of it and would re-send the unconverted word on the next key, duplicating text,
 * and this connection can't update that copy. Such conversions are deferred until the
 * keyboard commits the word.
 *
 * Keystrokes that convert nothing cost no IPC: the trigger is read from the event
 * itself and planned by the engine before anything talks to the editor.
 */
class HarfiyAccessibilityService : AccessibilityService() {

    companion object {
        private const val TAG = "Harfiy"
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

        // Lets the keyboard finish committing a word (word, then space) before the
        // deferred conversion reads it.
        private const val DEFERRED_FLUSH_DELAY_MS = 50L

        // A deferred span longer than this means the cursor went somewhere else.
        private const val MAX_DEFERRED_SPAN = 96

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

    private enum class EditResult { DONE, STALE, COMPOSING, MISMATCH }

    /** A planned replacement of the text right before the cursor. */
    private class Plan(val original: String, val replacement: String, val kinds: List<HarfiyEngine.RuleKind>)

    /**
     * Tracks the editor's selection and the keyboard's composing region, both needed
     * to place an InputConnection edit exactly on the trigger characters.
     */
    private inner class TrackingInputMethod : InputMethod(this@HarfiyAccessibilityService) {
        var selStart = -1
        var selEnd = -1
        var composingStart = -1
        var composingEnd = -1

        val isComposing: Boolean
            get() = composingStart in 0 until composingEnd

        override fun onStartInput(attribute: EditorInfo, restarting: Boolean) {
            super.onStartInput(attribute, restarting)
            selStart = attribute.initialSelStart
            selEnd = attribute.initialSelEnd
            composingStart = -1
            composingEnd = -1
            Log.d(TAG, "onStartInput: pkg=${attribute.packageName}, selStart=$selStart, selEnd=$selEnd")
            resetFieldState()
        }

        override fun onFinishInput() {
            super.onFinishInput()
            Log.d(TAG, "onFinishInput")
            selStart = -1
            selEnd = -1
            composingStart = -1
            composingEnd = -1
            resetFieldState()
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
            onSelectionUpdated(this)
        }
    }

    private val mainHandler = Handler(Looper.getMainLooper())
    private var lastUndo: UndoState? = null

    // End offset of a trigger pair we just restored via instant undo, so its echo
    // event isn't converted straight back.
    private var suppressTriggerEnd = -1

    // Start offset of the pair the user kept via instant undo. Word-sized conversions
    // (keyboard commits, autocorrect) skip it so it isn't converted back later.
    private var keptStart = -1

    // Earliest offset of text with conversions deferred while the keyboard composed it,
    // or -1 when none are pending.
    private var deferredFrom = -1
    private val flushRunnable = Runnable { flushDeferred() }

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

    override fun onCreateInputMethod(): InputMethod {
        Log.d(TAG, "onCreateInputMethod called")
        return TrackingInputMethod()
    }

    override fun onServiceConnected() {
        super.onServiceConnected()
        instance = this
        Log.d(TAG, "onServiceConnected: inputMethod=$inputMethod")
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
                Log.d(TAG, "TYPE_VIEW_FOCUSED: pkg=${event.packageName}")
                resetFieldState()
                return
            }
            else -> return
        }

        val pkgName = event.packageName?.toString() ?: ""
        if (!isActiveIn(pkgName)) return

        handleTextChanged(event, pkgName)
    }

    private fun isActiveIn(pkgName: String): Boolean {
        // Temporary pause (from Quick Settings Tile or in-app toggle)
        if (paused) return false
        if (pkgName.isNotEmpty() && pkgName in excludedApps) return false
        return !(gamePause && isGamePackage(pkgName))
    }

    /**
     * Only free text is converted: passwords, URLs and e-mail addresses would break, and
     * non-text editors (numbers, terminals reporting TYPE_NULL) aren't Uzbek prose.
     */
    private fun isConvertibleField(inputType: Int): Boolean {
        if (inputType and InputType.TYPE_MASK_CLASS != InputType.TYPE_CLASS_TEXT) return false
        return when (inputType and InputType.TYPE_MASK_VARIATION) {
            InputType.TYPE_TEXT_VARIATION_PASSWORD,
            InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD,
            InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD,
            InputType.TYPE_TEXT_VARIATION_URI,
            InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS,
            InputType.TYPE_TEXT_VARIATION_WEB_EMAIL_ADDRESS -> false
            else -> true
        }
    }

    private fun resetFieldState() {
        lastUndo = null
        suppressTriggerEnd = -1
        keptStart = -1
        deferredFrom = -1
        mainHandler.removeCallbacks(flushRunnable)
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

        // Every edit goes through the connected editor, so events from anything else
        // can't be acted on.
        val im = inputMethod as? TrackingInputMethod
        val editor = im?.currentInputEditorInfo
        if (editor == null || editor.packageName != pkgName || !isConvertibleField(editor.inputType)) {
            return
        }

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
                keptStart = undo.replacementIndex
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

            val edit = HarfiyEngine.planTail(prevChar, currChar, enableNg)
            if (edit != null) {
                val original = tail.subSequence(tail.length - edit.deleteCount, tail.length).toString()
                convert(pkgName, original, edit.replacement, listOf(edit.kind), insertEnd, undoable = true)
            } else if (isDelimiter(currChar)) {
                // If the user typed space or punctuation, flush any pending conversions immediately
                if (deferredFrom >= 0) {
                    mainHandler.removeCallbacks(flushRunnable)
                    flushDeferred()
                }
            }
        } else if (addedCount <= MAX_BULK_INSERT) {
            convertInsert(event, pkgName, addedCount, insertEnd)
        }
        // Larger inserts are pastes: rewriting them could drop the pasted formatting.
    }

    private fun isDelimiter(c: Char): Boolean {
        if (c == '\'' || c == '`' || c == '‘' || c == '’' || c == 'ʻ' || c == 'ʼ' || c == '´') {
            return false
        }
        return c.isWhitespace() || c in ".,!?:;()[]{}\"«»—–…•№"
    }

    /**
     * Converts a multi-character insert (glide typing, autocorrect, suggestion pick),
     * where checking only the last two characters would miss pairs inside the word.
     * The character before the insert is included so a pair split across it still matches.
     */
    private fun convertInsert(event: AccessibilityEvent, pkgName: String, addedCount: Int, insertEnd: Int) {
        val text = textBefore(event, pkgName, insertEnd, addedCount + 1)?.toString() ?: return
        val plan = planBeforeCursor(text, insertEnd) ?: return
        val singleKey = event.removedCount + 1 == addedCount
        val undoable = singleKey && plan.original.length in 2..3 && plan.replacement.length == 1
        convert(pkgName, plan.original, plan.replacement, plan.kinds, insertEnd, undoable = undoable)
    }

    /**
     * Plans the conversions in [text], which ends at [cursor], as one replacement from
     * the first edit to the cursor. That keeps the IC work to a single verified edit,
     * the same shape as the keyboard committing a word.
     */
    private fun planBeforeCursor(text: String, cursor: Int): Plan? {
        val textStart = cursor - text.length
        val edits = HarfiyEngine.planEdits(text, enableNg).filter { textStart + it.start != keptStart }
        if (edits.isEmpty()) return null

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
        return Plan(text.substring(first), converted, edits.mapNotNull { it.kind })
    }

    /** Remembers that the text from [from] has conversions waiting for the keyboard to commit it. */
    private fun defer(from: Int) {
        val im = inputMethod as? TrackingInputMethod ?: return
        // The whole composing word is rescanned on commit: autocorrect may rewrite it.
        var start = from
        if (im.composingStart in 0 until start) start = im.composingStart
        if (deferredFrom < 0 || start < deferredFrom) deferredFrom = start
        scheduleFlush(im)
    }

    private fun onSelectionUpdated(im: TrackingInputMethod) {
        // The kept pair was deleted or the cursor moved before it: its offset no longer holds.
        if (keptStart >= 0 && im.selStart < keptStart) keptStart = -1
        scheduleFlush(im)
    }

    /** Debounced to the keyboard's last update, so the flush sees the committed word. */
    private fun scheduleFlush(im: TrackingInputMethod) {
        if (deferredFrom < 0 || im.isComposing) return
        mainHandler.removeCallbacks(flushRunnable)
        mainHandler.postDelayed(flushRunnable, DEFERRED_FLUSH_DELAY_MS)
    }

    /**
     * Applies conversions deferred while the keyboard was composing, once it has
     * committed the text. Anything that doesn't line up yet stays pending for the next
     * selection update; a cursor that left the deferred text drops it.
     */
    private fun flushDeferred() {
        val from = deferredFrom
        if (from < 0) return
        val im = inputMethod as? TrackingInputMethod ?: return
        // The keyboard already started the next word; both are flushed after it.
        if (im.isComposing) return

        val cursor = im.selStart
        val editor = im.currentInputEditorInfo
        val pkgName = editor?.packageName
        if (cursor != im.selEnd || cursor < from || cursor - from > MAX_DEFERRED_SPAN ||
            pkgName == null || !isActiveIn(pkgName) || !isConvertibleField(editor.inputType)
        ) {
            deferredFrom = -1
            return
        }

        // One character before, so a pair split across the composing start still matches.
        val start = maxOf(0, from - 1)
        val length = cursor - start
        val ic = im.currentInputConnection ?: return
        val surrounding = ic.getSurroundingText(length, 0, 0) ?: return
        val sel = surrounding.selectionStart
        if (sel != surrounding.selectionEnd || sel < length) return
        if (surrounding.offset >= 0 && surrounding.offset + sel != cursor) return
        val text = surrounding.text.subSequence(sel - length, sel).toString()

        val plan = planBeforeCursor(text, cursor)
        if (plan == null) {
            deferredFrom = -1
            return
        }
        if (replaceBeforeCursor(pkgName, plan.original, plan.replacement, cursor) == EditResult.DONE) {
            deferredFrom = -1
            for (kind in plan.kinds) HarfiyStats.record(this, kind)
        }
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
        val result = replaceBeforeCursor(pkgName, original, replacement, cursor)
        Log.d(TAG, "convert: original='$original' -> '$replacement' at cursor=$cursor, result=$result")
        when (result) {
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
            } else {
                // Still lagging; the next selection update re-plans from the live text.
                defer(cursor - original.length)
            }
            EditResult.COMPOSING -> defer(cursor - original.length)
            EditResult.MISMATCH -> {}
        }
    }

    /**
     * Replaces [expected], which must sit right before a collapsed cursor at [cursor],
     * with [replacement] through the accessibility InputConnection.
     *
     * Nothing is edited while the keyboard has a composing region: commitText would
     * replace it and deleteSurroundingText skips over it, either way leaving the
     * keyboard's own copy of the word out of date, and its next key re-sends that copy.
     * Returns STALE when our selection snapshot lags the editor (worth one retry),
     * COMPOSING when the keyboard is mid-word (worth deferring) and MISMATCH when the
     * field no longer looks like we expect; none of them touches the text.
     */
    private fun replaceBeforeCursor(pkgName: String, expected: String, replacement: String, cursor: Int): EditResult {
        val im = inputMethod as? TrackingInputMethod
        if (im == null) {
            Log.d(TAG, "replaceBeforeCursor: im is null")
            return EditResult.MISMATCH
        }
        if (im.currentInputEditorInfo?.packageName != pkgName) {
            Log.d(TAG, "replaceBeforeCursor: pkg mismatch ${im.currentInputEditorInfo?.packageName} != $pkgName")
            return EditResult.MISMATCH
        }
        val ic = im.currentInputConnection
        if (ic == null) {
            Log.d(TAG, "replaceBeforeCursor: ic is null")
            return EditResult.MISMATCH
        }

        val surrounding = ic.getSurroundingText(expected.length, 0, 0)
        if (surrounding == null) {
            Log.d(TAG, "replaceBeforeCursor: surrounding null")
            return EditResult.MISMATCH
        }
        val sel = surrounding.selectionStart
        if (sel != surrounding.selectionEnd || sel < expected.length) {
            Log.d(TAG, "replaceBeforeCursor: sel invalid start=$sel end=${surrounding.selectionEnd} len=${expected.length}")
            return EditResult.MISMATCH
        }
        if (surrounding.offset >= 0 && surrounding.offset + sel != cursor) {
            Log.d(TAG, "replaceBeforeCursor: STALE offset=${surrounding.offset} sel=$sel cursor=$cursor")
            return EditResult.STALE
        }
        val sub = surrounding.text.subSequence(sel - expected.length, sel)
        if (!TextUtils.equals(sub, expected)) {
            Log.d(TAG, "replaceBeforeCursor: text mismatch '$sub' != '$expected'")
            return EditResult.MISMATCH
        }

        // Use setSelection to target the exact expected characters (including composing span)
        // and replace them with commitText. This avoids deleteSurroundingText skipping composing
        // text, preserves all formatting outside the replaced range, and cleanly commits the replacement.
        if (expected.isNotEmpty()) {
            ic.setSelection(cursor - expected.length, cursor)
        }
        ic.commitText(replacement, 1, null)
        Log.d(TAG, "replaceBeforeCursor: SUCCESS replaced '$expected' with '$replacement'")
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
