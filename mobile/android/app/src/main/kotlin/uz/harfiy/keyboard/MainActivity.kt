@file:Suppress("SetTextI18n")

package uz.harfiy.keyboard

import android.accessibilityservice.AccessibilityServiceInfo
import android.annotation.SuppressLint
import android.app.Activity
import android.app.AlertDialog
import android.app.AppOpsManager
import android.app.Dialog
import android.app.StatusBarManager
import android.content.ClipData
import android.content.ClipboardManager
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.SharedPreferences
import android.content.pm.PackageManager
import android.content.pm.ResolveInfo
import android.graphics.Color
import android.graphics.Typeface
import android.graphics.drawable.Drawable
import android.graphics.drawable.GradientDrawable
import android.graphics.drawable.Icon
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Process
import android.provider.Settings
import android.text.Editable
import android.text.TextWatcher
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.view.Window
import android.view.accessibility.AccessibilityManager
import android.widget.*
import androidx.core.graphics.toColorInt
import androidx.core.net.toUri
import androidx.core.content.edit
import java.util.Locale

/**
 * World-Class, Ultra-Modern Mobile Dashboard for Harfiy.
 *
 * Designed with senior UI/UX standards:
 * - Grand Hero Status Card with instant visual feedback and SSOT binding.
 * - Grouped iOS/Material 3 style settings with custom icon badges and dividers.
 * - Unified Magic Text Playground & Batch Converter.
 * - Gamified impact & statistics with visual character pills.
 * - Searchable Excluded Apps picker with real launcher app icons.
 * - One-tap Google Play update checker and official community links.
 */
@SuppressLint("ApplySharedPref")
class MainActivity : Activity() {

    private lateinit var prefs: SharedPreferences

    // Dynamic UI elements
    private lateinit var heroCard: LinearLayout
    private lateinit var heroBadge: TextView
    private lateinit var heroTitle: TextView
    private lateinit var heroSubtitle: TextView
    private lateinit var heroSwitch: Switch

    private lateinit var statConversionsCount: TextView
    private lateinit var statTimeSaved: TextView
    private lateinit var statPillsLayout: LinearLayout
    private lateinit var excludedSubtitle: TextView

    // Xiaomi Autostart UI components in Settings
    private var miuiAutostartSubtitle: TextView? = null
    private var miuiAutostartStatusPill: TextView? = null
    private var activeWizardDialog: SetupWizardDialog? = null

    private var isUpdatingUI = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        prefs = getSharedPreferences(HarfiyAccessibilityService.PREFS_NAME, Context.MODE_PRIVATE)

        val dp = resources.displayMetrics.density
        fun dp(v: Int): Int = (v * dp).toInt()

        // Root ScrollView with edge-to-edge support
        val scrollView = ScrollView(this).apply {
            layoutParams = ViewGroup.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            setBackgroundColor("#F8FAFC".toColorInt()) // Soft premium canvas
            fitsSystemWindows = true
            isFocusableInTouchMode = true
        }

        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            isFocusableInTouchMode = true
            val padH = dp(20)
            val padV = dp(16)
            setPadding(padH, padV, padH, dp(72))
        }
        scrollView.addView(root)

        // Helper: Section Label (uppercase micro-header)
        fun createSectionLabel(text: String): TextView {
            return TextView(this).apply {
                this.text = text
                textSize = 12f
                typeface = Typeface.DEFAULT_BOLD
                setTextColor("#64748B".toColorInt())
                letterSpacing = 0.05f
                setPadding(dp(4), dp(16), 0, dp(8))
            }
        }

        // Helper: Rounded Card Container
        fun createCard(): LinearLayout {
            return LinearLayout(this).apply {
                orientation = LinearLayout.VERTICAL
                val pad = dp(16)
                setPadding(pad, pad, pad, pad)
                background = GradientDrawable().apply {
                    setColor(Color.WHITE)
                    cornerRadius = dp(20).toFloat()
                    setStroke(dp(1), "#E2E8F0".toColorInt())
                }
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.MATCH_PARENT,
                    LinearLayout.LayoutParams.WRAP_CONTENT
                )
            }
        }

        // =====================================================================
        // 1. TOP APP BAR
        // =====================================================================
        val appBar = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(0, dp(4), 0, dp(14))
        }

        val brandCol = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
        }

        val titleRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
        }

        val appTitle = TextView(this).apply {
            text = "Harfiy"
            textSize = 28f
            setTextColor("#0F172A".toColorInt())
            typeface = Typeface.DEFAULT_BOLD
        }
        titleRow.addView(appTitle)

        val versionBadge = TextView(this).apply {
            text = "v1.0.1 ↗"
            textSize = 12f
            setTextColor("#2563EB".toColorInt())
            typeface = Typeface.DEFAULT_BOLD
            val padH = dp(8)
            val padV = dp(3)
            setPadding(padH, padV, padH, padV)
            background = GradientDrawable().apply {
                setColor("#EFF6FF".toColorInt())
                cornerRadius = dp(8).toFloat()
            }
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                leftMargin = dp(8)
            }
            isClickable = true
            isFocusable = true
            setOnClickListener {
                openPlayStoreForUpdate()
            }
        }
        titleRow.addView(versionBadge)
        brandCol.addView(titleRow)

        val appSubtitle = TextView(this).apply {
            text = "Yozuvni avtomatik to‘g‘rilovchi aqlli yordamchi"
            textSize = 12f
            setTextColor("#64748B".toColorInt())
            setPadding(0, dp(2), 0, 0)
        }
        brandCol.addView(appSubtitle)
        appBar.addView(brandCol)
        root.addView(appBar)

        // =====================================================================
        // 2. GRAND HERO STATUS CARD (Asosiy Holat)
        // =====================================================================
        heroCard = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            val pad = dp(20)
            setPadding(pad, pad, pad, pad)
            background = GradientDrawable().apply {
                setColor(Color.WHITE)
                cornerRadius = dp(24).toFloat()
                setStroke(dp(1), "#E2E8F0".toColorInt())
            }
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                bottomMargin = dp(4)
            }
            isClickable = true
            isFocusable = true
            setOnClickListener {
                if (!isAccessibilityServiceEnabled()) {
                    openAccessibilitySettings()
                } else {
                    val isPaused = HarfiyAccessibilityService.isPaused(this@MainActivity)
                    HarfiyAccessibilityService.setPaused(this@MainActivity, !isPaused)
                    updateStatus()
                }
            }
        }
        root.addView(heroCard)

        // Hero Status Badge Pill
        heroBadge = TextView(this).apply {
            textSize = 11f
            typeface = Typeface.DEFAULT_BOLD
            val padH = dp(10)
            val padV = dp(4)
            setPadding(padH, padV, padH, padV)
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                bottomMargin = dp(10)
            }
        }
        heroCard.addView(heroBadge)

        // Hero Main Row (Title + Switch)
        val heroMainRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
        }

        val heroTextCol = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f).apply {
                rightMargin = dp(12)
            }
        }

        heroTitle = TextView(this).apply {
            textSize = 22f
            typeface = Typeface.DEFAULT_BOLD
        }
        heroTextCol.addView(heroTitle)

        heroSubtitle = TextView(this).apply {
            textSize = 13f
            setPadding(0, dp(4), 0, 0)
        }
        heroTextCol.addView(heroSubtitle)
        heroMainRow.addView(heroTextCol)

        heroSwitch = Switch(this).apply {
            isClickable = true
            scaleX = 1.15f
            scaleY = 1.15f
            setOnCheckedChangeListener { _, isChecked ->
                if (isUpdatingUI) return@setOnCheckedChangeListener

                val isSystemActive = isAccessibilityServiceEnabled()
                if (isChecked && !isSystemActive) {
                    isUpdatingUI = true
                    this.isChecked = false
                    isUpdatingUI = false

                    openAccessibilitySettings()
                } else if (isSystemActive) {
                    HarfiyAccessibilityService.setPaused(this@MainActivity, !isChecked)
                    updateStatus()
                }
            }
        }
        heroMainRow.addView(heroSwitch)
        heroCard.addView(heroMainRow)

        // =====================================================================
        // 3. GROUPED SETTINGS (iOS/Material 3 uslubidagi sozlamalar)
        // =====================================================================
        root.addView(createSectionLabel("QULAY SOZLAMALAR"))

        val settingsCard = createCard()
        root.addView(settingsCard)

        // Helper: Settings Row with Icon Badge
        fun createSettingSwitchRow(
            iconEmoji: String,
            iconBg: String,
            title: String,
            subtitle: String,
            isCheckedInit: Boolean,
            onChecked: (Boolean) -> Unit
        ): LinearLayout {
            val row = LinearLayout(this).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER_VERTICAL
                val padV = dp(10)
                setPadding(0, padV, 0, padV)
            }

            val badge = TextView(this).apply {
                text = iconEmoji
                textSize = 16f
                gravity = Gravity.CENTER
                layoutParams = LinearLayout.LayoutParams(dp(36), dp(36)).apply {
                    rightMargin = dp(12)
                }
                background = GradientDrawable().apply {
                    setColor(iconBg.toColorInt())
                    cornerRadius = dp(10).toFloat()
                }
            }
            row.addView(badge)

            val textCol = LinearLayout(this).apply {
                orientation = LinearLayout.VERTICAL
                layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f).apply {
                    rightMargin = dp(8)
                }
                val t = TextView(context).apply {
                    text = title
                    textSize = 15f
                    typeface = Typeface.DEFAULT_BOLD
                    setTextColor("#0F172A".toColorInt())
                }
                val s = TextView(context).apply {
                    text = subtitle
                    textSize = 12f
                    setTextColor("#64748B".toColorInt())
                    setPadding(0, dp(1), 0, 0)
                }
                addView(t)
                addView(s)
            }
            row.addView(textCol)

            val sw = Switch(this).apply {
                isChecked = isCheckedInit
                setOnCheckedChangeListener { _, checked -> onChecked(checked) }
            }
            row.addView(sw)

            return row
        }

        fun createDivider(): View {
            return View(this).apply {
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.MATCH_PARENT,
                    dp(1)
                ).apply {
                    leftMargin = dp(48)
                }
                setBackgroundColor("#F1F5F9".toColorInt())
            }
        }

        // Setting 1: Xatoni qaytarish
        settingsCard.addView(
            createSettingSwitchRow(
                "↩️",
                "#EFF6FF",
                "Xatoni qaytarish",
                "Harf noto‘g‘ri o‘zgarganda, Backspace bosilsa asl holiga qaytadi",
                prefs.getBoolean(HarfiyAccessibilityService.KEY_INSTANT_UNDO, true)
            ) { isChecked ->
                prefs.edit(commit = true) { putBoolean(HarfiyAccessibilityService.KEY_INSTANT_UNDO, isChecked) }
            }
        )
        settingsCard.addView(createDivider())

        // Setting 2: «ng» harfi
        settingsCard.addView(
            createSettingSwitchRow(
                "🔤",
                "#F5F3FF",
                "«ng» harfini almashtirish",
                "«ng» yozganingizda avtomatik «ñ» ga o‘giriladi",
                prefs.getBoolean(HarfiyAccessibilityService.KEY_ENABLE_NG, false)
            ) { isChecked ->
                prefs.edit(commit = true) { putBoolean(HarfiyAccessibilityService.KEY_ENABLE_NG, isChecked) }
            }
        )
        settingsCard.addView(createDivider())

        // Setting 3: O'yinlarda to'xtatish
        settingsCard.addView(
            createSettingSwitchRow(
                "🎮",
                "#FEF3C7",
                "O‘yinlarda xalaqit bermaslik",
                "O‘yin o‘ynayotganingizda Harfiy avtomatik to‘xtatiladi",
                prefs.getBoolean(HarfiyAccessibilityService.KEY_GAME_PAUSE, true)
            ) { isChecked ->
                prefs.edit(commit = true) { putBoolean(HarfiyAccessibilityService.KEY_GAME_PAUSE, isChecked) }
            }
        )
        settingsCard.addView(createDivider())

        // Setting 4: Tanlangan ilovalarda o'chirish (Searchable picker)
        val excludedRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            val padV = dp(10)
            setPadding(0, padV, 0, padV)
            isClickable = true
            isFocusable = true
            setOnClickListener {
                showExcludedAppsDialog()
            }
        }

        val exBadge = TextView(this).apply {
            text = "🚫"
            textSize = 16f
            gravity = Gravity.CENTER
            layoutParams = LinearLayout.LayoutParams(dp(36), dp(36)).apply {
                rightMargin = dp(12)
            }
            background = GradientDrawable().apply {
                setColor("#FEE2E2".toColorInt())
                cornerRadius = dp(10).toFloat()
            }
        }
        excludedRow.addView(exBadge)

        val exTextCol = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
            val t = TextView(context).apply {
                text = "Tanlangan ilovalarda o‘chirish"
                textSize = 15f
                typeface = Typeface.DEFAULT_BOLD
                setTextColor("#0F172A".toColorInt())
            }
            excludedSubtitle = TextView(context).apply {
                textSize = 12f
                setTextColor("#64748B".toColorInt())
                setPadding(0, dp(1), 0, 0)
            }
            addView(t)
            addView(excludedSubtitle)
        }
        excludedRow.addView(exTextCol)

        val exArrow = TextView(this).apply {
            text = "›"
            textSize = 22f
            setTextColor("#94A3B8".toColorInt())
            setPadding(dp(4), 0, dp(4), 0)
        }
        excludedRow.addView(exArrow)
        settingsCard.addView(excludedRow)
        settingsCard.addView(createDivider())

        // Setting 5: Yuqoridan tushuvchi sozlamalar menyusi (Quick Settings Tile)
        val tileRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            val padV = dp(10)
            setPadding(0, padV, 0, padV)
            isClickable = true
            isFocusable = true
            setOnClickListener {
                promptAddQuickSettingsTile()
            }
        }

        val tileBadge = TextView(this).apply {
            text = "⚡"
            textSize = 16f
            gravity = Gravity.CENTER
            layoutParams = LinearLayout.LayoutParams(dp(36), dp(36)).apply {
                rightMargin = dp(12)
            }
            background = GradientDrawable().apply {
                setColor("#FEF3C7".toColorInt())
                cornerRadius = dp(10).toFloat()
            }
        }
        tileRow.addView(tileBadge)

        val tileTextCol = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f).apply {
                rightMargin = dp(8)
            }
            val t = TextView(context).apply {
                text = "Tezkor boshqarish tugmasi (ixtiyoriy)"
                textSize = 15f
                typeface = Typeface.DEFAULT_BOLD
                setTextColor("#0F172A".toColorInt())
            }
            val s = TextView(context).apply {
                text = "Yuqori tezkor paneldan to‘xtatishni xohlovchilar uchun"
                textSize = 12f
                setTextColor("#64748B".toColorInt())
                setPadding(0, dp(1), 0, 0)
            }
            addView(t)
            addView(s)
        }
        tileRow.addView(tileTextCol)

        val tileArrow = TextView(this).apply {
            text = "›"
            textSize = 22f
            setTextColor("#94A3B8".toColorInt())
            setPadding(dp(4), 0, dp(4), 0)
        }
        tileRow.addView(tileArrow)
        settingsCard.addView(tileRow)
        settingsCard.addView(createDivider())

        // Setting: Fonda avtomatik ishlash (Autostart) specifically for Xiaomi / Redmi / Poco
        if (isXiaomiOrMiui()) {
            val autostartRow = LinearLayout(this).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER_VERTICAL
                val padV = dp(10)
                setPadding(0, padV, 0, padV)
                isClickable = true
                isFocusable = true
                setOnClickListener {
                    openXiaomiAutostartSettings()
                }
            }

            val autoBadge = TextView(this).apply {
                text = "🚀"
                textSize = 16f
                gravity = Gravity.CENTER
                layoutParams = LinearLayout.LayoutParams(dp(36), dp(36)).apply {
                    rightMargin = dp(12)
                }
                background = GradientDrawable().apply {
                    setColor("#EFF6FF".toColorInt())
                    cornerRadius = dp(10).toFloat()
                }
            }
            autostartRow.addView(autoBadge)

            val autoTextCol = LinearLayout(this).apply {
                orientation = LinearLayout.VERTICAL
                layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
                val t = TextView(context).apply {
                    text = "Fonda avtomatik ishlash (Autostart)"
                    textSize = 15f
                    typeface = Typeface.DEFAULT_BOLD
                    setTextColor("#0F172A".toColorInt())
                }
                miuiAutostartSubtitle = TextView(context).apply {
                    textSize = 12f
                    setTextColor("#64748B".toColorInt())
                    setPadding(0, dp(1), 0, 0)
                }
                addView(t)
                addView(miuiAutostartSubtitle)
            }
            autostartRow.addView(autoTextCol)

            miuiAutostartStatusPill = TextView(this).apply {
                textSize = 12f
                typeface = Typeface.DEFAULT_BOLD
                val padH = dp(8)
                val padV = dp(4)
                setPadding(padH, padV, padH, padV)
            }
            autostartRow.addView(miuiAutostartStatusPill)

            val autoArrow = TextView(this).apply {
                text = "›"
                textSize = 22f
                setTextColor("#94A3B8".toColorInt())
                setPadding(dp(4), 0, dp(4), 0)
            }
            autostartRow.addView(autoArrow)
            settingsCard.addView(autostartRow)
            settingsCard.addView(createDivider())
        }

        // Setting 6: Boshlang'ich sozlash ustasi (Wizard)
        val wizardRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            val padV = dp(10)
            setPadding(0, padV, 0, padV)
            isClickable = true
            isFocusable = true
            setOnClickListener {
                if (activeWizardDialog?.isShowing == true) return@setOnClickListener
                activeWizardDialog = SetupWizardDialog(this@MainActivity, initialStep = 1) {
                    updateStatus()
                    activeWizardDialog = null
                }.apply {
                    setOnDismissListener {
                        activeWizardDialog = null
                        updateStatus()
                    }
                    show()
                }
            }
        }

        val wizardBadge = TextView(this).apply {
            text = "🧭"
            textSize = 16f
            gravity = Gravity.CENTER
            layoutParams = LinearLayout.LayoutParams(dp(36), dp(36)).apply {
                rightMargin = dp(12)
            }
            background = GradientDrawable().apply {
                setColor("#E0F2FE".toColorInt())
                cornerRadius = dp(10).toFloat()
            }
        }
        wizardRow.addView(wizardBadge)

        val wizardTextCol = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f).apply {
                rightMargin = dp(8)
            }
            val t = TextView(context).apply {
                text = "Boshlang‘ich sozlash ustasi (Wizard)"
                textSize = 15f
                typeface = Typeface.DEFAULT_BOLD
                setTextColor("#0F172A".toColorInt())
            }
            val s = TextView(context).apply {
                text = "Harfiy va tezkor boshqarish tugmasini ishlatish qo‘llanmasi"
                textSize = 12f
                setTextColor("#64748B".toColorInt())
                setPadding(0, dp(1), 0, 0)
            }
            addView(t)
            addView(s)
        }
        wizardRow.addView(wizardTextCol)

        val wizardArrow = TextView(this).apply {
            text = "›"
            textSize = 22f
            setTextColor("#94A3B8".toColorInt())
            setPadding(dp(4), 0, dp(4), 0)
        }
        wizardRow.addView(wizardArrow)
        settingsCard.addView(wizardRow)

        // =====================================================================
        // 4. SEHRLI MATN MAYDONI VA SINOV (Unified Playground & Converter)
        // =====================================================================
        root.addView(createSectionLabel("MATN MAYDONI VA SINOV"))

        val convertCard = createCard()
        root.addView(convertCard)

        val convertHeaderRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(0, 0, 0, dp(4))
        }
        val magicIcon = ImageView(this).apply {
            setImageResource(R.drawable.ic_magic)
            layoutParams = LinearLayout.LayoutParams(dp(20), dp(20)).apply {
                rightMargin = dp(8)
            }
        }
        val convertTitle = TextView(this).apply {
            text = "Yozib ko‘ring yoki matnni o‘giring"
            textSize = 16f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor("#0F172A".toColorInt())
        }
        convertHeaderRow.addView(magicIcon)
        convertHeaderRow.addView(convertTitle)
        convertCard.addView(convertHeaderRow)

        val convertSubtitle = TextView(this).apply {
            text = "Klaviaturangizda «sh, ch, o', g'» deb yozing yoki nusxalangan matnni qo‘ying:"
            textSize = 13f
            setTextColor("#64748B".toColorInt())
            setPadding(0, 0, 0, dp(10))
        }
        convertCard.addView(convertSubtitle)

        val convertInput = EditText(this).apply {
            hint = "Yozing (masalan: shahar, choy, o‘rik) yoki matn nusxasini qo‘ying..."
            textSize = 15f
            setTextColor("#0F172A".toColorInt())
            setHintTextColor("#94A3B8".toColorInt())
            minLines = 3
            gravity = Gravity.TOP or Gravity.START
            val padField = dp(12)
            setPadding(padField, padField, padField, padField)
            background = GradientDrawable().apply {
                setColor("#F8FAFC".toColorInt())
                cornerRadius = dp(14).toFloat()
                setStroke(dp(1), "#E2E8F0".toColorInt())
            }
        }
        convertCard.addView(convertInput)

        // Action Buttons Row
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
            textSize = 13f
            setTextColor(Color.WHITE)
            typeface = Typeface.DEFAULT_BOLD
            isAllCaps = false
            includeFontPadding = false
            maxLines = 1
            minHeight = 0
            minimumHeight = 0
            setPadding(dp(6), 0, dp(6), 0)
            gravity = Gravity.CENTER
            background = GradientDrawable().apply {
                setColor("#2563EB".toColorInt())
                cornerRadius = dp(12).toFloat()
            }
            layoutParams = LinearLayout.LayoutParams(0, dp(44), 1.15f).apply {
                rightMargin = dp(6)
            }
            setOnClickListener {
                val input = convertInput.text.toString()
                if (input.isNotEmpty()) {
                    val enableNg = prefs.getBoolean(HarfiyAccessibilityService.KEY_ENABLE_NG, false)
                    val converted = HarfiyEngine.convert(input, enableNg)
                    convertInput.setText(converted)
                    convertInput.setSelection(converted.length)
                    Toast.makeText(this@MainActivity, "Matn to‘liq o‘girildi!", Toast.LENGTH_SHORT).show()
                }
            }
        }
        convertBtnRow.addView(btnDoConvert)

        val btnCopy = Button(this).apply {
            text = "📋 Nusxa"
            textSize = 13f
            setTextColor("#1E293B".toColorInt())
            typeface = Typeface.DEFAULT_BOLD
            isAllCaps = false
            includeFontPadding = false
            maxLines = 1
            minHeight = 0
            minimumHeight = 0
            setPadding(dp(6), 0, dp(6), 0)
            gravity = Gravity.CENTER
            background = GradientDrawable().apply {
                setColor("#F1F5F9".toColorInt())
                cornerRadius = dp(12).toFloat()
                setStroke(dp(1), "#E2E8F0".toColorInt())
            }
            layoutParams = LinearLayout.LayoutParams(0, dp(44), 1.05f).apply {
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
            setTextColor("#64748B".toColorInt())
            typeface = Typeface.DEFAULT_BOLD
            isAllCaps = false
            includeFontPadding = false
            maxLines = 1
            minHeight = 0
            minimumHeight = 0
            setPadding(dp(6), 0, dp(6), 0)
            gravity = Gravity.CENTER
            background = GradientDrawable().apply {
                setColor("#F8FAFC".toColorInt())
                cornerRadius = dp(12).toFloat()
                setStroke(dp(1), "#E2E8F0".toColorInt())
            }
            layoutParams = LinearLayout.LayoutParams(0, dp(44), 0.95f)
            setOnClickListener {
                convertInput.setText("")
            }
        }
        convertBtnRow.addView(btnClear)
        convertCard.addView(convertBtnRow)

        // =====================================================================
        // 5. GAMIFIED IMPACT & STATISTICS (Sizning natijalaringiz)
        // =====================================================================
        root.addView(createSectionLabel("SIZNING NATIJALARINGIZ"))

        val statsCard = createCard()
        root.addView(statsCard)

        val statsTitleRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(0, 0, 0, dp(12))
        }
        val statsIcon = ImageView(this).apply {
            setImageResource(R.drawable.ic_stats)
            layoutParams = LinearLayout.LayoutParams(dp(20), dp(20)).apply {
                rightMargin = dp(8)
            }
        }
        val statsTitle = TextView(this).apply {
            text = "Harfiy bilan tejalgan vaqtingiz"
            textSize = 16f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor("#0F172A".toColorInt())
        }
        statsTitleRow.addView(statsIcon)
        statsTitleRow.addView(statsTitle)
        statsCard.addView(statsTitleRow)

        // 2 Big Metric Showcase Boxes
        val metricsRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                bottomMargin = dp(12)
            }
        }

        fun createMetricBox(valueView: TextView, labelText: String, bgTint: String): LinearLayout {
            return LinearLayout(this).apply {
                orientation = LinearLayout.VERTICAL
                gravity = Gravity.CENTER
                val padH = dp(8)
                val padV = dp(12)
                setPadding(padH, padV, padH, padV)
                background = GradientDrawable().apply {
                    setColor(bgTint.toColorInt())
                    cornerRadius = dp(14).toFloat()
                }
                layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f).apply {
                    rightMargin = dp(4)
                    leftMargin = dp(4)
                }
                addView(valueView)
                val label = TextView(context).apply {
                    text = labelText
                    textSize = 11.5f
                    gravity = Gravity.CENTER
                    includeFontPadding = false
                    setTextColor("#64748B".toColorInt())
                    setPadding(0, dp(3), 0, 0)
                }
                addView(label)
            }
        }

        statConversionsCount = TextView(this).apply {
            textSize = 21f
            typeface = Typeface.DEFAULT_BOLD
            maxLines = 1
            includeFontPadding = false
            setTextColor("#2563EB".toColorInt())
        }
        statTimeSaved = TextView(this).apply {
            textSize = 21f
            typeface = Typeface.DEFAULT_BOLD
            maxLines = 1
            includeFontPadding = false
            setTextColor("#059669".toColorInt())
        }
        metricsRow.addView(createMetricBox(statConversionsCount, "To‘g‘rilangan harflar", "#EFF6FF"))
        metricsRow.addView(createMetricBox(statTimeSaved, "Tejalgan vaqt", "#ECFDF5"))
        statsCard.addView(metricsRow)

        // Pills Row for individual characters
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
            setTextColor("#94A3B8".toColorInt())
            gravity = Gravity.CENTER
            setPadding(0, dp(4), 0, dp(2))
            isClickable = true
            setOnClickListener {
                HarfiyStats.reset(this@MainActivity)
                updateStats()
                Toast.makeText(this@MainActivity, "Natijalar tozalandi", Toast.LENGTH_SHORT).show()
            }
        }
        statsCard.addView(btnClearStats)

        // =====================================================================
        // 6. LOYIHA VA HAMJAMIYAT (Updates & Community Links)
        // =====================================================================
        root.addView(createSectionLabel("LOYIHA VA HAMJAMIYAT"))

        val linksCard = createCard()
        root.addView(linksCard)

        fun createLinkRow(
            iconRes: Int,
            badgeBgColor: String,
            title: String,
            subtitle: String,
            url: String
        ): LinearLayout {
            return LinearLayout(this).apply {
                orientation = LinearLayout.HORIZONTAL
                gravity = Gravity.CENTER_VERTICAL
                val padRow = dp(12)
                setPadding(padRow, padRow, padRow, padRow)
                background = GradientDrawable().apply {
                    setColor("#F8FAFC".toColorInt())
                    cornerRadius = dp(14).toFloat()
                    setStroke(dp(1), "#E2E8F0".toColorInt())
                }
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.MATCH_PARENT,
                    LinearLayout.LayoutParams.WRAP_CONTENT
                ).apply {
                    bottomMargin = dp(10)
                }
                isClickable = true
                isFocusable = true
                setOnClickListener {
                    if (url.startsWith("market://")) {
                        openPlayStoreForUpdate()
                    } else {
                        try {
                            startActivity(Intent(Intent.ACTION_VIEW, url.toUri()))
                        } catch (_: Exception) {
                            Toast.makeText(this@MainActivity, "Havolani ochib bo‘lmadi", Toast.LENGTH_SHORT).show()
                        }
                    }
                }

                // Modern visual icon badge
                val badge = FrameLayout(context).apply {
                    layoutParams = LinearLayout.LayoutParams(dp(44), dp(44)).apply {
                        rightMargin = dp(14)
                    }
                    background = GradientDrawable().apply {
                        setColor(badgeBgColor.toColorInt())
                        cornerRadius = dp(12).toFloat()
                    }
                    val icon = ImageView(context).apply {
                        setImageResource(iconRes)
                        layoutParams = FrameLayout.LayoutParams(dp(24), dp(24), Gravity.CENTER)
                    }
                    addView(icon)
                }
                addView(badge)

                // Text column
                val col = LinearLayout(context).apply {
                    orientation = LinearLayout.VERTICAL
                    layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
                    val t = TextView(context).apply {
                        text = title
                        textSize = 15f
                        typeface = Typeface.DEFAULT_BOLD
                        setTextColor("#0F172A".toColorInt())
                    }
                    val s = TextView(context).apply {
                        text = subtitle
                        textSize = 12f
                        setTextColor("#64748B".toColorInt())
                        setPadding(0, dp(2), 0, 0)
                    }
                    addView(t)
                    addView(s)
                }
                addView(col)

                // Chevron
                val arrow = TextView(context).apply {
                    text = "›"
                    textSize = 22f
                    setTextColor("#94A3B8".toColorInt())
                    setPadding(dp(4), 0, dp(4), 0)
                }
                addView(arrow)
            }
        }

        linksCard.addView(
            createLinkRow(
                R.drawable.ic_link_update,
                "#F5F3FF",
                "Yangilanishlarni tekshirish",
                "Google Play Store • Joriy versiya: 1.0.1",
                "market://details?id=$packageName"
            )
        )
        linksCard.addView(
            createLinkRow(
                R.drawable.ic_link_globe,
                "#EFF6FF",
                "Rasmiy veb-sahifa",
                "harfiy.uz • Yangiliklar va qo‘llanma",
                "https://abdumannon-shamsiyev.uz/harfiy"
            )
        )
        linksCard.addView(
            createLinkRow(
                R.drawable.ic_link_telegram,
                "#E0F2FE",
                "Telegram kanal",
                "@abdumannon_notes • Muallif blogi",
                "https://t.me/abdumannon_notes"
            )
        )
        linksCard.addView(
            createLinkRow(
                R.drawable.ic_link_support,
                "#ECFDF5",
                "Savollar va takliflar",
                "@abdumannon_shamsiyev • To‘g‘ridan-to‘g‘ri aloqa",
                "https://t.me/abdumannon_shamsiyev"
            )
        )

        // Footer note
        val footer = TextView(this).apply {
            text = "Harfiy • 100% oflayn va xavfsiz\nO‘zbekiston uchun mehr bilan yaratildi ❤️"
            textSize = 12f
            setTextColor("#94A3B8".toColorInt())
            gravity = Gravity.CENTER
            setPadding(0, dp(14), 0, dp(24))
        }
        root.addView(footer)

        setContentView(scrollView)
    }

    override fun onResume() {
        super.onResume()
        updateStatus()
        updateStats()
        updateExcludedAppsSummary()
        checkAndPromptIncompleteSteps()
    }

    private fun checkAndPromptIncompleteSteps() {
        if (isFinishing || isDestroyed) return
        if (activeWizardDialog?.isShowing == true) return

        val isAccessibilityMissing = !isAccessibilityServiceEnabled()
        val isAutostartMissing = isXiaomiOrMiui() && !isXiaomiAutostartEnabled()
        val isWizardNeverCompleted = !prefs.getBoolean("has_completed_wizard", false)

        val targetStep = when {
            isAccessibilityMissing -> 2
            isAutostartMissing -> 3
            isWizardNeverCompleted -> 1
            else -> return // Barcha majburiy sozlamalar to‘liq bajarilgan!
        }

        window.decorView.post {
            if (isFinishing || isDestroyed) return@post
            if (activeWizardDialog?.isShowing == true) return@post

            activeWizardDialog = SetupWizardDialog(this@MainActivity, initialStep = targetStep) {
                updateStatus()
                activeWizardDialog = null
            }.apply {
                setOnDismissListener {
                    activeWizardDialog = null
                    updateStatus()
                }
                show()
            }
        }
    }

    override fun onDestroy() {
        super.onDestroy()
        activeWizardDialog?.dismiss()
        activeWizardDialog = null
    }

    private fun promptAddQuickSettingsTile() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            val statusBarManager = getSystemService(StatusBarManager::class.java)
            statusBarManager.requestAddTileService(
                ComponentName(this, HarfiyTileService::class.java),
                "Harfiy",
                Icon.createWithResource(this, R.drawable.ic_tile_harfiy),
                mainExecutor
            ) { result ->
                when (result) {
                    StatusBarManager.TILE_ADD_REQUEST_RESULT_TILE_ADDED -> {
                        Toast.makeText(this, "Harfiy tezkor menyuga muvaffaqiyatli qo‘shildi!", Toast.LENGTH_SHORT).show()
                    }
                    StatusBarManager.TILE_ADD_REQUEST_RESULT_TILE_ALREADY_ADDED -> {
                        Toast.makeText(this, "Harfiy allaqachon tezkor menyuda mavjud", Toast.LENGTH_SHORT).show()
                    }
                }
            }
        } else {
            AlertDialog.Builder(this)
                .setTitle("Tezkor boshqarish tugmasi")
                .setMessage("Harfiyni yuqori tezkor paneldan 1 marta bosishda yoqish yoki to‘xtatish uchun:\n\n1. Ekranning yuqori qismidan 2 marta pastga torting\n2. Qalamcha (Tahrirlash) belgisini bosing\n3. 'Harfiy' tugmasini faol tugmalar qatoriga surib qo‘ying.")
                .setPositiveButton("Tushunarli", null)
                .show()
        }
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

    private fun openPlayStoreForUpdate() {
        val appPackageName = packageName
        try {
            startActivity(Intent(Intent.ACTION_VIEW, "market://details?id=$appPackageName".toUri()).apply {
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            })
        } catch (_: Exception) {
            try {
                startActivity(Intent(Intent.ACTION_VIEW, "https://play.google.com/store/apps/details?id=$appPackageName".toUri()).apply {
                    addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                })
            } catch (_: Exception) {
                Toast.makeText(this, "Google Play Store ochilmadi", Toast.LENGTH_SHORT).show()
            }
        }
    }

    private fun updateStatus() {
        val isServiceEnabled = isAccessibilityServiceEnabled()
        val isPaused = HarfiyAccessibilityService.isPaused(this)
        val dp = resources.displayMetrics.density

        isUpdatingUI = true
        heroSwitch.isChecked = isServiceEnabled && !isPaused
        isUpdatingUI = false

        if (isServiceEnabled && !isPaused) {
            heroCard.background = GradientDrawable().apply {
                setColor("#ECFDF5".toColorInt()) // Soft emerald tint
                cornerRadius = 24 * dp
                setStroke((1.5f * dp).toInt(), "#A7F3D0".toColorInt())
            }
            heroBadge.text = "🟢  TIZIMDA FAOL"
            heroBadge.setTextColor("#047857".toColorInt())
            heroBadge.background = GradientDrawable().apply {
                setColor("#D1FAE5".toColorInt())
                cornerRadius = 10 * dp
            }
            heroTitle.text = "Harfiy ishlamoqda"
            heroTitle.setTextColor("#065F46".toColorInt())
            heroSubtitle.text = "Klaviaturangizda yangi alifbo avtomatik to‘g‘rilanadi"
            heroSubtitle.setTextColor("#047857".toColorInt())
        } else if (isServiceEnabled) {
            heroCard.background = GradientDrawable().apply {
                setColor("#FFFBEB".toColorInt()) // Soft amber tint
                cornerRadius = 24 * dp
                setStroke((1.5f * dp).toInt(), "#FDE68A".toColorInt())
            }
            heroBadge.text = "🟡  VAQTINCHALIK TO‘XTATILGAN (PAUZA)"
            heroBadge.setTextColor("#B45309".toColorInt())
            heroBadge.background = GradientDrawable().apply {
                setColor("#FEF3C7".toColorInt())
                cornerRadius = 10 * dp
            }
            heroTitle.text = "Harfiy to‘xtatib turilibdi"
            heroTitle.setTextColor("#92400E".toColorInt())
            heroSubtitle.text = "Qayta faollashtirish uchun bosing yoki yuqori menyudan yoqing"
            heroSubtitle.setTextColor("#B45309".toColorInt())
        } else {
            heroCard.background = GradientDrawable().apply {
                setColor("#FFF1F2".toColorInt()) // Soft rose tint
                cornerRadius = 24 * dp
                setStroke((1.5f * dp).toInt(), "#FECDD3".toColorInt())
            }
            heroBadge.text = "⚪  TIZIMDA TO‘XTATILGAN"
            heroBadge.setTextColor("#BE123C".toColorInt())
            heroBadge.background = GradientDrawable().apply {
                setColor("#FFE4E6".toColorInt())
                cornerRadius = 10 * dp
            }
            heroTitle.text = "Harfiy o‘chirilgan"
            heroTitle.setTextColor("#9F1239".toColorInt())
            heroSubtitle.text = "Klaviaturada harflarni to‘g‘rilash uchun bosing"
            heroSubtitle.setTextColor("#BE123C".toColorInt())
        }

        // Dynamically update Xiaomi Autostart status card
        // Dynamically update Xiaomi Autostart setting row in Qulay sozlamalar
        if (isXiaomiOrMiui() && miuiAutostartSubtitle != null && miuiAutostartStatusPill != null) {
            val isAutostartGranted = isXiaomiAutostartEnabled()
            if (isAutostartGranted) {
                miuiAutostartSubtitle?.apply {
                    text = "Faol — Harfiy ilovalar yopilganda ham to‘xtamaydi"
                    setTextColor("#047857".toColorInt())
                }
                miuiAutostartStatusPill?.apply {
                    text = "🟢 Yoqilgan"
                    setTextColor("#047857".toColorInt())
                    background = GradientDrawable().apply {
                        setColor("#D1FAE5".toColorInt())
                        cornerRadius = 8 * dp
                    }
                }
            } else {
                miuiAutostartSubtitle?.apply {
                    text = "Xiaomi to‘xtatib qo‘ymasligi uchun yoqing"
                    setTextColor("#B45309".toColorInt())
                }
                miuiAutostartStatusPill?.apply {
                    text = "🔴 Ruxsat kerak"
                    setTextColor("#B91C1C".toColorInt())
                    background = GradientDrawable().apply {
                        setColor("#FEE2E2".toColorInt())
                        cornerRadius = 8 * dp
                    }
                }
            }
        }
    }

    private fun updateStats() {
        val counts = HarfiyStats.snapshot(this)
        val sh = counts[HarfiyEngine.RuleKind.SH.ordinal]
        val ch = counts[HarfiyEngine.RuleKind.CH.ordinal]
        val o = counts[HarfiyEngine.RuleKind.O.ordinal]
        val g = counts[HarfiyEngine.RuleKind.G.ordinal]
        val ng = counts[HarfiyEngine.RuleKind.NG.ordinal]
        val total = counts.sum()

        val minutesSaved = String.format(Locale.US, "%.1f", (total * 0.8) / 60.0)

        statConversionsCount.text = "$total ta"
        statTimeSaved.text = "~$minutesSaved daq"

        // Update visual pill breakdown
        val dp = resources.displayMetrics.density
        fun dp(v: Int): Int = (v * dp).toInt()

        statPillsLayout.removeAllViews()

        fun addPill(char: String, count: Long) {
            val pill = TextView(this).apply {
                text = "$char: $count"
                textSize = 12f
                typeface = Typeface.DEFAULT_BOLD
                setTextColor("#334155".toColorInt())
                setPadding(dp(10), dp(5), dp(10), dp(5))
                background = GradientDrawable().apply {
                    setColor("#F1F5F9".toColorInt())
                    cornerRadius = dp(8).toFloat()
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
            "${excluded.size} ta ilovada to‘xtatilgan"
        }
    }

    private data class ExcludedAppItem(
        val packageName: String,
        val label: String,
        val resolveInfo: ResolveInfo
    ) {
        var cachedIcon: Drawable? = null
        fun loadIcon(pm: PackageManager): Drawable {
            if (cachedIcon == null) {
                cachedIcon = resolveInfo.loadIcon(pm)
            }
            return cachedIcon!!
        }
    }

    private fun showExcludedAppsDialog() {
        val currentExcluded = prefs.getStringSet(HarfiyAccessibilityService.KEY_EXCLUDED_APPS, HashSet())?.toMutableSet()
            ?: mutableSetOf()

        val pm = packageManager
        val mainIntent = Intent(Intent.ACTION_MAIN, null).apply {
            addCategory(Intent.CATEGORY_LAUNCHER)
        }
        val resolveList = pm.queryIntentActivities(mainIntent, 0)
        val allApps = resolveList
            .filter { it.activityInfo.packageName != packageName }
            .distinctBy { it.activityInfo.packageName }
            .map {
                ExcludedAppItem(
                    packageName = it.activityInfo.packageName,
                    label = it.loadLabel(pm).toString(),
                    resolveInfo = it
                )
            }
            .sortedBy { it.label.lowercase(Locale.getDefault()) }

        if (allApps.isEmpty()) {
            Toast.makeText(this, "Ilovalar topilmadi", Toast.LENGTH_SHORT).show()
            return
        }

        val dp = resources.displayMetrics.density
        fun dp(v: Int): Int = (v * dp).toInt()

        val dialog = Dialog(this)
        dialog.requestWindowFeature(Window.FEATURE_NO_TITLE)

        val dialogView = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            background = GradientDrawable().apply {
                setColor(Color.WHITE)
                cornerRadius = dp(20).toFloat()
            }
            val pad = dp(16)
            setPadding(pad, pad, pad, pad)
        }

        // Title and description
        val titleView = TextView(this).apply {
            text = "Tanlangan ilovalarda o‘chirish"
            textSize = 18f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor("#0F172A".toColorInt())
        }
        dialogView.addView(titleView)

        val subtitleView = TextView(this).apply {
            text = "Belgilangan ilovalarda Harfiy ishlamaydi"
            textSize = 13f
            setTextColor("#64748B".toColorInt())
            setPadding(0, dp(2), 0, dp(12))
        }
        dialogView.addView(subtitleView)

        // Search Bar
        val searchBox = EditText(this).apply {
            hint = "🔍  Ilovani qidirish..."
            textSize = 14f
            setTextColor("#0F172A".toColorInt())
            setHintTextColor("#94A3B8".toColorInt())
            val padSearch = dp(10)
            setPadding(padSearch, padSearch, padSearch, padSearch)
            background = GradientDrawable().apply {
                setColor("#F1F5F9".toColorInt())
                cornerRadius = dp(12).toFloat()
                setStroke(dp(1), "#E2E8F0".toColorInt())
            }
            isSingleLine = true
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                bottomMargin = dp(10)
            }
        }
        dialogView.addView(searchBox)

        // ListView & Dynamic List
        var filteredApps = allApps.toMutableList()
        lateinit var saveBtn: Button

        fun updateSaveBtnText() {
            val count = currentExcluded.size
            saveBtn.text = if (count == 0) "Saqlash" else "Saqlash ($count)"
        }

        val listView = ListView(this).apply {
            divider = GradientDrawable().apply {
                setColor("#F1F5F9".toColorInt())
                setSize(ViewGroup.LayoutParams.MATCH_PARENT, dp(1))
            }
            dividerHeight = dp(1)
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                0,
                1f
            )
        }

        val adapter = object : BaseAdapter() {
            override fun getCount(): Int = filteredApps.size
            override fun getItem(position: Int): ExcludedAppItem = filteredApps[position]
            override fun getItemId(position: Int): Long = position.toLong()

            override fun getView(position: Int, convertView: View?, parent: ViewGroup?): View {
                val row = (convertView as? LinearLayout) ?: LinearLayout(this@MainActivity).apply {
                    orientation = LinearLayout.HORIZONTAL
                    gravity = Gravity.CENTER_VERTICAL
                    val padRow = dp(8)
                    setPadding(dp(4), padRow, dp(4), padRow)
                    isClickable = true

                    val icon = ImageView(this@MainActivity).apply {
                        tag = "icon"
                        layoutParams = LinearLayout.LayoutParams(dp(38), dp(38)).apply {
                            rightMargin = dp(12)
                        }
                    }
                    addView(icon)

                    val textCol = LinearLayout(this@MainActivity).apply {
                        orientation = LinearLayout.VERTICAL
                        layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)

                        val name = TextView(this@MainActivity).apply {
                            tag = "name"
                            textSize = 15f
                            setTextColor("#1E293B".toColorInt())
                            typeface = Typeface.DEFAULT_BOLD
                            isSingleLine = true
                        }
                        addView(name)
                    }
                    addView(textCol)

                    val checkbox = CheckBox(this@MainActivity).apply {
                        tag = "checkbox"
                        isFocusable = false
                        isClickable = false
                    }
                    addView(checkbox)
                }

                val item = getItem(position)
                val iconView = row.findViewWithTag<ImageView>("icon")
                val nameView = row.findViewWithTag<TextView>("name")
                val checkView = row.findViewWithTag<CheckBox>("checkbox")

                nameView.text = item.label
                iconView.setImageDrawable(item.loadIcon(pm))
                checkView.isChecked = currentExcluded.contains(item.packageName)

                row.setOnClickListener {
                    if (currentExcluded.contains(item.packageName)) {
                        currentExcluded.remove(item.packageName)
                        checkView.isChecked = false
                    } else {
                        currentExcluded.add(item.packageName)
                        checkView.isChecked = true
                    }
                    updateSaveBtnText()
                }

                return row
            }
        }

        listView.adapter = adapter
        dialogView.addView(listView)

        // Real-time search filter
        searchBox.addTextChangedListener(object : TextWatcher {
            override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {}
            override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) {
                val query = s?.toString()?.trim()?.lowercase(Locale.getDefault()) ?: ""
                filteredApps = if (query.isEmpty()) {
                    allApps.toMutableList()
                } else {
                    allApps.filter { it.label.lowercase(Locale.getDefault()).contains(query) }.toMutableList()
                }
                adapter.notifyDataSetChanged()
            }
            override fun afterTextChanged(s: Editable?) {}
        })

        // Action Buttons Row
        val btnRow = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                topMargin = dp(12)
            }
        }

        val btnCancel = Button(this).apply {
            text = "Bekor qilish"
            textSize = 14f
            setTextColor("#64748B".toColorInt())
            isAllCaps = false
            minHeight = 0
            minimumHeight = 0
            background = GradientDrawable().apply {
                setColor("#F1F5F9".toColorInt())
                cornerRadius = dp(12).toFloat()
            }
            layoutParams = LinearLayout.LayoutParams(0, dp(46), 1f).apply {
                rightMargin = dp(8)
            }
            setOnClickListener {
                dialog.dismiss()
            }
        }
        btnRow.addView(btnCancel)

        saveBtn = Button(this).apply {
            textSize = 14f
            setTextColor(Color.WHITE)
            typeface = Typeface.DEFAULT_BOLD
            isAllCaps = false
            minHeight = 0
            minimumHeight = 0
            background = GradientDrawable().apply {
                setColor("#2563EB".toColorInt())
                cornerRadius = dp(12).toFloat()
            }
            layoutParams = LinearLayout.LayoutParams(0, dp(46), 1f)
            setOnClickListener {
                prefs.edit(commit = true) { putStringSet(HarfiyAccessibilityService.KEY_EXCLUDED_APPS, currentExcluded) }
                updateExcludedAppsSummary()
                Toast.makeText(this@MainActivity, "Saqlandi", Toast.LENGTH_SHORT).show()
                dialog.dismiss()
            }
        }
        updateSaveBtnText()
        btnRow.addView(saveBtn)
        dialogView.addView(btnRow)

        dialog.setContentView(dialogView)

        dialog.window?.apply {
            val width = (resources.displayMetrics.widthPixels * 0.92).toInt()
            val height = (resources.displayMetrics.heightPixels * 0.82).toInt()
            setLayout(width, height)
            setBackgroundDrawableResource(android.R.color.transparent)
        }

        dialog.show()
    }

    private fun isAccessibilityServiceEnabled(): Boolean {
        if (HarfiyAccessibilityService.instance != null) {
            return true
        }

        val am = getSystemService(Context.ACCESSIBILITY_SERVICE) as? AccessibilityManager ?: return false
        val list = am.getEnabledAccessibilityServiceList(AccessibilityServiceInfo.FEEDBACK_ALL_MASK)
        return list.any {
            it.resolveInfo.serviceInfo.packageName == packageName &&
            it.resolveInfo.serviceInfo.name.contains("HarfiyAccessibilityService")
        }
    }

    private fun isXiaomiOrMiui(): Boolean {
        val man = Build.MANUFACTURER.lowercase()
        val brand = Build.BRAND.lowercase()
        return man.contains("xiaomi") || man.contains("redmi") || man.contains("poco") ||
                brand.contains("xiaomi") || brand.contains("redmi") || brand.contains("poco")
    }

    private fun isXiaomiAutostartEnabled(): Boolean {
        val appOps = getSystemService(Context.APP_OPS_SERVICE) as? AppOpsManager ?: return false
        return try {
            val checkOpNoThrowMethod = AppOpsManager::class.java.getMethod(
                "checkOpNoThrow",
                Int::class.javaPrimitiveType,
                Int::class.javaPrimitiveType,
                String::class.java
            )
            val uid = Process.myUid()
            val mode10008 = checkOpNoThrowMethod.invoke(appOps, 10008, uid, packageName) as? Int ?: -1
            if (mode10008 == AppOpsManager.MODE_ALLOWED) {
                return true
            }
            val mode10021 = checkOpNoThrowMethod.invoke(appOps, 10021, uid, packageName) as? Int ?: -1
            mode10021 == AppOpsManager.MODE_ALLOWED
        } catch (_: Exception) {
            false
        }
    }

    private fun openXiaomiAutostartSettings() {
        try {
            val intent = Intent().apply {
                component = ComponentName("com.miui.securitycenter", "com.miui.permcenter.autostart.AutoStartManagementActivity")
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            }
            startActivity(intent)
        } catch (_: Exception) {
            try {
                val intent = Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS).apply {
                    data = Uri.fromParts("package", packageName, null)
                    addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                }
                startActivity(intent)
            } catch (_: Exception) {
                Toast.makeText(this, "Sozlamalarni ochib bo‘lmadi", Toast.LENGTH_SHORT).show()
            }
        }
    }
}
