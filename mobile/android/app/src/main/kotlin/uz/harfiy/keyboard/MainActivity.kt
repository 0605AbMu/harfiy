package uz.harfiy.keyboard

import android.accessibilityservice.AccessibilityServiceInfo
import android.app.Activity
import android.app.AlertDialog
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.SharedPreferences
import android.content.pm.ApplicationInfo
import android.graphics.Color
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.net.Uri
import android.os.Bundle
import android.provider.Settings
import android.view.Gravity
import android.view.ViewGroup
import android.view.accessibility.AccessibilityManager
import android.widget.*
import java.util.Locale

/**
 * Modern dashboard for Harfiy on Android.
 *
 * Implements Single Source of Truth (SSOT):
 * - The 'Faol (Harfiy)' toggle is 1:1 bound to the system AccessibilityService state.
 * - Turning it ON opens system Accessibility settings to grant permission.
 * - Turning it OFF directly disables the service via disableSelf() in system settings.
 * - No conflicting local flags or duplicate buttons.
 */
class MainActivity : Activity() {

    private lateinit var prefs: SharedPreferences
    private var engineBridge: HarfiyEngineBridge? = null

    // UI elements requiring dynamic updates
    private lateinit var masterCard: LinearLayout
    private lateinit var masterTitle: TextView
    private lateinit var masterSubtitle: TextView
    private lateinit var switchActive: Switch
    private lateinit var statsSummaryText: TextView
    private lateinit var statsBreakdownText: TextView
    private lateinit var excludedSummaryText: TextView

    private var isUpdatingUI = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        prefs = getSharedPreferences(HarfiyAccessibilityService.PREFS_NAME, Context.MODE_PRIVATE)

        try {
            engineBridge = HarfiyEngineBridge(prefs.getBoolean(HarfiyAccessibilityService.KEY_ENABLE_NG, false))
        } catch (_: Exception) {
            engineBridge = null
        }

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
            val pad = dp(18)
            setPadding(pad, pad, pad, pad)
        }
        scrollView.addView(root)

        // Helper to create modern rounded cards
        fun createCard(): LinearLayout {
            return LinearLayout(this).apply {
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
                    bottomMargin = dp(16)
                }
            }
        }

        // =====================================================================
        // 1. APP HEADER
        // =====================================================================
        val headerRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(0, 0, 0, dp(4))
        }
        val appTitle = TextView(this).apply {
            text = "Harfiy"
            textSize = 26f
            setTextColor(Color.parseColor("#0F172A"))
            typeface = Typeface.DEFAULT_BOLD
        }
        val versionBadge = TextView(this).apply {
            text = "v1.0.1"
            textSize = 12f
            setTextColor(Color.parseColor("#2563EB"))
            typeface = Typeface.DEFAULT_BOLD
            val padH = dp(8)
            val padV = dp(2)
            setPadding(padH, padV, padH, padV)
            background = GradientDrawable().apply {
                setColor(Color.parseColor("#EFF6FF"))
                cornerRadius = dp(6).toFloat()
            }
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                leftMargin = dp(8)
            }
        }
        headerRow.addView(appTitle)
        headerRow.addView(versionBadge)
        root.addView(headerRow)

        val appSubtitle = TextView(this).apply {
            text = "Yangi o‘zbek lotin alifbosi yordamchisi"
            textSize = 14f
            setTextColor(Color.parseColor("#64748B"))
            setPadding(0, 0, 0, dp(16))
        }
        root.addView(appSubtitle)

        // =====================================================================
        // 2. MASTER TOGGLE: Faol (Harfiy) - Single Source of Truth
        // =====================================================================
        masterCard = createCard().apply {
            isClickable = true
            isFocusable = true
        }
        root.addView(masterCard)

        val masterRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
        }

        val textColumn = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f).apply {
                rightMargin = dp(12)
            }
        }

        masterTitle = TextView(this).apply {
            text = "Faol (Harfiy)"
            textSize = 18f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor(Color.parseColor("#0F172A"))
        }
        textColumn.addView(masterTitle)

        masterSubtitle = TextView(this).apply {
            textSize = 13f
            setPadding(0, dp(4), 0, 0)
        }
        textColumn.addView(masterSubtitle)
        masterRow.addView(textColumn)

        switchActive = Switch(this).apply {
            isClickable = true
            setOnCheckedChangeListener { _, isChecked ->
                if (isUpdatingUI) return@setOnCheckedChangeListener

                val isSystemActive = isAccessibilityServiceEnabled()
                if (isChecked && !isSystemActive) {
                    // Turn switch back visually until user actually enables in Settings
                    isUpdatingUI = true
                    this.isChecked = false
                    isUpdatingUI = false

                    openAccessibilitySettings()
                } else if (!isChecked && isSystemActive) {
                    // User deliberately turned off: disable the service in system
                    HarfiyAccessibilityService.instance?.disableService()
                    updateStatus()
                }
            }
        }
        masterRow.addView(switchActive)
        masterCard.addView(masterRow)

        // Tapping anywhere on the card when disabled prompts to enable
        masterCard.setOnClickListener {
            if (!isAccessibilityServiceEnabled()) {
                openAccessibilitySettings()
            } else {
                switchActive.performClick()
            }
        }

        // =====================================================================
        // 3. 🪄 MATNNI O'GIRISH (Batch Magic Text Converter)
        // =====================================================================
        val convertCard = createCard()
        root.addView(convertCard)

        val convertTitleRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
        }
        val magicIcon = ImageView(this).apply {
            setImageResource(R.drawable.ic_menu_magic)
            layoutParams = LinearLayout.LayoutParams(dp(20), dp(20)).apply {
                rightMargin = dp(8)
            }
        }
        val convertTitle = TextView(this).apply {
            text = "Matnni o'girish"
            textSize = 17f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor(Color.parseColor("#1E293B"))
        }
        convertTitleRow.addView(magicIcon)
        convertTitleRow.addView(convertTitle)
        convertCard.addView(convertTitleRow)

        val convertSubtitle = TextView(this).apply {
            text = "Istalgan matnni bir zumda yangi o‘zbek lotin imlosiga o‘giring:"
            textSize = 13f
            setTextColor(Color.parseColor("#64748B"))
            setPadding(0, dp(4), 0, dp(10))
        }
        convertCard.addView(convertSubtitle)

        val convertInput = EditText(this).apply {
            hint = "Matnni bu yerga qo'ying yoki yozing..."
            textSize = 15f
            setTextColor(Color.parseColor("#0F172A"))
            setHintTextColor(Color.parseColor("#94A3B8"))
            minLines = 3
            gravity = Gravity.TOP or Gravity.START
            val padField = dp(10)
            setPadding(padField, padField, padField, padField)
            background = GradientDrawable().apply {
                setColor(Color.parseColor("#F1F5F9"))
                cornerRadius = dp(8).toFloat()
            }
        }
        convertCard.addView(convertInput)

        val convertBtnRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                topMargin = dp(10)
            }
        }

        val btnDoConvert = Button(this).apply {
            text = "🪄 O'girish"
            textSize = 14f
            setTextColor(Color.WHITE)
            typeface = Typeface.DEFAULT_BOLD
            background = GradientDrawable().apply {
                setColor(Color.parseColor("#2563EB"))
                cornerRadius = dp(8).toFloat()
            }
            layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1.2f).apply {
                rightMargin = dp(6)
            }
            setOnClickListener {
                val input = convertInput.text.toString()
                if (input.isNotEmpty()) {
                    val enableNg = prefs.getBoolean(HarfiyAccessibilityService.KEY_ENABLE_NG, false)
                    val converted = if (engineBridge != null) {
                        engineBridge!!.convertText(input, smartApostrophe = true)
                    } else {
                        fallbackConvertText(input, enableNg)
                    }
                    convertInput.setText(converted)
                    convertInput.setSelection(converted.length)
                    Toast.makeText(this@MainActivity, "Matn muvaffaqiyatli o'girildi!", Toast.LENGTH_SHORT).show()
                }
            }
        }
        convertBtnRow.addView(btnDoConvert)

        val btnCopy = Button(this).apply {
            text = "📋 Nusxa olish"
            textSize = 13f
            setTextColor(Color.parseColor("#1E293B"))
            background = GradientDrawable().apply {
                setColor(Color.parseColor("#E2E8F0"))
                cornerRadius = dp(8).toFloat()
            }
            layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1.2f).apply {
                rightMargin = dp(6)
            }
            setOnClickListener {
                val textToCopy = convertInput.text.toString()
                if (textToCopy.isNotEmpty()) {
                    val cm = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                    cm.setPrimaryClip(ClipData.newPlainText("Harfiy", textToCopy))
                    Toast.makeText(this@MainActivity, "Nusxalandi!", Toast.LENGTH_SHORT).show()
                }
            }
        }
        convertBtnRow.addView(btnCopy)

        val btnClear = Button(this).apply {
            text = "Tozalash"
            textSize = 13f
            setTextColor(Color.parseColor("#64748B"))
            background = GradientDrawable().apply {
                setColor(Color.parseColor("#F1F5F9"))
                cornerRadius = dp(8).toFloat()
            }
            layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 0.9f)
            setOnClickListener {
                convertInput.setText("")
            }
        }
        convertBtnRow.addView(btnClear)
        convertCard.addView(convertBtnRow)

        // =====================================================================
        // 4. SOZLAMALAR (Rules & Options)
        // =====================================================================
        val settingsCard = createCard()
        root.addView(settingsCard)

        val settingsHeader = TextView(this).apply {
            text = "Qoidalar va sozlamalar"
            textSize = 17f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor(Color.parseColor("#1E293B"))
            setPadding(0, 0, 0, dp(8))
        }
        settingsCard.addView(settingsHeader)

        // Switch: 'ng' -> 'ñ'
        val switchNg = Switch(this).apply {
            text = "'ng' -> 'ñ' qoidasi"
            textSize = 15f
            setTextColor(Color.parseColor("#334155"))
            isChecked = prefs.getBoolean(HarfiyAccessibilityService.KEY_ENABLE_NG, false)
            setOnCheckedChangeListener { _, isChecked ->
                prefs.edit().putBoolean(HarfiyAccessibilityService.KEY_ENABLE_NG, isChecked).apply()
                engineBridge?.setEnableNg(isChecked)
            }
            setPadding(0, dp(6), 0, dp(6))
        }
        settingsCard.addView(switchNg)

        // Switch: Instant Undo
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

        // Switch: O'yinlarda avtomatik pauza
        val switchGame = Switch(this).apply {
            text = "O'yinlarda avtomatik pauza"
            textSize = 15f
            setTextColor(Color.parseColor("#334155"))
            isChecked = prefs.getBoolean(HarfiyAccessibilityService.KEY_GAME_PAUSE, true)
            setOnCheckedChangeListener { _, isChecked ->
                prefs.edit().putBoolean(HarfiyAccessibilityService.KEY_GAME_PAUSE, isChecked).apply()
            }
            setPadding(0, dp(6), 0, dp(6))
        }
        settingsCard.addView(switchGame)

        // =====================================================================
        // 5. ISTISNO DASTURLAR (Excluded Apps)
        // =====================================================================
        val excludedCard = createCard()
        root.addView(excludedCard)

        val excludedRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
        }
        val folderIcon = ImageView(this).apply {
            setImageResource(R.drawable.ic_menu_web)
            layoutParams = LinearLayout.LayoutParams(dp(18), dp(18)).apply {
                rightMargin = dp(8)
            }
        }
        val excludedTitle = TextView(this).apply {
            text = "Istisno dasturlar"
            textSize = 17f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor(Color.parseColor("#1E293B"))
        }
        excludedRow.addView(folderIcon)
        excludedRow.addView(excludedTitle)
        excludedCard.addView(excludedRow)

        excludedSummaryText = TextView(this).apply {
            textSize = 13f
            setTextColor(Color.parseColor("#64748B"))
            setPadding(0, dp(4), 0, dp(8))
        }
        excludedCard.addView(excludedSummaryText)

        val btnManageExcluded = Button(this).apply {
            text = "Dasturlar ro'yxatini boshqarish"
            textSize = 14f
            setTextColor(Color.parseColor("#1E293B"))
            background = GradientDrawable().apply {
                setColor(Color.parseColor("#F1F5F9"))
                cornerRadius = dp(8).toFloat()
            }
            setOnClickListener {
                showExcludedAppsDialog()
            }
        }
        excludedCard.addView(btnManageExcluded)

        // =====================================================================
        // 6. STATISTIKA (Statistics)
        // =====================================================================
        val statsCard = createCard()
        root.addView(statsCard)

        val statsTitleRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
        }
        val statsIcon = ImageView(this).apply {
            setImageResource(R.drawable.ic_menu_stats)
            layoutParams = LinearLayout.LayoutParams(dp(20), dp(20)).apply {
                rightMargin = dp(8)
            }
        }
        val statsTitle = TextView(this).apply {
            text = "Statistika"
            textSize = 17f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor(Color.parseColor("#1E293B"))
        }
        statsTitleRow.addView(statsIcon)
        statsTitleRow.addView(statsTitle)
        statsCard.addView(statsTitleRow)

        statsSummaryText = TextView(this).apply {
            textSize = 15f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor(Color.parseColor("#0F172A"))
            setPadding(0, dp(6), 0, dp(2))
        }
        statsCard.addView(statsSummaryText)

        statsBreakdownText = TextView(this).apply {
            textSize = 13f
            setTextColor(Color.parseColor("#64748B"))
            setPadding(0, 0, 0, dp(10))
        }
        statsCard.addView(statsBreakdownText)

        val btnClearStats = Button(this).apply {
            text = "Statistikani tozalash"
            textSize = 13f
            setTextColor(Color.parseColor("#EF4444"))
            background = GradientDrawable().apply {
                setColor(Color.parseColor("#FEF2F2"))
                cornerRadius = dp(8).toFloat()
            }
            setOnClickListener {
                prefs.edit()
                    .remove(HarfiyAccessibilityService.KEY_STATS_TOTAL)
                    .remove(HarfiyAccessibilityService.KEY_STATS_SH)
                    .remove(HarfiyAccessibilityService.KEY_STATS_CH)
                    .remove(HarfiyAccessibilityService.KEY_STATS_O)
                    .remove(HarfiyAccessibilityService.KEY_STATS_G)
                    .remove(HarfiyAccessibilityService.KEY_STATS_NG)
                    .apply()
                updateStats()
                Toast.makeText(this@MainActivity, "Statistika tozalandi", Toast.LENGTH_SHORT).show()
            }
        }
        statsCard.addView(btnClearStats)

        // =====================================================================
        // 7. SINAB KO'RISH MAYDONI (Interactive Playground)
        // =====================================================================
        val testCard = createCard()
        root.addView(testCard)

        val testTitle = TextView(this).apply {
            text = "Sinab ko‘ring"
            textSize = 17f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor(Color.parseColor("#1E293B"))
        }
        testCard.addView(testTitle)

        val testDesc = TextView(this).apply {
            text = "Telefoningiz klaviaturasida yozib ko‘ring (shahar, choy, o'rik, g'isht):"
            textSize = 13f
            setTextColor(Color.parseColor("#64748B"))
            setPadding(0, dp(4), 0, dp(10))
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

        // =====================================================================
        // 8. HAVOLALAR (Web, Blog, Support)
        // =====================================================================
        val linksCard = createCard()
        root.addView(linksCard)

        val linksTitle = TextView(this).apply {
            text = "Loyiha haqida"
            textSize = 17f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor(Color.parseColor("#1E293B"))
            setPadding(0, 0, 0, dp(8))
        }
        linksCard.addView(linksTitle)

        fun createLinkRow(iconRes: Int, label: String, url: String): LinearLayout {
            return LinearLayout(this).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER_VERTICAL
                val padRow = dp(10)
                setPadding(padRow, padRow, padRow, padRow)
                background = GradientDrawable().apply {
                    setColor(Color.parseColor("#F8FAFC"))
                    cornerRadius = dp(8).toFloat()
                }
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.MATCH_PARENT,
                    LinearLayout.LayoutParams.WRAP_CONTENT
                ).apply {
                    bottomMargin = dp(6)
                }
                setOnClickListener {
                    try {
                        startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)))
                    } catch (_: Exception) {
                        Toast.makeText(this@MainActivity, "Havolani ochib bo'lmadi", Toast.LENGTH_SHORT).show()
                    }
                }
                val icon = ImageView(context).apply {
                    setImageResource(iconRes)
                    layoutParams = LinearLayout.LayoutParams(dp(20), dp(20)).apply {
                        rightMargin = dp(10)
                    }
                }
                val text = TextView(context).apply {
                    text = label
                    textSize = 15f
                    setTextColor(Color.parseColor("#1E293B"))
                }
                addView(icon)
                addView(text)
            }
        }

        linksCard.addView(createLinkRow(R.drawable.ic_menu_web, "Veb-sahifa", "https://abdumannon-shamsiyev.uz/harfiy"))
        linksCard.addView(createLinkRow(R.drawable.ic_menu_blog, "Blog (@abdumannon_notes)", "https://t.me/abdumannon_notes"))
        linksCard.addView(createLinkRow(R.drawable.ic_menu_support, "Support (@abdumannon_shamsiyev)", "https://t.me/abdumannon_shamsiyev"))

        setContentView(scrollView)
    }

    override fun onResume() {
        super.onResume()
        updateStatus()
        updateStats()
        updateExcludedAppsSummary()
    }

    override fun onDestroy() {
        super.onDestroy()
        engineBridge?.close()
        engineBridge = null
    }

    private fun openAccessibilitySettings() {
        Toast.makeText(this, "Ro‘yxatdan 'Harfiy'ni tanlang va yoqing", Toast.LENGTH_LONG).show()
        try {
            startActivity(Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS))
        } catch (_: Exception) {
            startActivity(Intent(Settings.ACTION_SETTINGS))
        }
    }

    /**
     * Single Source of Truth update:
     * Reflects the EXACT system Accessibility status into the master switch and card.
     */
    private fun updateStatus() {
        val isServiceEnabled = isAccessibilityServiceEnabled()
        val dp = resources.displayMetrics.density

        isUpdatingUI = true
        switchActive.isChecked = isServiceEnabled
        isUpdatingUI = false

        if (isServiceEnabled) {
            masterCard.background = GradientDrawable().apply {
                setColor(Color.parseColor("#F0FDF4"))
                cornerRadius = 12 * dp
                setStroke((1.5f * dp).toInt(), Color.parseColor("#86EFAC"))
            }
            masterTitle.text = "✓ Faol (Harfiy)"
            masterTitle.setTextColor(Color.parseColor("#166534"))
            masterSubtitle.text = "Gboard / Samsung klaviaturasi ustida ishlayapti (O‘chirish uchun bosing)"
            masterSubtitle.setTextColor(Color.parseColor("#15803D"))
        } else {
            masterCard.background = GradientDrawable().apply {
                setColor(Color.parseColor("#FEF2F2"))
                cornerRadius = 12 * dp
                setStroke((1.5f * dp).toInt(), Color.parseColor("#FECACA"))
            }
            masterTitle.text = "Faol (Harfiy) — O‘chiq"
            masterTitle.setTextColor(Color.parseColor("#991B1B"))
            masterSubtitle.text = "Tizim sozlamalaridan yoqish uchun bu yerga bosing"
            masterSubtitle.setTextColor(Color.parseColor("#DC2626"))
        }
    }

    private fun updateStats() {
        val total = prefs.getLong(HarfiyAccessibilityService.KEY_STATS_TOTAL, 0L)
        val sh = prefs.getLong(HarfiyAccessibilityService.KEY_STATS_SH, 0L)
        val ch = prefs.getLong(HarfiyAccessibilityService.KEY_STATS_CH, 0L)
        val o = prefs.getLong(HarfiyAccessibilityService.KEY_STATS_O, 0L)
        val g = prefs.getLong(HarfiyAccessibilityService.KEY_STATS_G, 0L)
        val ng = prefs.getLong(HarfiyAccessibilityService.KEY_STATS_NG, 0L)

        // Time saved calculation: (total * 0.8s) / 60
        val minutesSaved = String.format(Locale.US, "%.1f", (total * 0.8) / 60.0)

        statsSummaryText.text = "Jami: $total ta o'zgartirish (~$minutesSaved daqiqa tejaldi)"
        statsBreakdownText.text = "ş: $sh  |  ç: $ch  |  ö: $o  |  ğ: $g  |  ñ: $ng"
    }

    private fun updateExcludedAppsSummary() {
        val excluded = prefs.getStringSet(HarfiyAccessibilityService.KEY_EXCLUDED_APPS, emptySet()) ?: emptySet()
        excludedSummaryText.text = if (excluded.isEmpty()) {
            "Hozirda istisnolar yo'q (barcha ilovalarda ishlaydi)"
        } else {
            "${excluded.size} ta dasturda Harfiy o'chirilgan"
        }
    }

    private fun showExcludedAppsDialog() {
        val currentExcluded = prefs.getStringSet(HarfiyAccessibilityService.KEY_EXCLUDED_APPS, HashSet())?.toMutableSet()
            ?: mutableSetOf()

        val pm = packageManager
        val installedApps = pm.getInstalledApplications(0)
            .filter { (it.flags and ApplicationInfo.FLAG_SYSTEM) == 0 || it.packageName == "com.google.android.apps.messaging" }
            .sortedBy { pm.getApplicationLabel(it).toString() }

        val appNames = installedApps.map { pm.getApplicationLabel(it).toString() }.toTypedArray()
        val checkedItems = installedApps.map { currentExcluded.contains(it.packageName) }.toBooleanArray()

        AlertDialog.Builder(this)
            .setTitle("Istisno dasturlar")
            .setMultiChoiceItems(appNames, checkedItems) { _, which, isChecked ->
                val pkg = installedApps[which].packageName
                if (isChecked) {
                    currentExcluded.add(pkg)
                } else {
                    currentExcluded.remove(pkg)
                }
            }
            .setPositiveButton("Saqlash") { _, _ ->
                prefs.edit().putStringSet(HarfiyAccessibilityService.KEY_EXCLUDED_APPS, currentExcluded).apply()
                updateExcludedAppsSummary()
                Toast.makeText(this, "Istisnolar saqlandi", Toast.LENGTH_SHORT).show()
            }
            .setNegativeButton("Bekor qilish", null)
            .show()
    }

    private fun isAccessibilityServiceEnabled(): Boolean {
        val am = getSystemService(Context.ACCESSIBILITY_SERVICE) as? AccessibilityManager ?: return false
        val enabledServices = am.getEnabledAccessibilityServiceList(AccessibilityServiceInfo.FEEDBACK_ALL_MASK)
        return enabledServices.any {
            it.resolveInfo.serviceInfo.packageName == packageName &&
            it.resolveInfo.serviceInfo.name.contains("HarfiyAccessibilityService")
        }
    }

    private fun fallbackConvertText(input: String, enableNg: Boolean): String {
        var res = input
        // sh -> ş
        res = res.replace("sh", "ş").replace("Sh", "Ş").replace("SH", "Ş")
        // ch -> ç
        res = res.replace("ch", "ç").replace("Ch", "Ç").replace("CH", "Ç")
        // o' -> ö
        val apos = charArrayOf('\'', '`', '‘', '’', 'ʻ', 'ʼ', '´')
        for (a in apos) {
            res = res.replace("o$a", "ö").replace("O$a", "Ö")
            res = res.replace("g$a", "ğ").replace("G$a", "Ğ")
        }
        if (enableNg) {
            res = res.replace("ng", "ñ").replace("Ng", "Ñ").replace("NG", "Ñ")
        }
        return res
    }
}
