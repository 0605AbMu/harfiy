package uz.harfiy.keyboard

import android.annotation.SuppressLint
import android.app.Activity
import android.content.Context
import android.os.Bundle
import android.text.Editable
import android.text.InputType
import android.text.TextWatcher
import android.util.Log
import android.view.Gravity
import android.view.ViewGroup
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputConnectionWrapper
import android.view.inputmethod.InputMethodManager
import android.view.inputmethod.SurroundingText
import android.webkit.JavascriptInterface
import android.webkit.WebView
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.TextView

/**
 * Debug-only test bench for the accessibility service, driven by
 * `mobile/scripts/e2e_android.py`.
 *
 * `--es field <kind>` picks the editor under test:
 * - `edit`: a plain [EditText] (Telegram, WhatsApp... report absolute offsets);
 * - `plain`: an EditText with `TYPE_TEXT_FLAG_NO_SUGGESTIONS`, where keyboards such as
 *   Samsung Keyboard type key by key without a composing region (live conversion and
 *   instant undo path);
 * - `nooffset`: an EditText whose InputConnection reports `SurroundingText.offset == -1`,
 *   like Jetpack Compose and other editors that don't extend BaseInputConnection;
 * - `web`: a `<textarea>` in a WebView.
 *
 * Every change of the field's text is logged as `HarfiyTestbed: TEXT=<text>` so the
 * script can read the result without UiAutomator (which rebinds accessibility services).
 * `--ez dump true` logs the edit session's diagnostics as `HarfiyTestbed: DIAG ...`.
 */
class TestbedActivity : Activity() {

    companion object {
        private const val TAG = "HarfiyTestbed"

        private fun report(text: String) {
            Log.i(TAG, "TEXT=" + text.replace("\n", "\\n"))
        }
    }

    // No auto-capitalisation, so the script can predict the result.
    private val plainText = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        if (intent.getBooleanExtra("dump", false)) {
            // `--ez dump true`: print the session's recent decisions and close.
            HarfiySession.diagnostics().lineSequence().forEach { Log.i(TAG, "DIAG $it") }
            finish()
            return
        }
        val kind = intent.getStringExtra("field") ?: "edit"
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(32, 96, 32, 32)
        }
        root.addView(TextView(this).apply { text = "Harfiy testbed: $kind" })

        val params = LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 360)
        when (kind) {
            "web" -> root.addView(createWebField(), params)
            else -> {
                val field = if (kind == "nooffset") NoOffsetEditText(this) else EditText(this)
                field.inputType = if (kind == "plain") {
                    plainText or InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
                } else {
                    plainText
                }
                field.gravity = Gravity.TOP or Gravity.START
                field.addTextChangedListener(object : TextWatcher {
                    override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {}
                    override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) {}
                    override fun afterTextChanged(s: Editable?) = report(s?.toString() ?: "")
                })
                root.addView(field, params)
                field.requestFocus()
                field.post {
                    getSystemService(InputMethodManager::class.java)
                        .showSoftInput(field, InputMethodManager.SHOW_IMPLICIT)
                }
            }
        }
        setContentView(root)
        report("")
    }

    @SuppressLint("SetJavaScriptEnabled")
    private fun createWebField(): WebView = WebView(this).apply {
        settings.javaScriptEnabled = true
        addJavascriptInterface(object {
            @JavascriptInterface
            fun text(value: String) = report(value)
        }, "harfiy")
        val html = """
            <html><body style="margin:0">
            <textarea id="t" autocapitalize="off" style="width:100%;height:100%;font-size:20px"
                oninput="harfiy.text(this.value)"></textarea>
            <script>document.getElementById('t').focus();</script>
            </body></html>
        """.trimIndent()
        loadDataWithBaseURL(null, html, "text/html", "utf-8", null)
        requestFocus()
    }

    /** An EditText whose InputConnection doesn't report absolute offsets. */
    private class NoOffsetEditText(context: Context) : EditText(context) {
        override fun onCreateInputConnection(outAttrs: EditorInfo): InputConnection? {
            val base = super.onCreateInputConnection(outAttrs) ?: return null
            return object : InputConnectionWrapper(base, true) {
                override fun getSurroundingText(beforeLength: Int, afterLength: Int, flags: Int): SurroundingText? {
                    val st = super.getSurroundingText(beforeLength, afterLength, flags) ?: return null
                    return SurroundingText(st.text, st.selectionStart, st.selectionEnd, -1)
                }
            }
        }
    }
}
