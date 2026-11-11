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
import android.util.Log
import android.view.accessibility.AccessibilityEvent
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.SurroundingText
import androidx.core.content.edit

/**
 * Harfiy Accessibility Service.
 *
 * Converts typing in every app while the user keeps their own keyboard (Gboard, Samsung,
 * SwiftKey...). Text is edited through the accessibility InputConnection (API 33+), the
 * channel the keyboard types through, so only the trigger characters change.
 *
 * This class is a thin adapter: it applies the user's settings (pause, excluded apps,
 * games, password fields), forwards editor events to [HarfiySession], and performs the
 * reads and writes the session asks for. The session (Rust, `crates/ffi/src/session.rs`)
 * decides when writing is safe and verifies every write; see its docs for the protocol.
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

        // A write that couldn't be confirmed switches the field off. After this many in
        // one app, the app is skipped until the service restarts.
        private const val MAX_FAILURES_PER_APP = 2

        // A read-back that doesn't show the write yet is repeated once after this delay,
        // in case the editor hadn't applied it when the read was served.
        private const val READBACK_RETRY_MS = 40L

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

        /** Debug-only logging; never logs typed text. Stripped from release by R8. */
        private fun debug(msg: String) {
            if (BuildConfig.DEBUG) Log.d(TAG, msg)
        }
    }

    /** Forwards the editor's selection and composing state to the session. */
    private inner class TrackingInputMethod : InputMethod(this@HarfiyAccessibilityService) {

        override fun onStartInput(attribute: EditorInfo, restarting: Boolean) {
            super.onStartInput(attribute, restarting)
            val pkg = attribute.packageName?.toString() ?: ""
            editorPkg = pkg
            val enabled = isActiveIn(pkg) && isConvertibleField(attribute.inputType) &&
                (failures[pkg] ?: 0) < MAX_FAILURES_PER_APP
            cancelTick()
            verifying = false
            HarfiySession.startInput(enabled, attribute.initialSelStart, attribute.initialSelEnd, now())
            debug("onStartInput pkg=$pkg enabled=$enabled restarting=$restarting")
        }

        override fun onFinishInput() {
            super.onFinishInput()
            cancelTick()
            verifying = false
            // Remember that the keyboard composes, for the in-app real-time tip. Only the
            // positive observation: some fields (no-suggestion, URL...) make a composing
            // keyboard type key by key, which says nothing about the user's setting.
            // Key-by-key is confirmed in Harfiy's own test fields instead.
            if (HarfiySession.keyboardMode() == 1) {
                KeyboardMode.record(this@HarfiyAccessibilityService, composes = true)
            }
            HarfiySession.finishInput(now())
            editorPkg = ""
        }

        override fun onUpdateSelection(
            oldSelStart: Int, oldSelEnd: Int,
            newSelStart: Int, newSelEnd: Int,
            candidatesStart: Int, candidatesEnd: Int
        ) {
            super.onUpdateSelection(oldSelStart, oldSelEnd, newSelStart, newSelEnd, candidatesStart, candidatesEnd)
            val compStart = minOf(candidatesStart, candidatesEnd)
            val compEnd = maxOf(candidatesStart, candidatesEnd)
            arm(HarfiySession.onSelection(newSelStart, newSelEnd, compStart, compEnd, now()))
        }
    }

    private val mainHandler = Handler(Looper.getMainLooper())
    private val tickRunnable = Runnable { onTick() }

    // Package of the editor the keyboard is connected to ("" when none).
    private var editorPkg = ""

    // A write's read-back is pending: don't start another write meanwhile.
    private var verifying = false

    // Unconfirmed writes per app, for this service's lifetime.
    private val failures = HashMap<String, Int>()

    private val snapshotMeta = IntArray(HarfiySession.META_LEN)

    private lateinit var prefs: SharedPreferences

    // Settings mirrored into fields so the typing path never reads preferences.
    private var paused = false
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
        gamePause = prefs.getBoolean(KEY_GAME_PAUSE, true)
        excludedApps = prefs.getStringSet(KEY_EXCLUDED_APPS, emptySet())?.toSet() ?: emptySet()
        HarfiySession.configure(
            prefs.getBoolean(KEY_ENABLE_NG, false),
            prefs.getBoolean(KEY_INSTANT_UNDO, true)
        )
    }

    override fun onCreateInputMethod(): InputMethod = TrackingInputMethod()

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
    }

    /**
     * Programmatically disables this accessibility service in system settings.
     */
    fun disableService() {
        disableSelf()
        instance = null
    }

    override fun onAccessibilityEvent(event: AccessibilityEvent?) {
        if (event == null || event.eventType != AccessibilityEvent.TYPE_VIEW_TEXT_CHANGED) return
        try {
            handleTextChanged(event)
        } catch (e: Throwable) {
            Log.e(TAG, "Unhandled error in onAccessibilityEvent", e)
        }
    }

    private fun handleTextChanged(event: AccessibilityEvent) {
        if (event.isPassword) return
        val pkg = event.packageName?.toString() ?: ""
        // Only the field the keyboard is typing into; other views' text changes
        // (chat lists, counters) are not ours to convert.
        if (editorPkg.isEmpty() || pkg != editorPkg) return
        if (!isActiveIn(pkg)) return

        val from = event.fromIndex
        val removed = event.removedCount
        val added = event.addedCount
        val tail = tailOf(event.text.firstOrNull(), from + added)
        arm(HarfiySession.onTextChanged(from, removed, added, tail, now()))
    }

    /** Up to [HarfiySession.TAIL_LEN] units of [text] before [end], never splitting a surrogate pair. */
    private fun tailOf(text: CharSequence?, end: Int): String? {
        if (text == null || end < 0 || end > text.length) return null
        var start = maxOf(0, end - HarfiySession.TAIL_LEN)
        if (start in 1 until end && Character.isLowSurrogate(text[start])) start++
        return text.subSequence(start, end).toString()
    }

    private fun arm(code: Long) {
        when {
            code == HarfiySession.TICK_IDLE -> Unit
            code == HarfiySession.TICK_SNAPSHOT -> post(0L)
            else -> post(code)
        }
    }

    private fun post(delayMs: Long) {
        mainHandler.removeCallbacks(tickRunnable)
        mainHandler.postDelayed(tickRunnable, delayMs)
    }

    private fun cancelTick() = mainHandler.removeCallbacks(tickRunnable)

    private fun onTick() {
        if (verifying) {
            debug("tick: verifying, skipped")
            return
        }
        when (val code = HarfiySession.onTick(now())) {
            HarfiySession.TICK_SNAPSHOT -> snapshot()
            else -> arm(code)
        }
    }

    private fun snapshot() {
        if (!isActiveIn(editorPkg)) return
        val im = inputMethod as? TrackingInputMethod ?: return
        val ic = im.currentInputConnection
        if (ic == null) {
            debug("snapshot: no input connection")
            return
        }
        val surrounding = readSurrounding(ic, HarfiySession.SNAPSHOT_LEN)
        if (surrounding == null) {
            // Some editors (WebView/Chromium) don't serve surrounding text to accessibility;
            // without it nothing can be verified, so nothing is written.
            debug("snapshot: editor returned no surrounding text")
            return
        }

        val before = textBeforeSelection(surrounding)
        val collapsed = surrounding.selectionStart == surrounding.selectionEnd
        val meta = snapshotMeta
        val replacement = HarfiySession.onSnapshot(before, collapsed, absCursor(surrounding), now(), meta)

        debug("snapshot: result=${meta[HarfiySession.META_RESULT]} len=${before.length} abs=${absCursor(surrounding)}")
        when (meta[HarfiySession.META_RESULT]) {
            HarfiySession.RESULT_RETRY -> post(meta[HarfiySession.META_RETRY_MS].toLong())
            HarfiySession.RESULT_WRITE -> if (replacement != null) write(ic, replacement, meta.copyOf())
        }
    }

    private fun write(ic: InputMethod.AccessibilityInputConnection, replacement: String, meta: IntArray) {
        val pkg = editorPkg
        try {
            when (meta[HarfiySession.META_MODE]) {
                HarfiySession.MODE_DELETE -> {
                    ic.deleteSurroundingText(meta[HarfiySession.META_EXPECTED_LEN], 0)
                    ic.commitText(replacement, 1, null)
                }
                HarfiySession.MODE_INSERT -> ic.commitText(replacement, 1, null)
            }
        } catch (e: Exception) {
            Log.e(TAG, "write failed", e)
            confirm(pkg, null, meta)
            return
        }

        val verifyLen = meta[HarfiySession.META_VERIFY_LEN] + HarfiySession.VERIFY_SLACK
        val first = readSurrounding(ic, verifyLen)
        if (first != null && textBeforeSelection(first).endsWith(replacement)) {
            confirm(pkg, first, meta)
        } else {
            // The editor may not have applied the write when it served the read.
            verifying = true
            mainHandler.postDelayed({
                verifying = false
                val again = (inputMethod as? TrackingInputMethod)?.currentInputConnection
                    ?.let { readSurrounding(it, verifyLen) }
                confirm(pkg, again, meta)
            }, READBACK_RETRY_MS)
        }
    }

    private fun confirm(pkg: String, readback: SurroundingText?, meta: IntArray) {
        val before = readback?.let { textBeforeSelection(it) }
        val cursor = readback?.let { absCursor(it) } ?: -1
        if (HarfiySession.onWritten(before, cursor, now())) {
            for (kind in HarfiyEngine.RuleKind.entries) {
                repeat(meta[HarfiySession.META_KINDS + kind.ordinal]) { HarfiyStats.record(this, kind) }
            }
        } else {
            val count = (failures[pkg] ?: 0) + 1
            failures[pkg] = count
            Log.w(TAG, "Unconfirmed write in $pkg ($count); Harfiy is off for this field")
        }
        // Anything that queued up while verifying gets its turn now.
        post(0L)
    }

    private fun readSurrounding(ic: InputMethod.AccessibilityInputConnection, before: Int): SurroundingText? =
        try {
            ic.getSurroundingText(before, 0, 0)
        } catch (_: Exception) {
            null
        }

    private fun textBeforeSelection(s: SurroundingText): String {
        val sel = s.selectionStart.coerceIn(0, s.text.length)
        return s.text.subSequence(0, sel).toString()
    }

    private fun absCursor(s: SurroundingText): Int =
        if (s.offset >= 0 && s.selectionStart >= 0) s.offset + s.selectionStart else -1

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

    private fun now(): Long = SystemClock.uptimeMillis()

    override fun onInterrupt() {}

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
        HarfiySession.finishInput(now())
        try {
            prefs.unregisterOnSharedPreferenceChangeListener(prefsListener)
        } catch (_: Exception) {}
        if (instance === this) {
            instance = null
        }
    }
}
