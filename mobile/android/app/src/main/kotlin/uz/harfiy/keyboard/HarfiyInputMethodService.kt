package uz.harfiy.keyboard

import android.inputmethodservice.InputMethodService
import android.view.KeyEvent
import android.view.View
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.widget.Button
import android.widget.LinearLayout

/**
 * Android InputMethodService implementing the Harfiy auto-conversion keyboard.
 */
class HarfiyInputMethodService : InputMethodService() {

    private lateinit var engine: HarfiyEngineBridge
    private var isShifted: Boolean = false

    override fun onCreate() {
        super.onCreate()
        engine = HarfiyEngineBridge(enableNg = false)
    }

    override fun onDestroy() {
        engine.close()
        super.onDestroy()
    }

    override fun onStartInputView(info: EditorInfo?, restarting: Boolean) {
        super.onStartInputView(info, restarting)
        engine.reset()
    }

    override fun onUpdateSelection(
        oldSelStart: Int,
        oldSelEnd: Int,
        newSelStart: Int,
        newSelEnd: Int,
        candidatesStart: Int,
        candidatesEnd: Int
    ) {
        super.onUpdateSelection(oldSelStart, oldSelEnd, newSelStart, newSelEnd, candidatesStart, candidatesEnd)
        // If selection moved significantly or user tapped elsewhere, reset engine buffer
        if (oldSelStart != newSelStart && Math.abs(oldSelStart - newSelStart) > 1) {
            engine.reset()
        }
    }

    override fun onCreateInputView(): View {
        return createKeyboardLayout()
    }

    // MARK: - Programmatic Clean Keyboard View

    private fun createKeyboardLayout(): View {
        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(0xFFECEFF1.toInt()) // Soft slate/grey background
            val padding = (6 * resources.displayMetrics.density).toInt()
            setPadding(padding, padding, padding, padding)
        }

        val row1 = listOf("q", "w", "e", "r", "t", "y", "u", "i", "o", "p")
        val row2 = listOf("a", "s", "d", "f", "g", "h", "j", "k", "l")
        val row3 = listOf("z", "x", "c", "v", "b", "n", "m")

        root.addView(createKeyRow(row1))
        root.addView(createKeyRow(row2))

        // Row 3: Shift + Letters + Backspace
        val row3Layout = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                0,
                1.0f
            )
        }

        // Shift button
        val shiftBtn = createSpecialButton(if (isShifted) "⇧" else "⇪") {
            isShifted = !isShifted
            setInputView(createKeyboardLayout())
        }
        row3Layout.addView(shiftBtn)

        // Letters
        for (letter in row3) {
            val display = if (isShifted) letter.uppercase() else letter.lowercase()
            row3Layout.addView(createKeyButton(display) { handleKey(display) })
        }

        // Backspace button
        val backspaceBtn = createSpecialButton("⌫") {
            handleBackspace()
        }
        row3Layout.addView(backspaceBtn)
        root.addView(row3Layout)

        // Row 4: Symbols, Space, Enter
        val row4Layout = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                0,
                1.0f
            )
        }

        val numBtn = createSpecialButton("123") {
            // Reserved for number row
        }
        row4Layout.addView(numBtn)

        val spaceBtn = Button(this).apply {
            text = "bo'sh joy"
            isAllCaps = false
            textSize = 14f
            setBackgroundColor(0xFFFFFFFF.toInt())
            layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.MATCH_PARENT, 4.0f).apply {
                val margin = (2 * resources.displayMetrics.density).toInt()
                setMargins(margin, margin, margin, margin)
            }
            setOnClickListener { handleSpace() }
        }
        row4Layout.addView(spaceBtn)

        val enterBtn = createSpecialButton("↵") {
            handleEnter()
        }
        row4Layout.addView(enterBtn)
        root.addView(row4Layout)

        return root
    }

    private fun createKeyRow(keys: List<String>): LinearLayout {
        return LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                0,
                1.0f
            )
            for (key in keys) {
                val display = if (isShifted) key.uppercase() else key.lowercase()
                addView(createKeyButton(display) { handleKey(display) })
            }
        }
    }

    private fun createKeyButton(title: String, onClick: () -> Unit): Button {
        return Button(this).apply {
            text = title
            isAllCaps = false
            textSize = 18f
            setTextColor(0xFF263238.toInt())
            setBackgroundColor(0xFFFFFFFF.toInt())
            layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.MATCH_PARENT, 1.0f).apply {
                val margin = (2 * resources.displayMetrics.density).toInt()
                setMargins(margin, margin, margin, margin)
            }
            setOnClickListener { onClick() }
        }
    }

    private fun createSpecialButton(title: String, onClick: () -> Unit): Button {
        return Button(this).apply {
            text = title
            isAllCaps = false
            textSize = 16f
            setTextColor(0xFF37474F.toInt())
            setBackgroundColor(0xFFCFD8DC.toInt())
            layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.MATCH_PARENT, 1.3f).apply {
                val margin = (2 * resources.displayMetrics.density).toInt()
                setMargins(margin, margin, margin, margin)
            }
            setOnClickListener { onClick() }
        }
    }

    // MARK: - Keystroke Handling

    private fun handleKey(str: String) {
        val ic: InputConnection = currentInputConnection ?: return
        val char = str.firstOrNull() ?: return

        when (val action = engine.processChar(char)) {
            is HarfiyAction.PassThrough -> {
                ic.commitText(str, 1)
            }
            is HarfiyAction.Suppress -> {
                // Key event suppressed
            }
            is HarfiyAction.Transform -> {
                // Erase previous char(s) and emit replacement
                ic.deleteSurroundingText(action.backspacesCount, 0)
                ic.commitText(action.replacement, 1)
            }
        }

        if (isShifted) {
            isShifted = false
            setInputView(createKeyboardLayout())
        }
    }

    private fun handleBackspace() {
        val ic: InputConnection = currentInputConnection ?: return

        when (val action = engine.processBackspace()) {
            is HarfiyAction.Transform -> {
                // Instant Undo: erase converted char and restore original pair
                ic.deleteSurroundingText(action.backspacesCount, 0)
                ic.commitText(action.replacement, 1)
            }
            else -> {
                ic.deleteSurroundingText(1, 0)
            }
        }
    }

    private fun handleSpace() {
        val ic: InputConnection = currentInputConnection ?: return
        engine.reset()
        ic.commitText(" ", 1)
    }

    private fun handleEnter() {
        val ic: InputConnection = currentInputConnection ?: return
        engine.reset()
        ic.sendKeyEvent(KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_ENTER))
        ic.sendKeyEvent(KeyEvent(KeyEvent.ACTION_UP, KeyEvent.KEYCODE_ENTER))
    }
}
