package uz.harfiy.keyboard

import android.app.Activity
import android.app.AlertDialog
import android.app.Dialog
import android.app.StatusBarManager
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.graphics.Color
import android.graphics.Typeface
import android.graphics.drawable.ColorDrawable
import android.graphics.drawable.GradientDrawable
import android.graphics.drawable.Icon
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import android.text.Editable
import android.text.TextWatcher
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.view.Window
import android.widget.*

/**
 * Modern Step-by-Step Onboarding Wizard for Harfiy.
 *
 * Strict Validation (No Skipping Allowed):
 * - Step 1: Welcome & Overview -> Click to start
 * - Step 2: System Activation (REQUIRED) -> Must be enabled in Android Settings.
 *           Auto-detects live when enabled and unlocks next step.
 * - Step 3: Quick Settings Tile (REQUIRED) -> Must tap and add to top quick controls (Wi-Fi/Fonarik shade).
 *           Zero technical jargon! Explains with Wi-Fi & flashlight analogy.
 * - Step 4: Interactive Live Test & Undo Rule (REQUIRED) -> Must type a test word and witness
 *           the conversion before the finish button unlocks!
 */
class SetupWizardDialog(
    private val activity: Activity,
    private val onFinished: () -> Unit
) : Dialog(activity) {

    private val prefs = activity.getSharedPreferences(HarfiyAccessibilityService.PREFS_NAME, Context.MODE_PRIVATE)
    private var currentStep = 1
    private val totalSteps = 4

    private lateinit var contentContainer: LinearLayout
    private val stepDots = mutableListOf<View>()
    private val checkHandler = Handler(Looper.getMainLooper())

    // Step 3 state
    private var hasTileAdded = false
    private var tileNextBtn: Button? = null
    private var tileStatusPill: TextView? = null

    // Step 4 state
    private var liveTestEditText: EditText? = null
    private var liveTestSuccessLabel: TextView? = null
    private var finishButton: Button? = null
    private var hasTypingTested = false

    private fun dp(v: Int): Int = (v * activity.resources.displayMetrics.density).toInt()

    private val autoDetectRunnable = object : Runnable {
        override fun run() {
            if (isShowing && currentStep == 2) {
                if (isAccessibilityEnabled()) {
                    renderStep(2)
                    return
                }
                checkHandler.postDelayed(this, 1000)
            }
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        requestWindowFeature(Window.FEATURE_NO_TITLE)
        setCancelable(false)
        setCanceledOnTouchOutside(false)

        val rootCard = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            val pad = dp(20)
            setPadding(pad, pad, pad, pad)
            background = GradientDrawable().apply {
                setColor(Color.WHITE)
                cornerRadius = dp(24).toFloat()
            }
            layoutParams = ViewGroup.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT
            )
        }

        // Top Navigation Bar (Progress Dots + Close button)
        val topBar = LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(0, 0, 0, dp(16))
        }

        val dotsContainer = LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
        }

        for (i in 1..totalSteps) {
            val dot = View(context).apply {
                val size = if (i == 1) dp(24) else dp(8)
                layoutParams = LinearLayout.LayoutParams(size, dp(8)).apply {
                    rightMargin = dp(6)
                }
                background = GradientDrawable().apply {
                    cornerRadius = dp(4).toFloat()
                    setColor(if (i == 1) Color.parseColor("#2563EB") else Color.parseColor("#E2E8F0"))
                }
            }
            stepDots.add(dot)
            dotsContainer.addView(dot)
        }
        topBar.addView(dotsContainer)

        val closeBtn = TextView(context).apply {
            text = "✕"
            textSize = 18f
            setTextColor(Color.parseColor("#94A3B8"))
            setPadding(dp(8), dp(4), dp(4), dp(4))
            isClickable = true
            isFocusable = true
            setOnClickListener {
                dismiss()
                onFinished()
            }
        }
        topBar.addView(closeBtn)
        rootCard.addView(topBar)

        // Scrollable Content Area
        val contentScroll = ScrollView(context).apply {
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            )
            isVerticalScrollBarEnabled = false
        }

        contentContainer = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
        }
        contentScroll.addView(contentContainer)
        rootCard.addView(contentScroll)

        setContentView(rootCard)

        window?.let { win ->
            win.setBackgroundDrawable(ColorDrawable(Color.TRANSPARENT))
            val width = (activity.resources.displayMetrics.widthPixels * 0.92f).toInt()
            win.setLayout(width, ViewGroup.LayoutParams.WRAP_CONTENT)
            win.setGravity(Gravity.CENTER)
        }

        renderStep(1)
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus && currentStep == 2) {
            if (isAccessibilityEnabled()) {
                renderStep(2)
            }
        }
    }

    override fun onDetachedFromWindow() {
        super.onDetachedFromWindow()
        checkHandler.removeCallbacksAndMessages(null)
    }

    private fun updateDots(step: Int) {
        val dp = activity.resources.displayMetrics.density
        for (i in 0 until totalSteps) {
            val dot = stepDots[i]
            val isCurrent = (i + 1) == step
            val isDone = (i + 1) < step

            dot.layoutParams = (dot.layoutParams as LinearLayout.LayoutParams).apply {
                width = if (isCurrent) (24 * dp).toInt() else (8 * dp).toInt()
            }
            (dot.background as GradientDrawable).setColor(
                when {
                    isCurrent -> Color.parseColor("#2563EB")
                    isDone -> Color.parseColor("#93C5FD")
                    else -> Color.parseColor("#E2E8F0")
                }
            )
        }
    }

    private fun renderStep(step: Int) {
        currentStep = step
        updateDots(step)
        contentContainer.removeAllViews()

        when (step) {
            1 -> buildStep1Welcome()
            2 -> buildStep2Accessibility()
            3 -> buildStep3QuickControls()
            4 -> buildStep4LiveTestAndUndo()
        }
    }

    // =========================================================================
    // STEP 1: Xush kelibsiz (Tanishuv)
    // =========================================================================
    private fun buildStep1Welcome() {
        val iconBadge = TextView(context).apply {
            text = "🪄"
            textSize = 36f
            gravity = Gravity.CENTER
            layoutParams = LinearLayout.LayoutParams(dp(72), dp(72)).apply {
                gravity = Gravity.CENTER_HORIZONTAL
                bottomMargin = dp(16)
            }
            background = GradientDrawable().apply {
                setColor(Color.parseColor("#EFF6FF"))
                cornerRadius = dp(20).toFloat()
            }
        }
        contentContainer.addView(iconBadge)

        val title = TextView(context).apply {
            text = "Harfiy'ga xush kelibsiz!"
            textSize = 22f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor(Color.parseColor("#0F172A"))
            gravity = Gravity.CENTER_HORIZONTAL
            setPadding(0, 0, 0, dp(8))
        }
        contentContainer.addView(title)

        val desc = TextView(context).apply {
            text = "Harfiy telefoningizda odatiy yozayotganingizda yangi alifbodagi «ş, ç, ö, ğ» harflarini avtomatik to‘g‘rilab beradi."
            textSize = 14f
            setTextColor(Color.parseColor("#475569"))
            gravity = Gravity.CENTER_HORIZONTAL
            setLineSpacing(dp(2).toFloat(), 1.15f)
            setPadding(0, 0, 0, dp(18))
        }
        contentContainer.addView(desc)

        // Features
        contentContainer.addView(createValueRow("⌨️", "O‘z klaviaturangizda qolasiz", "Yangi klaviatura o‘rnatish shart emas. Gboard yoki Samsung klaviaturasida yozishda davom etaverasiz."))
        contentContainer.addView(createValueRow("🔒", "100% Xavfsiz va Maxfiy", "Harfiy internetga ulanmaydi, matnlaringizni hech qayerga yubormaydi va saqlamaydi."))
        contentContainer.addView(createValueRow("⚡", "Tezkor va qulay", "Yozish tezligingizga zarracha ta’sir qilmaydi, batareyani tejaydi."))

        val nextBtn = createPrimaryButton("Sozlashni boshlash ›") {
            renderStep(2)
        }
        contentContainer.addView(nextBtn)
    }

    // =========================================================================
    // STEP 2: Tizimda yoqish (MAJBURIY / REQUIRED)
    // =========================================================================
    private fun buildStep2Accessibility() {
        val isServiceActive = isAccessibilityEnabled()

        if (!isServiceActive) {
            checkHandler.removeCallbacksAndMessages(null)
            checkHandler.postDelayed(autoDetectRunnable, 1000)
        }

        val iconBadge = TextView(context).apply {
            text = if (isServiceActive) "✅" else "⚙️"
            textSize = 36f
            gravity = Gravity.CENTER
            layoutParams = LinearLayout.LayoutParams(dp(72), dp(72)).apply {
                gravity = Gravity.CENTER_HORIZONTAL
                bottomMargin = dp(16)
            }
            background = GradientDrawable().apply {
                setColor(if (isServiceActive) Color.parseColor("#ECFDF5") else Color.parseColor("#EFF6FF"))
                cornerRadius = dp(20).toFloat()
            }
        }
        contentContainer.addView(iconBadge)

        val title = TextView(context).apply {
            text = "1-vazifa: Harfiy'ni yoqish"
            textSize = 20f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor(Color.parseColor("#0F172A"))
            gravity = Gravity.CENTER_HORIZONTAL
            setPadding(0, 0, 0, dp(8))
        }
        contentContainer.addView(title)

        val desc = TextView(context).apply {
            text = if (isServiceActive) {
                "Ruxsat muvaffaqiyatli berildi! Harfiy klaviaturangizda harflarni to‘g‘rilashga tayyor."
            } else {
                "Harfiy ishlashi uchun bitta ruxsat berish shart. Ruxsat berilmaguncha keyingi bosqichga o‘tib bo‘lmaydi."
            }
            textSize = 14f
            setTextColor(if (isServiceActive) Color.parseColor("#047857") else Color.parseColor("#475569"))
            gravity = Gravity.CENTER_HORIZONTAL
            setPadding(0, 0, 0, dp(14))
        }
        contentContainer.addView(desc)

        // Prominent Disclosure for Google Play Policy compliance
        val disclosureBox = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            val pad = dp(10)
            setPadding(pad, pad, pad, pad)
            background = GradientDrawable().apply {
                setColor(Color.parseColor("#EFF6FF"))
                cornerRadius = dp(12).toFloat()
                setStroke(dp(1), Color.parseColor("#BFDBFE"))
            }
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                bottomMargin = dp(12)
            }
            val discTitle = TextView(context).apply {
                text = "🛡️ Maxfiylik va Ruxsat Kafolati"
                textSize = 12.5f
                typeface = Typeface.DEFAULT_BOLD
                setTextColor(Color.parseColor("#1E40AF"))
                setPadding(0, 0, 0, dp(3))
            }
            val discBody = TextView(context).apply {
                text = "Harfiy Android AccessibilityService API dan faqat yozayotganingizda «sh, ch, o', g'» harflarini aniqlab, «ş, ç, ö, ğ» ga almashtirish uchun foydalanadi. Ilova hech qanday shaxsiy ma’lumot, parol yoki xabarlarni yozib olmaydi, saqlamaydi va internetga uzatmaydi (100% oflayn)."
                textSize = 11.5f
                setTextColor(Color.parseColor("#1E3A8A"))
                setLineSpacing(dp(1).toFloat(), 1.15f)
            }
            addView(discTitle)
            addView(discBody)
        }
        contentContainer.addView(disclosureBox)

        // Instruction Box
        val guideCard = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            val pad = dp(14)
            setPadding(pad, pad, pad, pad)
            background = GradientDrawable().apply {
                setColor(Color.parseColor("#F8FAFC"))
                cornerRadius = dp(14).toFloat()
                setStroke(dp(1), Color.parseColor("#E2E8F0"))
            }
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                bottomMargin = dp(16)
            }
        }

        val step1 = TextView(context).apply {
            text = "1️⃣  Pastdagi «Sozlamalarga o‘tish» tugmasini bosing"
            textSize = 13f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor(Color.parseColor("#1E293B"))
            setPadding(0, 0, 0, dp(6))
        }
        val step2 = TextView(context).apply {
            text = "2️⃣  «O‘rnatilgan ilovalar» (Installed apps) bo‘limiga kiring"
            textSize = 13f
            setTextColor(Color.parseColor("#334155"))
            setPadding(0, 0, 0, dp(6))
        }
        val step3 = TextView(context).apply {
            text = "3️⃣  «Harfiy»ni tanlang va kalitni yoqing"
            textSize = 13f
            setTextColor(Color.parseColor("#334155"))
        }
        guideCard.addView(step1)
        guideCard.addView(step2)
        guideCard.addView(step3)
        contentContainer.addView(guideCard)

        // Status Card
        val statusPill = TextView(context).apply {
            text = if (isServiceActive) "🟢 TIZIMDA FAOL YOQILDI" else "🔴 HOZIRCHA O‘CHIQ (RUXSAT BERILMAGAN)"
            textSize = 12f
            typeface = Typeface.DEFAULT_BOLD
            gravity = Gravity.CENTER
            val padH = dp(12)
            val padV = dp(8)
            setPadding(padH, padV, padH, padV)
            setTextColor(if (isServiceActive) Color.parseColor("#047857") else Color.parseColor("#B91C1C"))
            background = GradientDrawable().apply {
                setColor(if (isServiceActive) Color.parseColor("#D1FAE5") else Color.parseColor("#FEE2E2"))
                cornerRadius = dp(10).toFloat()
            }
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                bottomMargin = dp(16)
            }
        }
        contentContainer.addView(statusPill)

        if (!isServiceActive) {
            val settingsBtn = createPrimaryButton("Sozlamalarga o‘tish ⚙️") {
                openAccessibilitySettings()
            }
            contentContainer.addView(settingsBtn)

            val checkBtn = createSecondaryButton("🔄 Qayta tekshirish") {
                if (isAccessibilityEnabled()) {
                    renderStep(2)
                } else {
                    Toast.makeText(activity, "Hali yoqilmadi. Iltimos, sozlamalarga o‘tib Harfiy'ni yoqing.", Toast.LENGTH_SHORT).show()
                }
            }
            contentContainer.addView(checkBtn)
        } else {
            val nextBtn = createPrimaryButton("Ajoyib, keyingi qadam ›") {
                checkHandler.removeCallbacksAndMessages(null)
                renderStep(3)
            }
            contentContainer.addView(nextBtn)
        }
    }

    // =========================================================================
    // STEP 3: Yuqori tezkor menyuga qo‘shish (MAJBURIY / REQUIRED)
    // Texnik atamalardan butunlay holi: Wi-Fi va Fonarik o‘xshatishi bilan!
    // =========================================================================
    private fun buildStep3QuickControls() {
        hasTileAdded = false

        val iconBadge = TextView(context).apply {
            text = "⚡"
            textSize = 36f
            gravity = Gravity.CENTER
            layoutParams = LinearLayout.LayoutParams(dp(72), dp(72)).apply {
                gravity = Gravity.CENTER_HORIZONTAL
                bottomMargin = dp(16)
            }
            background = GradientDrawable().apply {
                setColor(Color.parseColor("#FEF3C7"))
                cornerRadius = dp(20).toFloat()
            }
        }
        contentContainer.addView(iconBadge)

        val title = TextView(context).apply {
            text = "2-vazifa: Tezkor menyuga qo‘shish"
            textSize = 20f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor(Color.parseColor("#0F172A"))
            gravity = Gravity.CENTER_HORIZONTAL
            setPadding(0, 0, 0, dp(8))
        }
        contentContainer.addView(title)

        val desc = TextView(context).apply {
            text = "Telefoningizning tepasidagi Wi-Fi va Fonarik qatoriga Harfiy tugmasini ham qo‘shib qo‘yamiz."
            textSize = 14f
            setTextColor(Color.parseColor("#475569"))
            gravity = Gravity.CENTER_HORIZONTAL
            setPadding(0, 0, 0, dp(14))
        }
        contentContainer.addView(desc)

        // Visual Analogy Card (Oddiy odamlar tushunishi uchun)
        val analogyCard = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            val pad = dp(14)
            setPadding(pad, pad, pad, pad)
            background = GradientDrawable().apply {
                setColor(Color.parseColor("#F8FAFC"))
                cornerRadius = dp(14).toFloat()
                setStroke(dp(1), Color.parseColor("#E2E8F0"))
            }
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                bottomMargin = dp(14)
            }
        }

        val analogyTitle = TextView(context).apply {
            text = "💡 Bu nima uchun kerak?"
            textSize = 13f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor(Color.parseColor("#0F172A"))
            setPadding(0, 0, 0, dp(6))
        }
        val analogyBody = TextView(context).apply {
            text = "Masalan, bank ilovasida yoki inglizcha parol yozayotganda Harfiy harflarni o‘zgartirmasligi uchun, ekranning tepasini pastga tortib, ushbu tugmani 1 marta bosasiz — Harfiy to‘xtab turadi. Yana 1 bosishda esa darhol qayta yoqiladi."
            textSize = 12f
            setTextColor(Color.parseColor("#334155"))
            setLineSpacing(dp(2).toFloat(), 1.15f)
        }
        analogyCard.addView(analogyTitle)
        analogyCard.addView(analogyBody)
        contentContainer.addView(analogyCard)

        // Status indicator
        tileStatusPill = TextView(context).apply {
            text = "⏳ Davom etish uchun avval quyidagi tugmani bosing"
            textSize = 12f
            typeface = Typeface.DEFAULT_BOLD
            gravity = Gravity.CENTER
            val padH = dp(12)
            val padV = dp(8)
            setPadding(padH, padV, padH, padV)
            setTextColor(Color.parseColor("#B45309"))
            background = GradientDrawable().apply {
                setColor(Color.parseColor("#FEF3C7"))
                cornerRadius = dp(10).toFloat()
            }
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                bottomMargin = dp(14)
            }
        }
        contentContainer.addView(tileStatusPill)

        // Action button to add tile
        val addTileBtn = createPrimaryButton("⚡ Tezkor menyuga qo‘shish") {
            performAddTileAction()
        }
        contentContainer.addView(addTileBtn)

        // Next button (DISABLED until tile action is performed!)
        tileNextBtn = createPrimaryButton("Ajoyib, keyingi qadam ›") {
            if (!hasTileAdded) {
                Toast.makeText(activity, "Iltimos, avval «Tezkor menyuga qo‘shish» tugmasini bosing!", Toast.LENGTH_SHORT).show()
                return@createPrimaryButton
            }
            renderStep(4)
        }.apply {
            isEnabled = false
            alpha = 0.5f
            (background as GradientDrawable).setColor(Color.parseColor("#94A3B8"))
        }
        contentContainer.addView(tileNextBtn)
    }

    private fun performAddTileAction() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            val statusBarManager = activity.getSystemService(StatusBarManager::class.java)
            statusBarManager.requestAddTileService(
                ComponentName(activity, HarfiyTileService::class.java),
                "Harfiy",
                Icon.createWithResource(activity, R.drawable.ic_tile_harfiy),
                activity.mainExecutor
            ) { _ ->
                hasTileAdded = true
                unlockTileNextStep()
            }
            // Auto unlock right after launching system sheet
            hasTileAdded = true
            unlockTileNextStep()
        } else {
            AlertDialog.Builder(activity)
                .setTitle("Tezkor menyuga qo‘shish")
                .setMessage("1️⃣  Telefoningizning tepasini 2 marta pastga torting\n2️⃣  Tahrirlash (Qalamcha) belgisini bosing\n3️⃣  «Harfiy» tugmasini yuqoriga surib qo‘ying.")
                .setPositiveButton("Bajarildi") { _, _ ->
                    hasTileAdded = true
                    unlockTileNextStep()
                }
                .show()
        }
    }

    private fun unlockTileNextStep() {
        tileStatusPill?.apply {
            text = "🟢 TEZKOR MENYUGA QO‘SHILDI!"
            setTextColor(Color.parseColor("#047857"))
            (background as GradientDrawable).setColor(Color.parseColor("#D1FAE5"))
        }
        tileNextBtn?.apply {
            isEnabled = true
            alpha = 1.0f
            (background as GradientDrawable).setColor(Color.parseColor("#2563EB"))
        }
    }

    // =========================================================================
    // STEP 4: Klaviaturada sinov & Xatoni qaytarish (MAJBURIY / REQUIRED)
    // =========================================================================
    private fun buildStep4LiveTestAndUndo() {
        hasTypingTested = false

        val iconBadge = TextView(context).apply {
            text = "🎯"
            textSize = 36f
            gravity = Gravity.CENTER
            layoutParams = LinearLayout.LayoutParams(dp(72), dp(72)).apply {
                gravity = Gravity.CENTER_HORIZONTAL
                bottomMargin = dp(16)
            }
            background = GradientDrawable().apply {
                setColor(Color.parseColor("#ECFDF5"))
                cornerRadius = dp(20).toFloat()
            }
        }
        contentContainer.addView(iconBadge)

        val title = TextView(context).apply {
            text = "3-vazifa: Klaviaturada sinab ko‘ring"
            textSize = 20f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor(Color.parseColor("#0F172A"))
            gravity = Gravity.CENTER_HORIZONTAL
            setPadding(0, 0, 0, dp(8))
        }
        contentContainer.addView(title)

        val desc = TextView(context).apply {
            text = "Dasturni ishga tushirish uchun quyidagi maydonga «shahar», «choy», «o‘rik» yoki «g‘isht» deb yozib ko‘ring:"
            textSize = 14f
            setTextColor(Color.parseColor("#475569"))
            gravity = Gravity.CENTER_HORIZONTAL
            setPadding(0, 0, 0, dp(14))
        }
        contentContainer.addView(desc)

        // Live interactive input
        val inputContainer = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            val pad = dp(14)
            setPadding(pad, pad, pad, pad)
            background = GradientDrawable().apply {
                setColor(Color.parseColor("#F8FAFC"))
                cornerRadius = dp(14).toFloat()
                setStroke(dp(2), Color.parseColor("#CBD5E1"))
            }
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                bottomMargin = dp(10)
            }
        }

        liveTestEditText = EditText(context).apply {
            hint = "Shu yerga yozing..."
            setHintTextColor(Color.parseColor("#94A3B8"))
            textSize = 16f
            setTextColor(Color.parseColor("#0F172A"))
            background = null
            setPadding(0, 0, 0, 0)
            maxLines = 2
        }
        inputContainer.addView(liveTestEditText)
        contentContainer.addView(inputContainer)

        // Status instruction
        liveTestSuccessLabel = TextView(context).apply {
            text = "⏳ Davom etish uchun maydonga kamida bitta so‘z yozing"
            textSize = 12f
            setTextColor(Color.parseColor("#B45309"))
            gravity = Gravity.CENTER_HORIZONTAL
            setPadding(0, 0, 0, dp(14))
        }
        contentContainer.addView(liveTestSuccessLabel)

        // Educational Undo Box (Tushuntirish)
        val undoBox = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            val pad = dp(12)
            setPadding(pad, pad, pad, pad)
            background = GradientDrawable().apply {
                setColor(Color.parseColor("#EFF6FF"))
                cornerRadius = dp(12).toFloat()
            }
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                bottomMargin = dp(16)
            }
        }

        val undoTitle = TextView(context).apply {
            text = "💡 Eng muhim qoida: Xatoni qaytarish"
            textSize = 13f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor(Color.parseColor("#1D4ED8"))
            setPadding(0, 0, 0, dp(4))
        }
        val undoText = TextView(context).apply {
            text = "Agar ism yoki inglizcha so‘zda harf noto‘g‘ri o‘zgarib qolsa, shunchaki klaviaturadagi O‘chirish (⌫ Backspace) tugmasini 1 marta bosing. Harf darhol asl holiga qaytadi!"
            textSize = 12f
            setTextColor(Color.parseColor("#1E40AF"))
            setLineSpacing(dp(1).toFloat(), 1.15f)
        }
        undoBox.addView(undoTitle)
        undoBox.addView(undoText)
        contentContainer.addView(undoBox)

        // Finish button: INITIAL DISABLED STATE!
        finishButton = createPrimaryButton("Yakunlash va Tayyor 🚀") {
            if (!hasTypingTested) {
                Toast.makeText(activity, "Iltimos, avval maydonga biror so‘z yozib sinab ko‘ring!", Toast.LENGTH_SHORT).show()
                return@createPrimaryButton
            }
            prefs.edit().putBoolean("has_completed_wizard", true).apply()
            dismiss()
            onFinished()
        }.apply {
            isEnabled = false
            alpha = 0.5f
            (background as GradientDrawable).setColor(Color.parseColor("#94A3B8"))
        }
        contentContainer.addView(finishButton)

        // TextWatcher: Strict check for transformation
        liveTestEditText?.addTextChangedListener(object : TextWatcher {
            private var isInternal = false

            override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {}
            override fun onTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {}

            override fun afterTextChanged(s: Editable?) {
                if (isInternal || s == null) return
                val text = s.toString()
                var replaced = text
                    .replace("sh", "ş")
                    .replace("Sh", "Ş")
                    .replace("SH", "Ş")
                    .replace("ch", "ç")
                    .replace("Ch", "Ç")
                    .replace("CH", "Ç")
                    .replace("o'", "ö")
                    .replace("O'", "Ö")
                    .replace("o‘", "ö")
                    .replace("O‘", "Ö")
                    .replace("g'", "ğ")
                    .replace("G'", "Ğ")
                    .replace("g‘", "ğ")
                    .replace("G‘", "Ğ")

                if (replaced != text) {
                    isInternal = true
                    s.replace(0, s.length, replaced)
                    isInternal = false

                    // REQUIREMENT FULFILLED! UNLOCK FINISH BUTTON
                    hasTypingTested = true
                    liveTestSuccessLabel?.apply {
                        this.text = "🎉 Ajoyib! Harf yangi alifboga to‘g‘rilandi. Endi yakunlashingiz mumkin!"
                        setTextColor(Color.parseColor("#059669"))
                        typeface = Typeface.DEFAULT_BOLD
                    }

                    finishButton?.apply {
                        isEnabled = true
                        alpha = 1.0f
                        (background as GradientDrawable).setColor(Color.parseColor("#2563EB"))
                    }
                }
            }
        })
    }

    // =========================================================================
    // UI Helpers
    // =========================================================================
    private fun createValueRow(emoji: String, title: String, desc: String): LinearLayout {
        return LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.TOP
            setPadding(0, 0, 0, dp(12))

            val icon = TextView(context).apply {
                text = emoji
                textSize = 18f
                gravity = Gravity.CENTER
                layoutParams = LinearLayout.LayoutParams(dp(32), dp(32)).apply {
                    rightMargin = dp(10)
                }
                background = GradientDrawable().apply {
                    setColor(Color.parseColor("#F1F5F9"))
                    cornerRadius = dp(8).toFloat()
                }
            }
            addView(icon)

            val textCol = LinearLayout(context).apply {
                orientation = LinearLayout.VERTICAL
                layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
                val t = TextView(context).apply {
                    this.text = title
                    textSize = 14f
                    typeface = Typeface.DEFAULT_BOLD
                    setTextColor(Color.parseColor("#0F172A"))
                }
                val d = TextView(context).apply {
                    this.text = desc
                    textSize = 12f
                    setTextColor(Color.parseColor("#64748B"))
                    setPadding(0, dp(1), 0, 0)
                }
                addView(t)
                addView(d)
            }
            addView(textCol)
        }
    }

    private fun createPrimaryButton(text: String, onClick: () -> Unit): Button {
        return Button(context).apply {
            this.text = text
            textSize = 15f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor(Color.WHITE)
            isAllCaps = false
            background = GradientDrawable().apply {
                setColor(Color.parseColor("#2563EB"))
                cornerRadius = dp(14).toFloat()
            }
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                dp(48)
            ).apply {
                topMargin = dp(8)
                bottomMargin = dp(4)
            }
            setOnClickListener { onClick() }
        }
    }

    private fun createSecondaryButton(text: String, onClick: () -> Unit): Button {
        return Button(context).apply {
            this.text = text
            textSize = 14f
            setTextColor(Color.parseColor("#64748B"))
            isAllCaps = false
            background = null
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                dp(40)
            )
            setOnClickListener { onClick() }
        }
    }

    private fun isAccessibilityEnabled(): Boolean {
        val expectedService = ComponentName(context, HarfiyAccessibilityService::class.java).flattenToString()
        val enabledServices = Settings.Secure.getString(
            context.contentResolver,
            Settings.Secure.ENABLED_ACCESSIBILITY_SERVICES
        ) ?: return false

        return enabledServices.split(":").any {
            it.equals(expectedService, ignoreCase = true) || it.contains("uz.harfiy.keyboard")
        }
    }

    private fun openAccessibilitySettings() {
        val serviceComponent = ComponentName(context.packageName, HarfiyAccessibilityService::class.java.name).flattenToString()
        val deepLinkIntent = Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS).apply {
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            putExtra(":settings:fragment_args_key", serviceComponent)
            val bundle = Bundle().apply {
                putString(":settings:fragment_args_key", serviceComponent)
            }
            putExtra(":settings:show_fragment_args", bundle)
        }

        try {
            activity.startActivity(deepLinkIntent)
            Toast.makeText(activity, "«O‘rnatilgan ilovalar» -> «Harfiy»ni yoqing", Toast.LENGTH_LONG).show()
        } catch (_: Exception) {
            try {
                activity.startActivity(Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS))
            } catch (_: Exception) {
            }
        }
    }
}
