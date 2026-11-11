package uz.harfiy.keyboard

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.provider.Settings
import android.text.Editable
import android.text.TextWatcher
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.InputMethodInfo
import android.view.inputmethod.InputMethodManager
import android.widget.Toast
import androidx.core.content.edit

/**
 * Whether the user's keyboard composes words (predictive text on) or types key by key,
 * and how to change it.
 *
 * Harfiy works either way. With a composing keyboard a word is converted once it is
 * finished (after a space or punctuation) and instant undo is unavailable; with predictive
 * text off, letters convert as they are typed and Backspace restores the pair. That is a
 * trade-off the user makes (suggestions, autocorrect and swipe typing need predictive
 * text), so Harfiy only explains it and never insists.
 *
 * The mode is remembered per keyboard (input method id), so switching keyboards shows the
 * right advice. It is learned from the accessibility session while the user types in any
 * app, and from Harfiy's own test fields via [watchComposing].
 */
object KeyboardMode {

    /** Input method id of the keyboard last seen composing, or absent. */
    private const val KEY_COMPOSING_IME = "harfiy_composing_ime"

    /** Input method id of the keyboard last seen typing key by key, or absent. */
    private const val KEY_PLAIN_IME = "harfiy_plain_ime"

    /** The user closed the in-app tip for this keyboard id. */
    private const val KEY_TIP_DISMISSED_IME = "harfiy_realtime_tip_dismissed_ime"

    private fun prefs(context: Context) =
        context.getSharedPreferences(HarfiyAccessibilityService.PREFS_NAME, Context.MODE_PRIVATE)

    /** Id of the current default keyboard, e.g. `com.samsung.android.honeyboard/.service.HoneyBoardService`. */
    fun currentImeId(context: Context): String? =
        Settings.Secure.getString(context.contentResolver, Settings.Secure.DEFAULT_INPUT_METHOD)

    private fun currentIme(context: Context): InputMethodInfo? {
        val id = currentImeId(context) ?: return null
        val imm = context.getSystemService(InputMethodManager::class.java) ?: return null
        return imm.inputMethodList.firstOrNull { it.id == id }
    }

    /** User-visible name of the current keyboard ("Samsung Keyboard", "Gboard"...). */
    fun keyboardName(context: Context): String =
        currentIme(context)?.loadLabel(context.packageManager)?.toString() ?: "Klaviatura"

    /**
     * Records what was observed for the current keyboard: [composes] true when it
     * composed, false when it typed key by key. Writes only on change.
     */
    fun record(context: Context, composes: Boolean) {
        val id = currentImeId(context) ?: return
        val p = prefs(context)
        val (key, other) = if (composes) KEY_COMPOSING_IME to KEY_PLAIN_IME else KEY_PLAIN_IME to KEY_COMPOSING_IME
        if (p.getString(key, null) == id && p.getString(other, null) != id) return
        p.edit {
            putString(key, id)
            if (p.getString(other, null) == id) remove(other)
        }
    }

    /** The current keyboard was last seen composing (true), typing key by key (false), or never (null). */
    fun currentComposes(context: Context): Boolean? {
        val id = currentImeId(context) ?: return null
        val p = prefs(context)
        return when (id) {
            p.getString(KEY_COMPOSING_IME, null) -> true
            p.getString(KEY_PLAIN_IME, null) -> false
            else -> null
        }
    }

    /** Whether the in-app tip should show: composing keyboard, tip not closed for it. */
    fun shouldShowTip(context: Context): Boolean =
        currentComposes(context) == true &&
            prefs(context).getString(KEY_TIP_DISMISSED_IME, null) != currentImeId(context)

    fun dismissTip(context: Context) {
        prefs(context).edit { putString(KEY_TIP_DISMISSED_IME, currentImeId(context)) }
    }

    /** Where the predictive-text switch lives in the current keyboard's settings. */
    fun predictiveTextHint(context: Context): String {
        val pkg = currentImeId(context)?.substringBefore('/') ?: ""
        return when {
            pkg.startsWith("com.samsung.android.honeyboard") || pkg.startsWith("com.sec.android.inputmethod") ->
                "Samsung Keyboard: Sozlamalar → «Aqlli yozish» (Smart typing) → «Bashoratli matn» (Predictive text) ni o‘chiring."
            pkg == "com.google.android.inputmethod.latin" ->
                "Gboard: Sozlamalar → «Matnni tuzatish» (Text correction) → «Takliflar panelini ko‘rsatish» (Show suggestion strip) ni o‘chiring."
            pkg.startsWith("com.touchtype.swiftkey") ->
                "SwiftKey: Sozlamalar → «Yozish» (Typing) → takliflarni (predictions) o‘chiring."
            else ->
                "Klaviatura sozlamalarida «takliflar» yoki «bashoratli matn» (predictive text) bandini o‘chiring."
        }
    }

    /** Opens the current keyboard's own settings, falling back to the system keyboard list. */
    fun openKeyboardSettings(activity: Activity) {
        val ime = currentIme(activity)
        val settings = ime?.settingsActivity
        val intent = if (ime != null && !settings.isNullOrEmpty()) {
            Intent(Intent.ACTION_MAIN).setClassName(ime.packageName, settings)
        } else {
            Intent(Settings.ACTION_INPUT_METHOD_SETTINGS)
        }
        try {
            activity.startActivity(intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        } catch (_: Exception) {
            try {
                activity.startActivity(Intent(Settings.ACTION_INPUT_METHOD_SETTINGS).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
            } catch (_: Exception) {
                Toast.makeText(activity, "Klaviatura sozlamalarini ochib bo‘lmadi", Toast.LENGTH_SHORT).show()
            }
        }
    }

    /**
     * Watches an in-app field: a composing span means the keyboard composes; several edits
     * without one mean it types key by key. [onResult] gets each new verdict and the result
     * is [record]ed for the current keyboard.
     */
    fun watchComposing(context: Context, onResult: (composes: Boolean) -> Unit): TextWatcher =
        object : TextWatcher {
            private var plainEdits = 0
            private var verdict: Boolean? = null

            override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {}
            override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) {}

            override fun afterTextChanged(s: Editable?) {
                // A composing span is decisive and final: composing keyboards also commit
                // plain edits (spaces, punctuation). Key by key needs three clean edits.
                if (s == null || s.isEmpty() || verdict == true) return
                val now = when {
                    BaseInputConnection.getComposingSpanStart(s) >= 0 -> true
                    ++plainEdits >= 3 -> false
                    else -> return
                }
                if (now != verdict) {
                    verdict = now
                    record(context, now)
                    onResult(now)
                }
            }
        }
}
