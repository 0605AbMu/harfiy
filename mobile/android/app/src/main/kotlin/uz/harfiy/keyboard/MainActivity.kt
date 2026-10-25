package uz.harfiy.keyboard

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.provider.Settings
import android.view.inputmethod.InputMethodManager
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.TextView

/**
 * Onboarding and settings activity guiding the user to activate Harfiy Keyboard.
 */
class MainActivity : Activity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            val p = (24 * resources.displayMetrics.density).toInt()
            setPadding(p, p, p, p)
            setBackgroundColor(0xFFFAFAFA.toInt())
        }

        val title = TextView(this).apply {
            text = "Harfiy Klaviaturasi"
            textSize = 24f
            setTextColor(0xFF1E88E5.toInt())
            paint.isFakeBoldText = true
        }
        root.addView(title)

        val desc = TextView(this).apply {
            text = "O'zbek lotin alifbosi yangi unikal harflari (ş, ç, ö, ğ) ni real vaqtda avtomatik kirituvchi klaviatura."
            textSize = 14f
            setTextColor(0xFF546E7A.toInt())
            setPadding(0, 16, 0, 32)
        }
        root.addView(desc)

        // Step 1 Button: Enable in Settings
        val btnEnable = Button(this).apply {
            text = "1. Sozlamalardan Klaviaturani Yoqish"
            setOnClickListener {
                startActivity(Intent(Settings.ACTION_INPUT_METHOD_SETTINGS))
            }
        }
        root.addView(btnEnable)

        // Step 2 Button: Select Harfiy
        val btnSelect = Button(this).apply {
            text = "2. Harfiy Klaviaturasini Tanlash"
            setOnClickListener {
                val imm = getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
                imm.showInputMethodPicker()
            }
        }
        root.addView(btnSelect)

        // Practice Input Field
        val testLabel = TextView(this).apply {
            text = "\nBu yerda sinab ko'ring (shahar, choy, o'rik, g'isht):"
            textSize = 14f
            setTextColor(0xFF37474F.toInt())
        }
        root.addView(testLabel)

        val practiceField = EditText(this).apply {
            hint = "Matn kiriting..."
            textSize = 16f
        }
        root.addView(practiceField)

        setContentView(root)
    }
}
