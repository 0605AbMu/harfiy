package uz.harfiy.keyboard

import android.accessibilityservice.AccessibilityService
import android.accessibilityservice.AccessibilityServiceInfo
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
 * channel the keyboard types through, so only the trigger characters change and
 * the rest of the field (formatting, mentions, custom emoji) is left untouched.
 *
 * Highly robust against fast typing, composing states, and device-specific quirks.
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
        private const val ECHO_WINDOW_MS = 200L

        // The selection snapshot from onUpdateSelection can lag the text event slightly.
        private const val STALE_RETRY_DELAY_MS = 40L

        // Inserts up to this long are treated as typed words (glide typing, autocorrect);
        // longer ones are pastes and are left as pasted.
        private const val MAX_BULK_INSERT = 48

        // Lets the keyboard finish committing a word (word, then space) before the
        // deferred conversion reads it.
        private const val DEFERRED_FLUSH_DELAY_MS = 40L

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
     * Tracks the editor's selection and input state.
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
            if (deferredFrom >= 0) {
                flushDeferred()
            }
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
            val wasComposing = isComposing
            selStart = newSelStart
            selEnd = newSelEnd
            composingStart = minOf(candidatesStart, candidatesEnd)
            composingEnd = maxOf(candidatesStart, candidatesEnd)
            Log.d(TAG, "onUpdateSelection: sel=[$newSelStart..$newSelEnd], candidates=[$candidatesStart..$candidatesEnd], wasComposing=$wasComposing, isComposing=$isComposing")
            if (keptStart >= 0 && newSelStart <= keptStart) {
                keptStart = -1
            }
            if (wasComposing && !isComposing) {
                scheduleFlush(this)
            }
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

    // Echo suppression and loop circuit breaker
    private var lastSelfEditTime = 0L
    private var rapidEditCount = 0
    private var lastRapidEditTime = 0L

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
        try {
            val info = serviceInfo ?: AccessibilityServiceInfo()
            info.flags = info.flags or
                    AccessibilityServiceInfo.FLAG_INPUT_METHOD_EDITOR or
                    AccessibilityServiceInfo.FLAG_RETRIEVE_INTERACTIVE_WINDOWS
            serviceInfo = info
        } catch (e: Exception) {
            Log.e(TAG, "Failed to configure serviceInfo flags", e)
        }
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

        try {
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
        } catch (e: Throwable) {
            Log.e(TAG, "Unhandled error in onAccessibilityEvent", e)
        }
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
        rapidEditCount = 0
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

        val im = inputMethod as? TrackingInputMethod
        val editor = im?.currentInputEditorInfo
        val activePkg = editor?.packageName?.toString() ?: pkgName
        if (!isActiveIn(activePkg) || !isActiveIn(pkgName)) {
            return
        }
        if (editor != null && !isConvertibleField(editor.inputType)) {
            return
        }

        val fromIndex = event.fromIndex
        val addedCount = event.addedCount
        val removedCount = event.removedCount

        Log.d(TAG, "handleTextChanged: from=$fromIndex, add=$addedCount, rem=$removedCount, text='${event.text.firstOrNull()}', isComposing=${im?.isComposing}")

        // Echo suppression: Ignore events generated by Harfiy's own edits within ECHO_WINDOW_MS
        val now = SystemClock.uptimeMillis()
        if (now - lastSelfEditTime < ECHO_WINDOW_MS) {
            Log.d(TAG, "handleTextChanged: suppressing echo within ${now - lastSelfEditTime}ms")
            return
        }

        if (event.text.firstOrNull().isNullOrEmpty()) {
            resetFieldState()
        }
        if (removedCount > 0 && keptStart >= 0 && fromIndex <= keptStart) {
            keptStart = -1
        }

        // Case 1: Instant Undo check (User pressed Backspace right after our transformation)
        val undo = lastUndo
        if (instantUndo && undo != null && addedCount == 0 && removedCount == 1 &&
            fromIndex == undo.replacementIndex
        ) {
            lastUndo = null
            if (replaceBeforeCursor(pkgName, "", undo.original) == EditResult.DONE) {
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

        val tail = textBefore(event, insertEnd, 1)
        val lastChar = if (!tail.isNullOrEmpty()) tail[tail.length - 1] else null
        val isAtDelimiter = lastChar != null && isDelimiter(lastChar)

        if (isAtDelimiter) {
            mainHandler.removeCallbacks(flushRunnable)
            checkWordBeforeDelimiter(event, pkgName, insertEnd)
            deferredFrom = -1
            return
        }

        // When the keyboard has predictive text / composing active (Samsung Honeyboard, Gboard),
        // live modifications mid-word corrupt the keyboard's internal composing buffer.
        // A keystroke that replaces N characters with N+1 is the definition of setComposingText.
        // We defer conversion until the word is committed upon space, punctuation, or composing end.
        val isComposingKey = (im?.isComposing == true) || (removedCount > 0 && addedCount == removedCount + 1)
        if (isComposingKey) {
            defer(fromIndex)
            return
        }

        if (addedCount == 1) {
            val pairTail = textBefore(event, insertEnd, 2)
            if (pairTail != null && pairTail.length >= 2) {
                val currChar = pairTail[pairTail.length - 1]
                val prevChar = pairTail[pairTail.length - 2]
                val edit = HarfiyEngine.planTail(prevChar, currChar, enableNg)
                if (edit != null && pairTail.length >= edit.deleteCount) {
                    val original = pairTail.subSequence(pairTail.length - edit.deleteCount, pairTail.length).toString()
                    convert(pkgName, original, edit.replacement, listOf(edit.kind), insertEnd, undoable = true)
                }
            }
        } else if (addedCount <= MAX_BULK_INSERT) {
            convertInsert(event, pkgName, addedCount, insertEnd)
        }
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
        val singleKey = event.removedCount + 1 == addedCount
        if (singleKey) {
            defer(event.fromIndex)
            return
        }

        val text = textBefore(event, insertEnd, addedCount + 1)?.toString() ?: return
        Log.d(TAG, "convertInsert: addedCount=$addedCount, insertEnd=$insertEnd, textBefore='$text'")
        val plan = planBeforeCursor(text, insertEnd) ?: return
        val undoable = singleKey && plan.original.length in 2..3 && plan.replacement.length == 1
        convert(pkgName, plan.original, plan.replacement, plan.kinds, insertEnd, undoable = undoable)
    }

    /**
     * Safety net for fast typing or autocorrect: inspects the word directly before the delimiter
     * (e.g. space or punctuation) to convert any unconverted trigger pairs.
     */
    private fun checkWordBeforeDelimiter(event: AccessibilityEvent, pkgName: String, insertEnd: Int) {
        val count = 32
        val text = textBefore(event, insertEnd, count)?.toString() ?: return
        val plan = planBeforeCursor(text, insertEnd) ?: return
        convert(pkgName, plan.original, plan.replacement, plan.kinds, insertEnd, undoable = false)
    }

    /**
     * Plans the conversions in [text], which ends at [cursor], as one replacement from
     * the first edit in the current word to the cursor.
     * Restricting to the current word prevents deleting or merging earlier words.
     */
    private fun planBeforeCursor(text: String, cursor: Int): Plan? {
        if (text.isEmpty()) return null

        // Skip any trailing delimiters (whitespace, punctuation) to find the word end
        var wordEnd = text.length
        while (wordEnd > 0 && isDelimiter(text[wordEnd - 1])) wordEnd--
        if (wordEnd == 0) return null

        // Find the start of the word
        var wordStart = wordEnd
        while (wordStart > 0 && !isDelimiter(text[wordStart - 1])) wordStart--
        if (wordStart == wordEnd) return null

        val word = text.substring(wordStart, wordEnd)
        val wordCursor = cursor - (text.length - wordEnd)
        val wordTextStart = wordCursor - word.length

        val edits = HarfiyEngine.planEdits(word, enableNg).filter { wordTextStart + it.start != keptStart }
        if (edits.isEmpty()) return null

        val first = edits[0].start
        val convertedWord = buildString {
            var pos = first
            for (edit in edits) {
                append(word, pos, edit.start)
                append(edit.replacement)
                pos = edit.end
            }
            append(word, pos, word.length)
        }
        val trailing = text.substring(wordEnd)
        val original = word.substring(first) + trailing
        val replacement = convertedWord + trailing
        return Plan(original, replacement, edits.mapNotNull { it.kind })
    }

    /** Remembers that the text from [from] has conversions waiting for the keyboard to commit it. */
    private fun defer(from: Int) {
        val im = inputMethod as? TrackingInputMethod ?: return
        var start = from
        if (im.composingStart in 0 until start) start = im.composingStart
        if (deferredFrom < 0 || start < deferredFrom) deferredFrom = start
        // Never schedule flush here! Flushing must only happen when the word is finished
        // (upon space, delimiter, or when composing ends).
    }

    /** Debounced to the keyboard's last update, so the flush sees the committed word. */
    private fun scheduleFlush(im: TrackingInputMethod) {
        if (deferredFrom < 0 || im.isComposing) return
        mainHandler.removeCallbacks(flushRunnable)
        mainHandler.postDelayed(flushRunnable, DEFERRED_FLUSH_DELAY_MS)
    }

    /**
     * Applies conversions deferred while the keyboard was composing, once it has
     * committed the text.
     */
    private fun flushDeferred() {
        val from = deferredFrom
        if (from < 0) return
        val im = inputMethod as? TrackingInputMethod ?: return
        if (im.isComposing) return

        val editor = im.currentInputEditorInfo
        val pkgName = editor?.packageName?.toString() ?: ""
        if (pkgName.isNotEmpty() && !isActiveIn(pkgName)) {
            deferredFrom = -1
            return
        }
        if (editor != null && !isConvertibleField(editor.inputType)) {
            deferredFrom = -1
            return
        }

        val ic = im.currentInputConnection ?: return
        val surrounding = try {
            ic.getSurroundingText(MAX_DEFERRED_SPAN, 0, 0)
        } catch (_: Exception) {
            null
        } ?: return

        val sel = surrounding.selectionStart
        if (sel != surrounding.selectionEnd || sel <= 0) return
        val text = surrounding.text.subSequence(0, sel).toString()
        val cursor = if (surrounding.offset >= 0) surrounding.offset + sel else text.length

        val plan = planBeforeCursor(text, cursor)
        if (plan == null) {
            deferredFrom = -1
            return
        }
        if (replaceBeforeCursor(pkgName, plan.original, plan.replacement) == EditResult.DONE) {
            deferredFrom = -1
            for (kind in plan.kinds) HarfiyStats.record(this, kind)
        }
    }

    /**
     * The (up to) [count] characters ending at [end]. Taken from the event's own text,
     * which is already in this process; if an app leaves it empty, falls back to the
     * InputConnection.
     */
    private fun textBefore(event: AccessibilityEvent, end: Int, count: Int): CharSequence? {
        if (count < 1 || end < 1) return null

        try {
            val eventText = event.text.firstOrNull()
            if (eventText != null && end <= eventText.length) {
                val start = maxOf(0, end - count)
                return eventText.subSequence(start, end)
            }
        } catch (_: Exception) {}

        val im = inputMethod as? TrackingInputMethod ?: return null
        val ic = im.currentInputConnection ?: return null

        try {
            val surrounding = ic.getSurroundingText(count, 0, 0)
            if (surrounding != null) {
                val sel = surrounding.selectionStart
                if (sel == surrounding.selectionEnd && sel > 0) {
                    val start = maxOf(0, sel - count)
                    return surrounding.text.subSequence(start, sel)
                }
            }
        } catch (_: Exception) {}

        return null
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
        undoable: Boolean
    ) {
        val result = replaceBeforeCursor(pkgName, original, replacement)
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
            EditResult.COMPOSING -> {
                defer(cursor - original.length)
            }
            EditResult.STALE,
            EditResult.MISMATCH -> {}
        }
    }

    /**
     * Replaces [expected], which must sit right before the current cursor,
     * with [replacement] through the accessibility InputConnection.
     */
    private fun replaceBeforeCursor(
        pkgName: String,
        expected: String,
        replacement: String
    ): EditResult {
        val im = inputMethod as? TrackingInputMethod ?: return EditResult.MISMATCH
        val ic = im.currentInputConnection ?: return EditResult.MISMATCH

        val editor = im.currentInputEditorInfo
        val editorPkg = editor?.packageName?.toString()
        if (editorPkg != null && !isActiveIn(editorPkg)) return EditResult.MISMATCH
        if (pkgName.isNotEmpty() && !isActiveIn(pkgName)) return EditResult.MISMATCH

        // NEVER edit text while the keyboard has an active composing region.
        // Editing while composing desynchronizes predictive keyboards (Samsung, Gboard, SwiftKey).
        if (im.isComposing) {
            Log.d(TAG, "replaceBeforeCursor: COMPOSING active [${im.composingStart}..${im.composingEnd}], deferring")
            return EditResult.COMPOSING
        }

        // Loop circuit breaker: if more than 3 conversions happen within 400ms at the same spot,
        // stop to prevent automated feedback loops with an external IME.
        val now = SystemClock.uptimeMillis()
        if (now - lastRapidEditTime < 400L) {
            rapidEditCount++
            if (rapidEditCount > 3) {
                Log.w(TAG, "Circuit breaker tripped: $rapidEditCount edits within 400ms. Aborting to prevent loop.")
                return EditResult.MISMATCH
            }
        } else {
            rapidEditCount = 0
        }
        lastRapidEditTime = now

        try {
            if (expected.isNotEmpty()) {
                val surrounding = try {
                    ic.getSurroundingText(expected.length, 0, 0)
                } catch (_: Exception) {
                    null
                }

                if (surrounding == null) {
                    Log.d(TAG, "replaceBeforeCursor: surrounding null")
                    return EditResult.MISMATCH
                }

                val sel = surrounding.selectionStart
                if (sel != surrounding.selectionEnd || sel < expected.length) {
                    Log.d(TAG, "replaceBeforeCursor: sel invalid start=$sel end=${surrounding.selectionEnd}")
                    return EditResult.MISMATCH
                }

                val actualBefore = surrounding.text.subSequence(sel - expected.length, sel)
                if (!TextUtils.equals(actualBefore, expected)) {
                    Log.d(TAG, "replaceBeforeCursor: text mismatch '$actualBefore' != '$expected'")
                    return EditResult.MISMATCH
                }

                val docSel = if (surrounding.offset >= 0) surrounding.offset + sel else sel
                val selStart = docSel - expected.length
                val selEnd = docSel
                Log.d(TAG, "replaceBeforeCursor: offset=${surrounding.offset}, sel=$sel, docSel=$docSel, replacing [$selStart..$selEnd] ('$expected') with '$replacement'")
                if (selStart >= 0 && selEnd >= selStart) {
                    ic.setSelection(selStart, selEnd)
                } else {
                    ic.deleteSurroundingText(expected.length, 0)
                }
            }

            ic.commitText(replacement, 1, null)
            lastSelfEditTime = SystemClock.uptimeMillis()
            Log.d(TAG, "replaceBeforeCursor: SUCCESS replaced '$expected' with '$replacement'")
            return EditResult.DONE
        } catch (e: Exception) {
            Log.e(TAG, "replaceBeforeCursor error", e)
            return EditResult.MISMATCH
        }
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
        try {
            prefs.unregisterOnSharedPreferenceChangeListener(prefsListener)
        } catch (_: Exception) {}
        if (instance === this) {
            instance = null
        }
        lastUndo = null
    }
}
