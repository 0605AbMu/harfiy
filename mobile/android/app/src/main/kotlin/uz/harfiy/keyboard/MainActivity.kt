package uz.harfiy.keyboard

import android.accessibilityservice.AccessibilityServiceInfo
import android.app.Activity
import android.content.Context
import android.content.Intent
import android.content.SharedPreferences
import android.graphics.Color
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.os.Bundle
import android.provider.Settings
import android.view.Gravity
import android.view.ViewGroup
import android.view.accessibility.AccessibilityManager
import android.widget.*

/**
 * Modern dashboard for configuring and testing Harfiy Accessibility Service.
 */
class MainActivity : Activity() {

    private lateinit var prefs: SharedPreferences
    private lateinit var statusCard: LinearLayout
    private lateinit var statusText: TextView
    private lateinit var actionBtn: Button

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        prefs = getSharedPreferences(HarfiyAccessibilityService.PREFS_NAME, Context.MODE_PRIVATE)

        val dp = resources.displayMetrics.density
        fun dp(v: Int): Int = (v * dp).toInt()

        val scrollView = ScrollView(this).apply {
            layoutParams = ViewGroup.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            setBackgroundColor(Color.parseColor("#F8FAFC"))
        }

        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            val pad = dp(20)
            setPadding(pad, pad, pad, pad)
        }
        scrollView.addView(root)

        // 1. App Header
        val title = TextView(this).apply {
            text = "Harfiy Yordamchisi"
            textSize = 26f
            setTextColor(Color.parseColor("#0F172A"))
            typeface = Typeface.DEFAULT_BOLD
        }
        root.addView(title)

        val subtitle = TextView(this).apply {
            text = "Telefoningizning o‘z klaviaturasida (Gboard, Samsung) yozing — Harfiy fonda avtomatik to‘g‘rilaydi."
            textSize = 14f
            setTextColor(Color.parseColor("#64748B"))
            setPadding(0, dp(4), 0, dp(20))
        }
        root.addView(subtitle)

        // 2. Status Card
        statusCard = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            val padCard = dp(16)
            setPadding(padCard, padCard, padCard, padCard)
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                bottomMargin = dp(20)
            }
        }
        root.addView(statusCard)

        statusText = TextView(this).apply {
            textSize = 16f
            typeface = Typeface.DEFAULT_BOLD
            gravity = Gravity.CENTER_VERTICAL
        }
        statusCard.addView(statusText)

        actionBtn = Button(this).apply {
            text = "Sozlamalardan Harfiy'ni yoqish"
            textSize = 15f
            setTextColor(Color.WHITE)
            typeface = Typeface.DEFAULT_BOLD
            val btnDrawable = GradientDrawable().apply {
                setColor(Color.parseColor("#2563EB"))
                cornerRadius = dp(10).toFloat()
            }
            background = btnDrawable
            val padBtn = dp(12)
            setPadding(padBtn, padBtn, padBtn, padBtn)
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                topMargin = dp(12)
            }
            setOnClickListener {
                startActivity(Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS))
            }
        }
        statusCard.addView(actionBtn)

        // 3. Settings Card
        val settingsCard = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            val padCard = dp(16)
            setPadding(padCard, padCard, padCard, padCard)
            background = GradientDrawable().apply {
                setColor(Color.WHITE)
                cornerRadius = dp(12).toFloat()
                setStroke(dp(1), Color.parseColor("#E2E8F0"))
            }
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                bottomMargin = dp(20)
            }
        }
        root.addView(settingsCard)

        val settingsHeader = TextView(this).apply {
            text = "Sozlamalar"
            textSize = 17f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor(Color.parseColor("#1E293B"))
            setPadding(0, 0, 0, dp(12))
        }
        settingsCard.addView(settingsHeader)

        // Switch: Enable Harfiy
        val switchEnabled = Switch(this).apply {
            text = "Harfiy almashtirishini yoqish"
            textSize = 15f
            setTextColor(Color.parseColor("#334155"))
            isChecked = prefs.getBoolean(HarfiyAccessibilityService.KEY_ENABLED, true)
            setOnCheckedChangeListener { _, isChecked ->
                prefs.edit().putBoolean(HarfiyAccessibilityService.KEY_ENABLED, isChecked).apply()
            }
            setPadding(0, dp(6), 0, dp(6))
        }
        settingsCard.addView(switchEnabled)

        // Switch: Enable Instant Undo
        val switchUndo = Switch(this).apply {
            text = "Instant Undo (Backspace bilan qaytarish)"
            textSize = 15f
            setTextColor(Color.parseColor("#334155"))
            isChecked = prefs.getBoolean(HarfiyAccessibilityService.KEY_INSTANT_UNDO, true)
            setOnCheckedChangeListener { _, isChecked ->
                prefs.edit().putBoolean(HarfiyAccessibilityService.KEY_INSTANT_UNDO, isChecked).apply()
            }
            setPadding(0, dp(6), 0, dp(6))
        }
        settingsCard.addView(switchUndo)

        // Switch: Enable 'ng'
        val switchNg = Switch(this).apply {
            text = "\"ng\" qoidasi (ng -> ñ)"
            textSize = 15f
            setTextColor(Color.parseColor("#334155"))
            isChecked = prefs.getBoolean(HarfiyAccessibilityService.KEY_ENABLE_NG, false)
            setOnCheckedChangeListener { _, isChecked ->
                prefs.edit().putBoolean(HarfiyAccessibilityService.KEY_ENABLE_NG, isChecked).apply()
            }
            setPadding(0, dp(6), 0, dp(6))
        }
        settingsCard.addView(switchNg)

        // 4. Test Field Card
        val testCard = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            val padCard = dp(16)
            setPadding(padCard, padCard, padCard, padCard)
            background = GradientDrawable().apply {
                setColor(Color.WHITE)
                cornerRadius = dp(12).toFloat()
                setStroke(dp(1), Color.parseColor("#E2E8F0"))
            }
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                bottomMargin = dp(20)
            }
        }
        root.addView(testCard)

        val testTitle = TextView(this).apply {
            text = "Sinab ko‘ring"
            textSize = 17f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor(Color.parseColor("#1E293B"))
        }
        testCard.addView(testTitle)

        val testDesc = TextView(this).apply {
            text = "Telefoningiz klaviaturasida shahar, choy, o'rik, g'isht deb yozing:"
            textSize = 13f
            setTextColor(Color.parseColor("#64748B"))
            setPadding(0, dp(4), 0, dp(12))
        }
        testCard.addView(testDesc)

        val practiceField = EditText(this).apply {
            hint = "Bu yerga yozing..."
            textSize = 16f
            setHintTextColor(Color.parseColor("#94A3B8"))
            setTextColor(Color.parseColor("#0F172A"))
            val padField = dp(12)
            setPadding(padField, padField, padField, padField)
            background = GradientDrawable().apply {
                setColor(Color.parseColor("#F1F5F9"))
                cornerRadius = dp(8).toFloat()
            }
        }
        testCard.addView(practiceField)

        // 5. Info Box
        val infoBox = TextView(this).apply {
            text = "💡 Eslatma: Harfiy hech qanday g‘alati yoki yangi klaviatura ochmaydi. Gboard yoki Samsung klaviaturangizda odatdagidek yozaverasiz, Harfiy esa fonda harflarni avtomatik to‘g‘rilab beradi."
            textSize = 13f
            setTextColor(Color.parseColor("#475569"))
            setPadding(dp(14), dp(14), dp(14), dp(14))
            background = GradientDrawable().apply {
                setColor(Color.parseColor("#F8FAFC"))
                cornerRadius = dp(10).toFloat()
                setStroke(dp(1), Color.parseColor("#CBD5E1"))
            }
        }
        root.addView(infoBox)

        setContentView(scrollView)
    }

    override fun onResume() {
        super.onResume()
        updateStatus()
    }

    private fun updateStatus() {
        val isServiceEnabled = isAccessibilityServiceEnabled()
        val dp = resources.displayMetrics.density

        if (isServiceEnabled) {
            statusCard.background = GradientDrawable().apply {
                setColor(Color.parseColor("#F0FDF4"))
                cornerRadius = 12 * dp
                setStroke((1.5f * dp).toInt(), Color.parseColor("#86EFAC"))
            }
            statusText.text = "✅ Harfiy faol (Default klaviatura ustida ishlayapti)"
            statusText.setTextColor(Color.parseColor("#166534"))
            actionBtn.text = "Sozlamalarni ko‘rish"
            (actionBtn.background as? GradientDrawable)?.setColor(Color.parseColor("#15803D"))
        } else {
            statusCard.background = GradientDrawable().apply {
                setColor(Color.parseColor("#FEF2F2"))
                cornerRadius = 12 * dp
                setStroke((1.5f * dp).toInt(), Color.parseColor("#FECACA"))
            }
            statusText.text = "⚠️ Harfiy o‘chiq (Maxsus imkoniyat yoqilmagan)"
            statusText.setTextColor(Color.parseColor("#991B1B"))
            actionBtn.text = "Harfiy'ni yoqish (Sozlamalarga o‘tish)"
            (actionBtn.background as? GradientDrawable)?.setColor(Color.parseColor("#DC2626"))
        }
    }

    private fun isAccessibilityServiceEnabled(): Boolean {
        val am = getSystemService(Context.ACCESSIBILITY_SERVICE) as? AccessibilityManager ?: return false
        val enabledServices = am.getEnabledAccessibilityServiceList(AccessibilityServiceInfo.FEEDBACK_ALL_MASK)
        return enabledServices.any {
            it.resolveInfo.serviceInfo.packageName == packageName &&
            it.resolveInfo.serviceInfo.name.contains("HarfiyAccessibilityService")
        }
    }
}
