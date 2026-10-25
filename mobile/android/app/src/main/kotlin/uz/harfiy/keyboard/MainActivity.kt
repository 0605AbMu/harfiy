package uz.harfiy.keyboard

import android.accessibilityservice.AccessibilityServiceInfo
import android.app.Activity
import android.app.AlertDialog
import android.content.ClipData
import android.content.ClipboardManager
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.SharedPreferences
import android.content.pm.ApplicationInfo
import android.graphics.Color
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.Settings
import android.view.Gravity
import android.view.ViewGroup
import android.view.accessibility.AccessibilityManager
import android.widget.*
import java.util.Locale

/**
 * Modern, clean, human-friendly dashboard for Harfiy.
 *
 * Designed with zero technical jargon so that any ordinary user can immediately
 * understand, activate, test, and customize Harfiy with maximum ease.
 */
class MainActivity : Activity() {

    private lateinit var prefs: SharedPreferences
    private var engineBridge: HarfiyEngineBridge? = null

    // UI elements requiring dynamic updates
    private lateinit var masterCard: LinearLayout
    private lateinit var masterTitle: TextView
    private lateinit var masterSubtitle: TextView
    private lateinit var switchActive: Switch

    private lateinit var statConversionsCount: TextView
    private lateinit var statTimeSaved: TextView
    private lateinit var statPillsLayout: LinearLayout
    private lateinit var excludedSubtitle: TextView

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
            fitsSystemWindows = true
            isFocusableInTouchMode = true
        }

        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            isFocusableInTouchMode = true
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
                    cornerRadius = dp(16).toFloat()
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
            setPadding(0, 0, 0, dp(2))
        }
        val appTitle = TextView(this).apply {
            text = "Harfiy"
            textSize = 28f
            setTextColor(Color.parseColor("#0F172A"))
            typeface = Typeface.DEFAULT_BOLD
        }
        val versionBadge = TextView(this).apply {
            text = "1.0.1"
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
            text = "Yozuvni avtomatik to‘g‘rilovchi aqlli yordamchi"
            textSize = 14f
            setTextColor(Color.parseColor("#64748B"))
            setPadding(0, 0, 0, dp(16))
        }
        root.addView(appSubtitle)

        // =====================================================================
        // 2. ASOSIY HOLAT (Master Status Card)
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
            textSize = 19f
            typeface = Typeface.DEFAULT_BOLD
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
                    isUpdatingUI = true
                    this.isChecked = false
                    isUpdatingUI = false

                    openAccessibilitySettings()
                } else if (!isChecked && isSystemActive) {
                    HarfiyAccessibilityService.instance?.disableService()
                    updateStatus()
                }
            }
        }
        masterRow.addView(switchActive)
        masterCard.addView(masterRow)

        masterCard.setOnClickListener {
            if (!isAccessibilityServiceEnabled()) {
                openAccessibilitySettings()
            } else {
                switchActive.performClick()
            }
        }

        // =====================================================================
        // 3. SINAB KO‘RISH MAYDONI (Interactive Playground)
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
            text = "Klaviaturangizda shahar, choy, o'rik, g'isht deb yozing:"
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
                cornerRadius = dp(10).toFloat()
            }
        }
        testCard.addView(practiceField)

        // =====================================================================
        // 4. KATTA MATNLARNI O‘GIRISH (Batch Converter)
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
            text = "Katta matnlarni o‘girish"
            textSize = 17f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor(Color.parseColor("#1E293B"))
        }
        convertTitleRow.addView(magicIcon)
        convertTitleRow.addView(convertTitle)
        convertCard.addView(convertTitleRow)

        val convertSubtitle = TextView(this).apply {
            text = "Istalgan maqola yoki xabarni yangi alifboga o‘giring:"
            textSize = 13f
            setTextColor(Color.parseColor("#64748B"))
            setPadding(0, dp(4), 0, dp(10))
        }
        convertCard.addView(convertSubtitle)

        val convertInput = EditText(this).apply {
            hint = "Matnni bu yerga qo‘ying yoki yozing..."
            textSize = 15f
            setTextColor(Color.parseColor("#0F172A"))
            setHintTextColor(Color.parseColor("#94A3B8"))
            minLines = 3
            gravity = Gravity.TOP or Gravity.START
            val padField = dp(12)
            setPadding(padField, padField, padField, padField)
            background = GradientDrawable().apply {
                setColor(Color.parseColor("#F1F5F9"))
                cornerRadius = dp(10).toFloat()
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
            text = "🪄 O‘girish"
            textSize = 14f
            setTextColor(Color.WHITE)
            typeface = Typeface.DEFAULT_BOLD
            background = GradientDrawable().apply {
                setColor(Color.parseColor("#2563EB"))
                cornerRadius = dp(10).toFloat()
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
                    Toast.makeText(this@MainActivity, "Matn to‘liq o‘girildi!", Toast.LENGTH_SHORT).show()
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
                cornerRadius = dp(10).toFloat()
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
                cornerRadius = dp(10).toFloat()
            }
            layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 0.9f)
            setOnClickListener {
                convertInput.setText("")
            }
        }
        convertBtnRow.addView(btnClear)
        convertCard.addView(convertBtnRow)

        // =====================================================================
        // 5. QULAY SOZLAMALAR (Settings)
        // =====================================================================
        val settingsCard = createCard()
        root.addView(settingsCard)

        val settingsHeader = TextView(this).apply {
            text = "Qulay sozlamalar"
            textSize = 17f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor(Color.parseColor("#1E293B"))
            setPadding(0, 0, 0, dp(6))
        }
        settingsCard.addView(settingsHeader)

        // Switch: Xatoni qaytarish
        val switchUndo = Switch(this).apply {
            text = "Xatoni qaytarish"
            textSize = 15f
            setTextColor(Color.parseColor("#334155"))
            isChecked = prefs.getBoolean(HarfiyAccessibilityService.KEY_INSTANT_UNDO, true)
            setOnCheckedChangeListener { _, isChecked ->
                prefs.edit().putBoolean(HarfiyAccessibilityService.KEY_INSTANT_UNDO, isChecked).apply()
            }
            setPadding(0, dp(6), 0, dp(4))
        }
        settingsCard.addView(switchUndo)
        val undoHint = TextView(this).apply {
            text = "Harf noto‘g‘ri o‘zgarganda, Backspace bosilsa asl holiga qaytadi"
            textSize = 12f
            setTextColor(Color.parseColor("#64748B"))
            setPadding(0, 0, 0, dp(8))
        }
        settingsCard.addView(undoHint)

        // Switch: 'ng' -> 'ñ'
        val switchNg = Switch(this).apply {
            text = "«ng» harfini almashtirish"
            textSize = 15f
            setTextColor(Color.parseColor("#334155"))
            isChecked = prefs.getBoolean(HarfiyAccessibilityService.KEY_ENABLE_NG, false)
            setOnCheckedChangeListener { _, isChecked ->
                prefs.edit().putBoolean(HarfiyAccessibilityService.KEY_ENABLE_NG, isChecked).apply()
                engineBridge?.setEnableNg(isChecked)
            }
            setPadding(0, dp(6), 0, dp(4))
        }
        settingsCard.addView(switchNg)
        val ngHint = TextView(this).apply {
            text = "«ng» yozganingizda avtomatik «ñ» ga o‘giriladi"
            textSize = 12f
            setTextColor(Color.parseColor("#64748B"))
            setPadding(0, 0, 0, dp(8))
        }
        settingsCard.addView(ngHint)

        // Switch: O'yinlarda xalaqit bermaslik
        val switchGame = Switch(this).apply {
            text = "O‘yinlarda xalaqit bermaslik"
            textSize = 15f
            setTextColor(Color.parseColor("#334155"))
            isChecked = prefs.getBoolean(HarfiyAccessibilityService.KEY_GAME_PAUSE, true)
            setOnCheckedChangeListener { _, isChecked ->
                prefs.edit().putBoolean(HarfiyAccessibilityService.KEY_GAME_PAUSE, isChecked).apply()
            }
            setPadding(0, dp(6), 0, dp(4))
        }
        settingsCard.addView(switchGame)
        val gameHint = TextView(this).apply {
            text = "O‘yin o‘ynayotganingizda Harfiy avtomatik to‘xtatiladi"
            textSize = 12f
            setTextColor(Color.parseColor("#64748B"))
            setPadding(0, 0, 0, dp(12))
        }
        settingsCard.addView(gameHint)

        // Row: Keraksiz ilovalarda o'chirish
        val excludedRow = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            val padEx = dp(12)
            setPadding(padEx, padEx, padEx, padEx)
            background = GradientDrawable().apply {
                setColor(Color.parseColor("#F8FAFC"))
                cornerRadius = dp(10).toFloat()
            }
            isClickable = true
            isFocusable = true
            setOnClickListener {
                showExcludedAppsDialog()
            }
        }
        val excludedTitleRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
        }
        val exTitle = TextView(this).apply {
            text = "Tanlangan ilovalarda o‘chirish"
            textSize = 15f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor(Color.parseColor("#1E293B"))
            layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
        }
        val exArrow = TextView(this).apply {
            text = "›"
            textSize = 20f
            setTextColor(Color.parseColor("#94A3B8"))
        }
        excludedTitleRow.addView(exTitle)
        excludedTitleRow.addView(exArrow)
        excludedRow.addView(excludedTitleRow)

        excludedSubtitle = TextView(this).apply {
            textSize = 12f
            setTextColor(Color.parseColor("#64748B"))
            setPadding(0, dp(2), 0, 0)
        }
        excludedRow.addView(excludedSubtitle)
        settingsCard.addView(excludedRow)

        // =====================================================================
        // 6. SIZNING NATIJALARINGIZ (Statistics)
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
            text = "Sizning natijalaringiz"
            textSize = 17f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor(Color.parseColor("#1E293B"))
        }
        statsTitleRow.addView(statsIcon)
        statsTitleRow.addView(statsTitle)
        statsCard.addView(statsTitleRow)

        // Metric boxes row
        val metricsRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                topMargin = dp(12)
                bottomMargin = dp(10)
            }
        }

        fun createMetricBox(valueView: TextView, labelText: String): LinearLayout {
            return LinearLayout(this).apply {
                orientation = LinearLayout.VERTICAL
                gravity = Gravity.CENTER
                val padBox = dp(12)
                setPadding(padBox, padBox, padBox, padBox)
                background = GradientDrawable().apply {
                    setColor(Color.parseColor("#F1F5F9"))
                    cornerRadius = dp(10).toFloat()
                }
                layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f).apply {
                    rightMargin = dp(4)
                    leftMargin = dp(4)
                }
                addView(valueView)
                val label = TextView(context).apply {
                    text = labelText
                    textSize = 12f
                    setTextColor(Color.parseColor("#64748B"))
                    setPadding(0, dp(2), 0, 0)
                }
                addView(label)
            }
        }

        statConversionsCount = TextView(this).apply {
            textSize = 20f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor(Color.parseColor("#2563EB"))
        }
        statTimeSaved = TextView(this).apply {
            textSize = 20f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor(Color.parseColor("#059669"))
        }
        metricsRow.addView(createMetricBox(statConversionsCount, "To‘g‘rilangan harflar"))
        metricsRow.addView(createMetricBox(statTimeSaved, "Tejalgan vaqt"))
        statsCard.addView(metricsRow)

        // Pill breakdown for characters
        statPillsLayout = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                bottomMargin = dp(8)
            }
        }
        statsCard.addView(statPillsLayout)

        val btnClearStats = TextView(this).apply {
            text = "Hisoblagichni tozalash"
            textSize = 12f
            setTextColor(Color.parseColor("#94A3B8"))
            gravity = Gravity.CENTER
            setPadding(0, dp(4), 0, dp(2))
            isClickable = true
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
                Toast.makeText(this@MainActivity, "Natijalar tozalandi", Toast.LENGTH_SHORT).show()
            }
        }
        statsCard.addView(btnClearStats)

        // =====================================================================
        // 7. BIZ BILAN BOG‘LANISH (Links & About)
        // =====================================================================
        val linksCard = createCard()
        root.addView(linksCard)

        val linksTitle = TextView(this).apply {
            text = "Biz bilan bog‘lanish"
            textSize = 17f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor(Color.parseColor("#1E293B"))
            setPadding(0, 0, 0, dp(10))
        }
        linksCard.addView(linksTitle)

        fun createLinkRow(iconRes: Int, label: String, sublabel: String, url: String): LinearLayout {
            return LinearLayout(this).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER_VERTICAL
                val padRow = dp(10)
                setPadding(padRow, padRow, padRow, padRow)
                background = GradientDrawable().apply {
                    setColor(Color.parseColor("#F8FAFC"))
                    cornerRadius = dp(10).toFloat()
                }
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.MATCH_PARENT,
                    LinearLayout.LayoutParams.WRAP_CONTENT
                ).apply {
                    bottomMargin = dp(8)
                }
                setOnClickListener {
                    try {
                        startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)))
                    } catch (_: Exception) {
                        Toast.makeText(this@MainActivity, "Havolani ochib bo‘lmadi", Toast.LENGTH_SHORT).show()
                    }
                }
                val icon = ImageView(context).apply {
                    setImageResource(iconRes)
                    layoutParams = LinearLayout.LayoutParams(dp(20), dp(20)).apply {
                        rightMargin = dp(12)
                    }
                }
                val col = LinearLayout(context).apply {
                    orientation = LinearLayout.VERTICAL
                    val t = TextView(context).apply {
                        text = label
                        textSize = 15f
                        typeface = Typeface.DEFAULT_BOLD
                        setTextColor(Color.parseColor("#1E293B"))
                    }
                    val s = TextView(context).apply {
                        text = sublabel
                        textSize = 12f
                        setTextColor(Color.parseColor("#64748B"))
                    }
                    addView(t)
                    addView(s)
                }
                addView(icon)
                addView(col)
            }
        }

        linksCard.addView(createLinkRow(R.drawable.ic_menu_web, "Rasmiy veb-sahifa", "harfiy.uz", "https://abdumannon-shamsiyev.uz/harfiy"))
        linksCard.addView(createLinkRow(R.drawable.ic_menu_blog, "Telegram kanal", "@abdumannon_notes", "https://t.me/abdumannon_notes"))
        linksCard.addView(createLinkRow(R.drawable.ic_menu_support, "Savollar va takliflar", "@abdumannon_shamsiyev", "https://t.me/abdumannon_shamsiyev"))

        // Footer note
        val footer = TextView(this).apply {
            text = "Harfiy • 100% oflayn va xavfsiz"
            textSize = 12f
            setTextColor(Color.parseColor("#94A3B8"))
            gravity = Gravity.CENTER
            setPadding(0, dp(6), 0, dp(16))
        }
        root.addView(footer)

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
        val isSamsung = Build.MANUFACTURER.contains("samsung", ignoreCase = true)
        val instructions = if (isSamsung) {
            "Harfiy telefoningiz klaviaturasida harflarni avtomatik to‘g‘rilashi uchun bitta ruxsat kerak:\n\n" +
            "1️⃣  Ochilgan oynada «O‘rnatilgan ilovalar» (Installed apps) bo‘limiga kiring.\n" +
            "2️⃣  «Harfiy»ni tanlang va yoqing.\n\n" +
            "🔒 Xavfsiz: Harfiy internetga ulanmaydi va shaxsiy ma’lumotlarni saqlamaydi."
        } else {
            "Harfiy telefoningiz klaviaturasida harflarni avtomatik to‘g‘rilashi uchun bitta ruxsat kerak:\n\n" +
            "1️⃣  «Yuklangan ilovalar» (yoki Maxsus imkoniyatlar) bo‘limiga kiring.\n" +
            "2️⃣  «Harfiy»ni tanlang va yoqing.\n\n" +
            "🔒 Xavfsiz: Harfiy internetga ulanmaydi va shaxsiy ma’lumotlarni saqlamaydi."
        }

        AlertDialog.Builder(this)
            .setTitle("Harfiy'ni yoqish")
            .setMessage(instructions)
            .setPositiveButton("Sozlamalarga o‘tish") { _, _ ->
                launchAccessibilitySettingsIntent()
            }
            .setNegativeButton("Keyinroq", null)
            .show()
    }

    private fun launchAccessibilitySettingsIntent() {
        val serviceComponent = ComponentName(packageName, HarfiyAccessibilityService::class.java.name).flattenToString()

        val deepLinkIntent = Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS).apply {
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            putExtra(":settings:fragment_args_key", serviceComponent)
            val bundle = Bundle().apply {
                putString(":settings:fragment_args_key", serviceComponent)
            }
            putExtra(":settings:show_fragment_args", bundle)
        }

        try {
            startActivity(deepLinkIntent)
            Toast.makeText(this, "«O‘rnatilgan ilovalar» -> «Harfiy»ni yoqing", Toast.LENGTH_LONG).show()
            return
        } catch (_: Exception) {
            // Fallback for custom ROMs
        }

        try {
            startActivity(Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS).apply {
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            })
            Toast.makeText(this, "«O‘rnatilgan ilovalar» -> «Harfiy»ni yoqing", Toast.LENGTH_LONG).show()
        } catch (_: Exception) {
            startActivity(Intent(Settings.ACTION_SETTINGS))
        }
    }

    private fun updateStatus() {
        val isServiceEnabled = isAccessibilityServiceEnabled()
        val dp = resources.displayMetrics.density

        isUpdatingUI = true
        switchActive.isChecked = isServiceEnabled
        isUpdatingUI = false

        if (isServiceEnabled) {
            masterCard.background = GradientDrawable().apply {
                setColor(Color.parseColor("#ECFDF5"))
                cornerRadius = 16 * dp
                setStroke((1.5f * dp).toInt(), Color.parseColor("#A7F3D0"))
            }
            masterTitle.text = "🟢  Harfiy ishlamoqda"
            masterTitle.setTextColor(Color.parseColor("#065F46"))
            masterSubtitle.text = "Klaviaturangizda yozganingizda harflar to‘g‘rilanadi"
            masterSubtitle.setTextColor(Color.parseColor("#047857"))
        } else {
            masterCard.background = GradientDrawable().apply {
                setColor(Color.parseColor("#FEF2F2"))
                cornerRadius = 16 * dp
                setStroke((1.5f * dp).toInt(), Color.parseColor("#FECACA"))
            }
            masterTitle.text = "⚪  Harfiy o‘chirilgan"
            masterTitle.setTextColor(Color.parseColor("#991B1B"))
            masterSubtitle.text = "Yoqish uchun bu yerga bosing"
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

        val minutesSaved = String.format(Locale.US, "%.1f", (total * 0.8) / 60.0)

        statConversionsCount.text = "$total ta"
        statTimeSaved.text = "~$minutesSaved daqiqa"

        // Update visual pill breakdown
        val dp = resources.displayMetrics.density
        fun dp(v: Int): Int = (v * dp).toInt()

        statPillsLayout.removeAllViews()

        fun addPill(char: String, count: Long) {
            val pill = TextView(this).apply {
                text = "$char: $count"
                textSize = 12f
                typeface = Typeface.DEFAULT_BOLD
                setTextColor(Color.parseColor("#334155"))
                setPadding(dp(8), dp(4), dp(8), dp(4))
                background = GradientDrawable().apply {
                    setColor(Color.parseColor("#F1F5F9"))
                    cornerRadius = dp(6).toFloat()
                }
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.WRAP_CONTENT,
                    LinearLayout.LayoutParams.WRAP_CONTENT
                ).apply {
                    leftMargin = dp(3)
                    rightMargin = dp(3)
                }
            }
            statPillsLayout.addView(pill)
        }

        addPill("ş", sh)
        addPill("ç", ch)
        addPill("ö", o)
        addPill("ğ", g)
        if (ng > 0 || prefs.getBoolean(HarfiyAccessibilityService.KEY_ENABLE_NG, false)) {
            addPill("ñ", ng)
        }
    }

    private fun updateExcludedAppsSummary() {
        val excluded = prefs.getStringSet(HarfiyAccessibilityService.KEY_EXCLUDED_APPS, emptySet()) ?: emptySet()
        excludedSubtitle.text = if (excluded.isEmpty()) {
            "Barcha ilovalarda ishlamoqda"
        } else {
            "${excluded.size} ta ilovada o‘chirilgan"
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
            .setTitle("Tanlangan ilovalarda o‘chirish")
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
                Toast.makeText(this, "Saqlandi", Toast.LENGTH_SHORT).show()
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
        res = res.replace("sh", "ş").replace("Sh", "Ş").replace("SH", "Ş")
        res = res.replace("ch", "ç").replace("Ch", "Ç").replace("CH", "Ç")
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
