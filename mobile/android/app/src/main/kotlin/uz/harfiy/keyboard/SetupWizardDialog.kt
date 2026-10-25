@file:Suppress("SetTextI18n")

package uz.harfiy.keyboard

import android.app.Activity
import android.app.AlertDialog
import android.app.AppOpsManager
import android.app.Dialog
import android.app.StatusBarManager
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.graphics.Color
import android.graphics.Typeface
import android.graphics.drawable.ColorDrawable
import android.graphics.drawable.GradientDrawable
import android.graphics.drawable.Icon
import android.os.Build
import android.os.Bundle
import android.os.Process
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import android.text.Editable
import android.text.TextWatcher
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.view.Window
import androidx.core.graphics.toColorInt
import androidx.core.content.edit
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
    private val initialStep: Int = 1,
    private val onFinished: () -> Unit
) : Dialog(activity) {

    private val prefs = activity.getSharedPreferences(HarfiyAccessibilityService.PREFS_NAME, Context.MODE_PRIVATE)
    private val isXiaomi = isXiaomiOrMiui()
    private val totalSteps = if (isXiaomi) 5 else 4
    private var currentStep = initialStep

    private lateinit var contentContainer: LinearLayout
    private val stepDots = mutableListOf<View>()
    private val checkHandler = Handler(Looper.getMainLooper())

    // Step 3/4 Quick Controls state
    private var hasTileAdded = false
    private var tileNextBtn: Button? = null
    private var tileStatusPill: TextView? = null

    // Step 4/5 Live test state
    private var liveTestEditText: EditText? = null
    private var liveTestSuccessLabel: TextView? = null
    private var finishButton: Button? = null
    private var hasTypingTested = false
    private var stepBadge: TextView? = null

    private fun dp(v: Int): Int = (v * activity.resources.displayMetrics.density).toInt()

    private val autoDetectRunnable = object : Runnable {
        override fun run() {
            if (isShowing) {
                if (currentStep == 2 && isAccessibilityEnabled()) {
                    renderStep(2)
                    return
                } else if (isXiaomi && currentStep == 3 && isXiaomiAutostartEnabled()) {
                    renderStep(3)
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
            val pad = dp(16)
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
            setPadding(0, 0, 0, dp(10))
        }

        val dotsContainer = LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            layoutParams = LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f)
        }

        val hasCompletedWizard = prefs.getBoolean("has_completed_wizard", false)

        for (i in 1..totalSteps) {
            val dot = View(context).apply {
                val size = if (i == initialStep) dp(24) else dp(8)
                layoutParams = LinearLayout.LayoutParams(size, dp(8)).apply {
                    rightMargin = dp(6)
                }
                background = GradientDrawable().apply {
                    cornerRadius = dp(4).toFloat()
                    setColor(
                        when {
                            i == initialStep -> "#2563EB".toColorInt()
                            i < initialStep -> "#93C5FD".toColorInt()
                            else -> "#E2E8F0".toColorInt()
                        }
                    )
                }
            }
            stepDots.add(dot)
            dotsContainer.addView(dot)
        }
        topBar.addView(dotsContainer)

        val badge = TextView(context).apply {
            text = "$initialStep / $totalSteps"
            textSize = 12f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor("#2563EB".toColorInt())
            background = GradientDrawable().apply {
                setColor("#EFF6FF".toColorInt())
                cornerRadius = dp(8).toFloat()
            }
            val padH = dp(8)
            val padV = dp(3)
            setPadding(padH, padV, padH, padV)
        }
        stepBadge = badge
        topBar.addView(badge)

        // Close button (only available if user previously completed onboarding setup)
        if (hasCompletedWizard) {
            val closeBtn = TextView(context).apply {
                text = "✕"
                textSize = 15f
                typeface = Typeface.DEFAULT_BOLD
                setTextColor("#64748B".toColorInt())
                val pad = dp(6)
                setPadding(pad, pad, pad, pad)
                layoutParams = LinearLayout.LayoutParams(
                    LinearLayout.LayoutParams.WRAP_CONTENT,
                    LinearLayout.LayoutParams.WRAP_CONTENT
                ).apply {
                    leftMargin = dp(8)
                }
                isClickable = true
                isFocusable = true
                setOnClickListener {
                    dismiss()
                }
            }
            topBar.addView(closeBtn)
        }
        rootCard.addView(topBar)

        // Content Area
        val contentScroll = ScrollView(context).apply {
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            )
            isVerticalScrollBarEnabled = false
            overScrollMode = View.OVER_SCROLL_NEVER
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

        renderStep(initialStep)
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus) {
            if (currentStep == 2 && isAccessibilityEnabled()) {
                renderStep(2)
            } else if (isXiaomi && currentStep == 3 && isXiaomiAutostartEnabled()) {
                renderStep(3)
            }
        }
    }

    override fun onDetachedFromWindow() {
        super.onDetachedFromWindow()
        checkHandler.removeCallbacksAndMessages(null)
    }

    @Deprecated("Deprecated in Java")
    override fun onBackPressed() {
        if (prefs.getBoolean("has_completed_wizard", false)) {
            dismiss()
            return
        }
        if (currentStep > 1) {
            renderStep(currentStep - 1)
        } else {
            Toast.makeText(context, "Harfiy to‘liq ishlashi uchun barcha bosqichlarni yakunlash shart", Toast.LENGTH_SHORT).show()
        }
    }

    private fun updateDots(step: Int) {
        stepBadge?.text = "$step / $totalSteps"
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
                    isCurrent -> "#2563EB".toColorInt()
                    isDone -> "#93C5FD".toColorInt()
                    else -> "#E2E8F0".toColorInt()
                }
            )
        }
    }

    private fun renderStep(step: Int) {
        currentStep = step
        updateDots(step)
        contentContainer.removeAllViews()

        if (isXiaomi) {
            when (step) {
                1 -> buildStep1Welcome()
                2 -> buildStep2Accessibility()
                3 -> buildStepXiaomiAutostart()
                4 -> buildStepQuickControls(stepNumber = 3)
                5 -> buildStepLiveTestAndUndo(stepNumber = 4)
            }
        } else {
            when (step) {
                1 -> buildStep1Welcome()
                2 -> buildStep2Accessibility()
                3 -> buildStepQuickControls(stepNumber = 2)
                4 -> buildStepLiveTestAndUndo(stepNumber = 3)
            }
        }
    }

    // =========================================================================
    // STEP 1: Xush kelibsiz (Tanishuv)
    // =========================================================================
    private fun buildStep1Welcome() {
        val iconBadge = TextView(context).apply {
            text = "🪄"
            textSize = 28f
            gravity = Gravity.CENTER
            layoutParams = LinearLayout.LayoutParams(dp(52), dp(52)).apply {
                gravity = Gravity.CENTER_HORIZONTAL
                bottomMargin = dp(8)
            }
            background = GradientDrawable().apply {
                setColor("#EFF6FF".toColorInt())
                cornerRadius = dp(16).toFloat()
            }
        }
        contentContainer.addView(iconBadge)

        val title = TextView(context).apply {
            text = "Harfiy'ga xush kelibsiz!"
            textSize = 19f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor("#0F172A".toColorInt())
            gravity = Gravity.CENTER_HORIZONTAL
            setPadding(0, 0, 0, dp(4))
        }
        contentContainer.addView(title)

        val desc = TextView(context).apply {
            text = "Yozayotganingizda «sh, ch, o', g'» harflarini avtomatik «ş, ç, ö, ğ» ga to‘g‘rilab beradi."
            textSize = 13f
            setTextColor("#475569".toColorInt())
            gravity = Gravity.CENTER_HORIZONTAL
            setPadding(0, 0, 0, dp(10))
        }
        contentContainer.addView(desc)

        // Compact Features Card
        val featCard = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            val pad = dp(10)
            setPadding(pad, pad, pad, 0)
            background = GradientDrawable().apply {
                setColor("#F8FAFC".toColorInt())
                cornerRadius = dp(12).toFloat()
                setStroke(dp(1), "#E2E8F0".toColorInt())
            }
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                bottomMargin = dp(8)
            }
        }
        featCard.addView(createCompactFeature("⌨️", "O‘z klaviaturangizda qolasiz (Gboard, Samsung)"))
        featCard.addView(createCompactFeature("🔒", "100% Xavfsiz va Oflayn (Internet ishlatilmaydi)"))
        featCard.addView(createCompactFeature("⚡", "Tezkor, qulay va batareyani tejaydi"))
        contentContainer.addView(featCard)

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
            textSize = 28f
            gravity = Gravity.CENTER
            layoutParams = LinearLayout.LayoutParams(dp(52), dp(52)).apply {
                gravity = Gravity.CENTER_HORIZONTAL
                bottomMargin = dp(8)
            }
            background = GradientDrawable().apply {
                setColor(if (isServiceActive) "#ECFDF5".toColorInt() else "#EFF6FF".toColorInt())
                cornerRadius = dp(16).toFloat()
            }
        }
        contentContainer.addView(iconBadge)

        val title = TextView(context).apply {
            text = "1-vazifa: Harfiy'ni yoqish"
            textSize = 19f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor("#0F172A".toColorInt())
            gravity = Gravity.CENTER_HORIZONTAL
            setPadding(0, 0, 0, dp(4))
        }
        contentContainer.addView(title)

        val desc = TextView(context).apply {
            text = if (isServiceActive) {
                "Ruxsat muvaffaqiyatli berildi! Keyingi bosqichga o‘ting."
            } else {
                "Harfiy ishlashi uchun bitta ruxsat berish shart."
            }
            textSize = 13f
            setTextColor(if (isServiceActive) "#047857".toColorInt() else "#475569".toColorInt())
            gravity = Gravity.CENTER_HORIZONTAL
            setPadding(0, 0, 0, dp(8))
        }
        contentContainer.addView(desc)

        // Combined Instruction & Disclosure Card
        val guideCard = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            val pad = dp(10)
            setPadding(pad, pad, pad, pad)
            background = GradientDrawable().apply {
                setColor("#F8FAFC".toColorInt())
                cornerRadius = dp(12).toFloat()
                setStroke(dp(1), "#E2E8F0".toColorInt())
            }
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                bottomMargin = dp(8)
            }
        }

        val step1 = TextView(context).apply {
            text = "1️⃣  Pastdagi «Sozlamalarga o‘tish»ni bosing"
            textSize = 12.5f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor("#1E293B".toColorInt())
            setPadding(0, 0, 0, dp(4))
        }
        val step2 = TextView(context).apply {
            text = "2️⃣  «O‘rnatilgan ilovalar» -> «Harfiy»ni yoqing"
            textSize = 12.5f
            setTextColor("#334155".toColorInt())
            setPadding(0, 0, 0, dp(4))
        }
        val step3 = TextView(context).apply {
            text = "🛡️  Harfiy 100% oflayn, shaxsiy ma’lumotlarni yig‘maydi"
            textSize = 11.5f
            setTextColor("#2563EB".toColorInt())
        }
        guideCard.addView(step1)
        guideCard.addView(step2)
        guideCard.addView(step3)
        contentContainer.addView(guideCard)

        // Status Pill
        val statusPill = TextView(context).apply {
            text = if (isServiceActive) "🟢 TIZIMDA FAOL YOQILDI" else "🔴 HOZIRCHA O‘CHIQ (RUXSAT KERAK)"
            textSize = 11.5f
            typeface = Typeface.DEFAULT_BOLD
            gravity = Gravity.CENTER
            val padH = dp(10)
            val padV = dp(6)
            setPadding(padH, padV, padH, padV)
            setTextColor(if (isServiceActive) "#047857".toColorInt() else "#B91C1C".toColorInt())
            background = GradientDrawable().apply {
                setColor(if (isServiceActive) "#D1FAE5".toColorInt() else "#FEE2E2".toColorInt())
                cornerRadius = dp(8).toFloat()
            }
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                bottomMargin = dp(6)
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
            val isWizardCompleted = prefs.getBoolean("has_completed_wizard", false)
            val isAutostartNeeded = isXiaomi && !isXiaomiAutostartEnabled()

            val btnText = when {
                isAutostartNeeded -> "Keyingi vazifa (Autostart) ›"
                isWizardCompleted -> "Tayyor, ilovaga o‘tish 🚀"
                else -> "Ajoyib, keyingi qadam ›"
            }

            val nextBtn = createPrimaryButton(btnText) {
                checkHandler.removeCallbacksAndMessages(null)
                if (isAutostartNeeded) {
                    renderStep(3)
                } else if (isWizardCompleted) {
                    dismiss()
                    onFinished()
                } else {
                    renderStep(if (isXiaomi) 4 else 3)
                }
            }
            contentContainer.addView(nextBtn)
        }
    }

    // =========================================================================
    // STEP (XIAOMI ONLY): Fonda avtomatik ishga tushirish (Autostart)
    // =========================================================================
    private fun buildStepXiaomiAutostart() {
        val isAutostartGranted = isXiaomiAutostartEnabled()

        if (!isAutostartGranted) {
            checkHandler.removeCallbacksAndMessages(null)
            checkHandler.postDelayed(autoDetectRunnable, 1000)
        }

        val iconBadge = TextView(context).apply {
            text = if (isAutostartGranted) "✅" else "🚀"
            textSize = 28f
            gravity = Gravity.CENTER
            layoutParams = LinearLayout.LayoutParams(dp(52), dp(52)).apply {
                gravity = Gravity.CENTER_HORIZONTAL
                bottomMargin = dp(8)
            }
            background = GradientDrawable().apply {
                setColor(if (isAutostartGranted) "#ECFDF5".toColorInt() else "#EFF6FF".toColorInt())
                cornerRadius = dp(16).toFloat()
            }
        }
        contentContainer.addView(iconBadge)

        val title = TextView(context).apply {
            text = "2-vazifa: Avto-ishga tushirish"
            textSize = 19f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor("#0F172A".toColorInt())
            gravity = Gravity.CENTER_HORIZONTAL
            setPadding(0, 0, 0, dp(4))
        }
        contentContainer.addView(title)

        val desc = TextView(context).apply {
            text = if (isAutostartGranted) {
                "Ajoyib! Harfiy fonda to‘xtamasdan ishlash huquqiga ega."
            } else {
                "Xiaomi dasturlarni tozalaganda Harfiy o‘chib qolmasligi uchun bu ruxsat shart."
            }
            textSize = 13f
            setTextColor(if (isAutostartGranted) "#047857".toColorInt() else "#475569".toColorInt())
            gravity = Gravity.CENTER_HORIZONTAL
            setPadding(0, 0, 0, dp(8))
        }
        contentContainer.addView(desc)

        // Guide Card
        val guideCard = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            val pad = dp(10)
            setPadding(pad, pad, pad, pad)
            background = GradientDrawable().apply {
                setColor("#F8FAFC".toColorInt())
                cornerRadius = dp(12).toFloat()
                setStroke(dp(1), "#E2E8F0".toColorInt())
            }
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                bottomMargin = dp(8)
            }
        }

        val step1 = TextView(context).apply {
            text = "1️⃣  Pastdagi «Sozlamalarga o‘tish»ni bosing"
            textSize = 12.5f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor("#1E293B".toColorInt())
            setPadding(0, 0, 0, dp(4))
        }
        val step2 = TextView(context).apply {
            text = "2️⃣  Ro‘yxatdan «Harfiy»ni topib, kalitni yoqing"
            textSize = 12.5f
            setTextColor("#334155".toColorInt())
            setPadding(0, 0, 0, dp(4))
        }
        val step3 = TextView(context).apply {
            text = "🛡️  Bu ilova ochiq dasturlardan yopilsa ham ishlashini ta’minlaydi"
            textSize = 11.5f
            setTextColor("#2563EB".toColorInt())
        }
        guideCard.addView(step1)
        guideCard.addView(step2)
        guideCard.addView(step3)
        contentContainer.addView(guideCard)

        // Status Pill
        val statusPill = TextView(context).apply {
            text = if (isAutostartGranted) "🟢 FONDA ISHLASHGA RUXSAT BERILDI" else "🔴 HOZIRCHA O‘CHIQ (RUXSAT SHART)"
            textSize = 11.5f
            typeface = Typeface.DEFAULT_BOLD
            gravity = Gravity.CENTER
            val padH = dp(10)
            val padV = dp(6)
            setPadding(padH, padV, padH, padV)
            setTextColor(if (isAutostartGranted) "#047857".toColorInt() else "#B91C1C".toColorInt())
            background = GradientDrawable().apply {
                setColor(if (isAutostartGranted) "#D1FAE5".toColorInt() else "#FEE2E2".toColorInt())
                cornerRadius = dp(8).toFloat()
            }
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                bottomMargin = dp(6)
            }
        }
        contentContainer.addView(statusPill)

        if (!isAutostartGranted) {
            val settingsBtn = createPrimaryButton("Sozlamalarga o‘tish ⚙️") {
                openXiaomiAutostartSettings()
            }
            contentContainer.addView(settingsBtn)

            val checkBtn = createSecondaryButton("🔄 Qayta tekshirish") {
                if (isXiaomiAutostartEnabled()) {
                    renderStep(3)
                } else {
                    Toast.makeText(activity, "Hali yoqilmadi. Iltimos, ro‘yxatdan Harfiy'ni yoqing.", Toast.LENGTH_SHORT).show()
                }
            }
            contentContainer.addView(checkBtn)
        } else {
            val isWizardCompleted = prefs.getBoolean("has_completed_wizard", false)
            val btnText = if (isWizardCompleted) "Tayyor, ilovaga o‘tish 🚀" else "Ajoyib, keyingi qadam ›"
            val nextBtn = createPrimaryButton(btnText) {
                checkHandler.removeCallbacksAndMessages(null)
                if (isWizardCompleted) {
                    dismiss()
                    onFinished()
                } else {
                    renderStep(4)
                }
            }
            contentContainer.addView(nextBtn)
        }
    }

    // =========================================================================
    // STEP: Yuqori tezkor menyuga qo‘shish (MAJBURIY / REQUIRED)
    // =========================================================================
    private fun buildStepQuickControls(stepNumber: Int) {
        hasTileAdded = false

        val iconBadge = TextView(context).apply {
            text = "⚡"
            textSize = 28f
            gravity = Gravity.CENTER
            layoutParams = LinearLayout.LayoutParams(dp(52), dp(52)).apply {
                gravity = Gravity.CENTER_HORIZONTAL
                bottomMargin = dp(8)
            }
            background = GradientDrawable().apply {
                setColor("#FEF3C7".toColorInt())
                cornerRadius = dp(16).toFloat()
            }
        }
        contentContainer.addView(iconBadge)

        val title = TextView(context).apply {
            text = "$stepNumber-vazifa: Tezkor boshqarish tugmasi"
            textSize = 19f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor("#0F172A".toColorInt())
            gravity = Gravity.CENTER_HORIZONTAL
            setPadding(0, 0, 0, dp(4))
        }
        contentContainer.addView(title)

        val desc = TextView(context).apply {
            text = "Ekranning yuqori boshqaruv paneliga Harfiy tugmasini qo‘shamiz."
            textSize = 13f
            setTextColor("#475569".toColorInt())
            gravity = Gravity.CENTER_HORIZONTAL
            setPadding(0, 0, 0, dp(8))
        }
        contentContainer.addView(desc)

        // Compact Analogy Card
        val analogyCard = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            val pad = dp(10)
            setPadding(pad, pad, pad, pad)
            background = GradientDrawable().apply {
                setColor("#F8FAFC".toColorInt())
                cornerRadius = dp(12).toFloat()
                setStroke(dp(1), "#E2E8F0".toColorInt())
            }
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                bottomMargin = dp(8)
            }
        }

        val analogyTitle = TextView(context).apply {
            text = "💡 Bu nima uchun kerak?"
            textSize = 12.5f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor("#0F172A".toColorInt())
            setPadding(0, 0, 0, dp(3))
        }
        val analogyBody = TextView(context).apply {
            text = "Bank ilovalari yoki inglizcha parol yozayotganda ekranning yuqorisidan Harfiy'ni 1 marta bosishda to‘xtatib turishingiz mumkin."
            textSize = 11.5f
            setTextColor("#334155".toColorInt())
            setLineSpacing(dp(1).toFloat(), 1.15f)
        }
        analogyCard.addView(analogyTitle)
        analogyCard.addView(analogyBody)
        contentContainer.addView(analogyCard)

        // Status indicator
        tileStatusPill = TextView(context).apply {
            text = "⏳ Davom etish uchun avval quyidagi tugmani bosing"
            textSize = 11.5f
            typeface = Typeface.DEFAULT_BOLD
            gravity = Gravity.CENTER
            val padH = dp(10)
            val padV = dp(6)
            setPadding(padH, padV, padH, padV)
            setTextColor("#B45309".toColorInt())
            background = GradientDrawable().apply {
                setColor("#FEF3C7".toColorInt())
                cornerRadius = dp(8).toFloat()
            }
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                bottomMargin = dp(8)
            }
        }
        contentContainer.addView(tileStatusPill)

        // Transforming Action Button (Saves screen height, prevents scrolling!)
        tileNextBtn = createPrimaryButton("⚡ Tezkor boshqarish tugmasini qo‘shish") {
            if (!hasTileAdded) {
                performAddTileAction()
            } else {
                renderStep(if (isXiaomi) 5 else 4)
            }
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
                .setTitle("Tezkor boshqarish tugmasi")
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
            text = "🟢 TEZKOR BOSHQARISH TUGMASI QO‘SHILDI!"
            setTextColor("#047857".toColorInt())
            (background as GradientDrawable).setColor("#D1FAE5".toColorInt())
        }
        tileNextBtn?.apply {
            text = "Ajoyib, keyingi qadam ›"
            isEnabled = true
            alpha = 1.0f
            (background as GradientDrawable).setColor("#2563EB".toColorInt())
            setOnClickListener {
                renderStep(if (isXiaomi) 5 else 4)
            }
        }
    }

    // =========================================================================
    // STEP: Klaviaturada sinov & Xatoni qaytarish (MAJBURIY / REQUIRED)
    // =========================================================================
    private fun buildStepLiveTestAndUndo(stepNumber: Int) {
        hasTypingTested = false

        val iconBadge = TextView(context).apply {
            text = "🎯"
            textSize = 28f
            gravity = Gravity.CENTER
            layoutParams = LinearLayout.LayoutParams(dp(52), dp(52)).apply {
                gravity = Gravity.CENTER_HORIZONTAL
                bottomMargin = dp(8)
            }
            background = GradientDrawable().apply {
                setColor("#ECFDF5".toColorInt())
                cornerRadius = dp(16).toFloat()
            }
        }
        contentContainer.addView(iconBadge)

        val title = TextView(context).apply {
            text = "$stepNumber-vazifa: Klaviaturada sinab ko‘ring"
            textSize = 19f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor("#0F172A".toColorInt())
            gravity = Gravity.CENTER_HORIZONTAL
            setPadding(0, 0, 0, dp(4))
        }
        contentContainer.addView(title)

        val desc = TextView(context).apply {
            text = "Pastdagi maydonga «shahar», «choy» yoki «o‘rik» deb yozing:"
            textSize = 13f
            setTextColor("#475569".toColorInt())
            gravity = Gravity.CENTER_HORIZONTAL
            setPadding(0, 0, 0, dp(8))
        }
        contentContainer.addView(desc)

        // Live interactive input
        val inputContainer = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            val pad = dp(10)
            setPadding(pad, pad, pad, pad)
            background = GradientDrawable().apply {
                setColor("#F8FAFC".toColorInt())
                cornerRadius = dp(12).toFloat()
                setStroke(dp(1), "#CBD5E1".toColorInt())
            }
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                bottomMargin = dp(6)
            }
        }

        liveTestEditText = EditText(context).apply {
            hint = "Shu yerga yozing..."
            setHintTextColor("#94A3B8".toColorInt())
            textSize = 15f
            setTextColor("#0F172A".toColorInt())
            background = null
            setPadding(0, 0, 0, 0)
            maxLines = 1
        }
        inputContainer.addView(liveTestEditText)
        contentContainer.addView(inputContainer)

        // Status instruction
        liveTestSuccessLabel = TextView(context).apply {
            text = "⏳ Davom etish uchun maydonga biror so‘z yozing"
            textSize = 11.5f
            setTextColor("#B45309".toColorInt())
            gravity = Gravity.CENTER_HORIZONTAL
            setPadding(0, 0, 0, dp(6))
        }
        contentContainer.addView(liveTestSuccessLabel)

        // Compact Educational Undo Box
        val undoBox = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            val pad = dp(8)
            setPadding(pad, pad, pad, pad)
            background = GradientDrawable().apply {
                setColor("#EFF6FF".toColorInt())
                cornerRadius = dp(10).toFloat()
            }
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            ).apply {
                bottomMargin = dp(8)
            }
        }

        val undoTitle = TextView(context).apply {
            text = "💡 Eng muhim qoida: Xatoni qaytarish"
            textSize = 12f
            typeface = Typeface.DEFAULT_BOLD
            setTextColor("#1D4ED8".toColorInt())
            setPadding(0, 0, 0, dp(2))
        }
        val undoText = TextView(context).apply {
            text = "Agar harf noto‘g‘ri o‘zgarib qolsa, shunchaki klaviaturadagi O‘chirish (⌫) tugmasini 1 marta bosing — darhol asl holiga qaytadi."
            textSize = 11.5f
            setTextColor("#1E40AF".toColorInt())
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
            prefs.edit { putBoolean("has_completed_wizard", true) }
            dismiss()
            onFinished()
        }.apply {
            isEnabled = false
            alpha = 0.5f
            (background as GradientDrawable).setColor("#94A3B8".toColorInt())
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
                        setTextColor("#059669".toColorInt())
                        typeface = Typeface.DEFAULT_BOLD
                    }

                    finishButton?.apply {
                        isEnabled = true
                        alpha = 1.0f
                        (background as GradientDrawable).setColor("#2563EB".toColorInt())
                    }
                }
            }
        })
    }

    // =========================================================================
    // UI Helpers
    // =========================================================================
    private fun createCompactFeature(emoji: String, text: String): LinearLayout {
        return LinearLayout(context).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(0, 0, 0, dp(8))

            val icon = TextView(context).apply {
                this.text = emoji
                textSize = 14f
                setPadding(0, 0, dp(8), 0)
            }
            val label = TextView(context).apply {
                this.text = text
                textSize = 12.5f
                setTextColor("#334155".toColorInt())
            }
            addView(icon)
            addView(label)
        }
    }

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
                    setColor("#F1F5F9".toColorInt())
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
                    setTextColor("#0F172A".toColorInt())
                }
                val d = TextView(context).apply {
                    this.text = desc
                    textSize = 12f
                    setTextColor("#64748B".toColorInt())
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
                setColor("#2563EB".toColorInt())
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
            setTextColor("#64748B".toColorInt())
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

    private fun isXiaomiOrMiui(): Boolean {
        val man = Build.MANUFACTURER.lowercase()
        val brand = Build.BRAND.lowercase()
        return man.contains("xiaomi") || man.contains("redmi") || man.contains("poco") ||
                brand.contains("xiaomi") || brand.contains("redmi") || brand.contains("poco")
    }

    private fun isXiaomiAutostartEnabled(): Boolean {
        val appOps = context.getSystemService(Context.APP_OPS_SERVICE) as? AppOpsManager ?: return false
        return try {
            val checkOpNoThrowMethod = AppOpsManager::class.java.getMethod(
                "checkOpNoThrow",
                Int::class.javaPrimitiveType,
                Int::class.javaPrimitiveType,
                String::class.java
            )
            val uid = Process.myUid()
            val mode10008 = checkOpNoThrowMethod.invoke(appOps, 10008, uid, context.packageName) as? Int ?: -1
            if (mode10008 == AppOpsManager.MODE_ALLOWED) {
                return true
            }
            val mode10021 = checkOpNoThrowMethod.invoke(appOps, 10021, uid, context.packageName) as? Int ?: -1
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
            activity.startActivity(intent)
            Toast.makeText(activity, "Ro‘yxatdan «Harfiy»ni topib yoqing", Toast.LENGTH_LONG).show()
        } catch (_: Exception) {
            try {
                val intent = Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS).apply {
                    data = Uri.fromParts("package", context.packageName, null)
                    addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                }
                activity.startActivity(intent)
            } catch (_: Exception) {
                Toast.makeText(activity, "Sozlamalarni ochib bo‘lmadi", Toast.LENGTH_SHORT).show()
            }
        }
    }
}
