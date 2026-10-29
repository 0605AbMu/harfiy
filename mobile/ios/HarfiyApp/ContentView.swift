//
//  ContentView.swift
//  HarfiyApp
//
//  World-Class, Ultra-Modern Mobile Dashboard for Harfiy on iOS.
//  Matches Android senior UI/UX standards:
//  - Grand Hero Status Card with live keyboard status
//  - Grouped settings with custom icon badges and dividers
//  - Unified Magic Text Playground & Batch Converter
//  - Gamified impact & statistics with visual character pills
//  - Step-by-Step interactive Setup Wizard
//  - Official community & author links
//

import SwiftUI
import UIKit
import KeyboardKit

public struct ContentView: View {
    @ObservedObject var settings = HarfiySettings.shared
    @StateObject private var keyboardStatus = KeyboardStatusContext(bundleId: "uz.harfiy.keyboard.*")
    @State private var practiceText: String = ""
    @State private var showingWizard: Bool = false
    @State private var showingClearStatsAlert: Bool = false
    @State private var toastMessage: String? = nil
    @State private var isKeyboardActiveInSystem: Bool = false
    @FocusState private var isFieldFocused: Bool

    public init() {}

    public var body: some View {
        NavigationView {
            ZStack {
                Color(hex: "#F8FAFC")
                    .ignoresSafeArea()

                ScrollView {
                    VStack(spacing: 20) {
                        // 1. Top App Bar
                        appBarView

                        // 2. Grand Hero Status Card
                        heroStatusCard

                        // 3. Grouped Settings
                        settingsSection

                        // 4. Magic Text Playground & Batch Converter
                        playgroundSection

                        // 5. Gamified Statistics & Impact
                        statisticsSection

                        // 6. Project & Community Links
                        communitySection

                        // 7. Footer
                        footerView
                    }
                    .padding(.horizontal, 18)
                    .padding(.top, 12)
                    .padding(.bottom, 40)
                }

                // Toast notification banner
                if let msg = toastMessage {
                    VStack {
                        Spacer()
                        toastView(message: msg)
                            .transition(.move(edge: .bottom).combined(with: .opacity))
                            .padding(.bottom, 24)
                    }
                }
            }
            .navigationBarHidden(true)
            .toolbar {
                ToolbarItemGroup(placement: .keyboard) {
                    Spacer()
                    Button("Tayyor") { isFieldFocused = false }
                        .font(.system(size: 16, weight: .semibold))
                        .foregroundColor(Color(hex: "#2563EB"))
                }
            }
            .sheet(isPresented: $showingWizard) {
                SetupWizardView(isPresented: $showingWizard, initialStep: isKeyboardActiveInSystem ? 1 : 2)
            }
            .alert(isPresented: $showingClearStatsAlert) {
                Alert(
                    title: Text("Hisoblagichni tozalash"),
                    message: Text("Barcha to‘g‘rilangan harflar va tejalgan vaqt ko‘rsatkichlari nolga tushiriladi. Davom etasizmi?"),
                    primaryButton: .destructive(Text("Tozalash")) {
                        settings.clearStats()
                        showToast("Natijalar tozalandi")
                    },
                    secondaryButton: .cancel(Text("Bekor qilish"))
                )
            }
            .onAppear {
                checkKeyboardSystemStatus()
                if !isKeyboardActiveInSystem {
                    showingWizard = true
                }
            }
            .onReceive(NotificationCenter.default.publisher(for: UIApplication.willEnterForegroundNotification)) { _ in
                checkKeyboardSystemStatus()
                settings.reloadStats()
                if !isKeyboardActiveInSystem {
                    showingWizard = true
                }
            }
        }
        .navigationViewStyle(StackNavigationViewStyle())
    }

    // MARK: - 1. Top App Bar
    private var appBarView: some View {
        HStack(alignment: .center) {
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 8) {
                    Text("Harfiy")
                        .font(.system(size: 28, weight: .bold))
                        .foregroundColor(Color(hex: "#0F172A"))

                    Button(action: openAppStoreOrSite) {
                        Text("v1.0.1 ↗")
                            .font(.system(size: 12, weight: .bold))
                            .foregroundColor(Color(hex: "#2563EB"))
                            .padding(.horizontal, 8)
                            .padding(.vertical, 3)
                            .background(Color(hex: "#EFF6FF"))
                            .cornerRadius(8)
                    }
                }

                Text("Yozuvni avtomatik to‘g‘rilovchi aqlli yordamchi")
                    .font(.system(size: 12))
                    .foregroundColor(Color(hex: "#64748B"))
            }

            Spacer()
        }
        .padding(.top, 4)
        .padding(.bottom, 2)
    }

    // MARK: - 2. Grand Hero Status Card
    private var heroStatusCard: some View {
        VStack(alignment: .leading, spacing: 12) {
            // Badge
            HStack(spacing: 6) {
                Circle()
                    .fill(isKeyboardActiveInSystem ? Color(hex: "#10B981") : Color(hex: "#EF4444"))
                    .frame(width: 8, height: 8)

                Text(isKeyboardActiveInSystem ? "FAOL VA TAYYOR" : "TIZIMDA QO‘SHILMAGAN")
                    .font(.system(size: 11, weight: .bold))
                    .foregroundColor(isKeyboardActiveInSystem ? Color(hex: "#059669") : Color(hex: "#DC2626"))
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 4)
            .background(isKeyboardActiveInSystem ? Color(hex: "#ECFDF5") : Color(hex: "#FEF2F2"))
            .cornerRadius(8)

            // Main Info Row
            HStack(alignment: .center) {
                VStack(alignment: .leading, spacing: 4) {
                    Text(isKeyboardActiveInSystem ? "Harfiy klaviaturasi faol" : "Harfiyni yoqing")
                        .font(.system(size: 21, weight: .bold))
                        .foregroundColor(Color(hex: "#0F172A"))

                    Text(isKeyboardActiveInSystem
                        ? "Klaviatura bilan yozganingizda avtomatik to‘g‘rilaydi"
                        : "Klaviaturadan foydalanish uchun sozlamalardan faollashtiring")
                        .font(.system(size: 13))
                        .foregroundColor(Color(hex: "#64748B"))
                        .fixedSize(horizontal: false, vertical: true)
                }

                Spacer()

                Button(action: {
                    if !isKeyboardActiveInSystem {
                        showingWizard = true
                    } else {
                        openSettings()
                    }
                }) {
                    HStack(spacing: 4) {
                        Text(isKeyboardActiveInSystem ? "Sozlash" : "Yoqish")
                            .font(.system(size: 13, weight: .bold))
                        Image(systemName: "chevron.right")
                            .font(.system(size: 11, weight: .bold))
                    }
                    .foregroundColor(.white)
                    .padding(.horizontal, 14)
                    .padding(.vertical, 8)
                    .background(Color(hex: "#2563EB"))
                    .cornerRadius(10)
                }
            }
        }
        .padding(18)
        .background(Color.white)
        .cornerRadius(20)
        .overlay(
            RoundedRectangle(cornerRadius: 20)
                .stroke(isKeyboardActiveInSystem ? Color(hex: "#E2E8F0") : Color(hex: "#FECACA"), lineWidth: 1)
        )
    }

    // MARK: - 3. Grouped Settings
    private var settingsSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            sectionHeader("QULAY SOZLAMALAR")

            VStack(spacing: 0) {
                // Setting 1: Instant Undo
                settingToggleRow(
                    icon: "↩️",
                    iconBg: "#EFF6FF",
                    title: "Xatoni qaytarish",
                    subtitle: "Harf noto‘g‘ri o‘zgarganda, Backspace bosilsa asl holiga qaytadi",
                    isOn: $settings.instantUndo
                )

                settingsDivider

                // Setting 2: «ng»
                settingToggleRow(
                    icon: "🔤",
                    iconBg: "#F5F3FF",
                    title: "«ng» harfini almashtirish",
                    subtitle: "«ng» yozganingizda avtomatik «ñ» ga o‘giriladi",
                    isOn: $settings.enableNg
                )

                settingsDivider

                // Setting 3: Haptic Feedback
                settingToggleRow(
                    icon: "📳",
                    iconBg: "#ECFDF5",
                    title: "Haptic tebranish",
                    subtitle: "Tugmalar bosilganda yoqimli tebranish berish",
                    isOn: $settings.hapticFeedback
                )

                settingsDivider

                // Setting 4: Sound Feedback
                settingToggleRow(
                    icon: "🔊",
                    iconBg: "#FDF2F8",
                    title: "Tugma bosish tovushi",
                    subtitle: "Tugmalar bosilganda chertish ovozi chiqarish",
                    isOn: $settings.audioFeedback
                )

                settingsDivider

                // Setting 6: Setup Wizard Navigation
                Button(action: { showingWizard = true }) {
                    HStack(spacing: 12) {
                        badgeView(emoji: "🧭", bgColor: "#E0F2FE")

                        VStack(alignment: .leading, spacing: 2) {
                            Text("Boshlang‘ich sozlash ustasi (Wizard)")
                                .font(.system(size: 15, weight: .bold))
                                .foregroundColor(Color(hex: "#0F172A"))
                                .multilineTextAlignment(.leading)

                            Text("Klaviaturani yoqish va ishlatish bo‘yicha qo‘llanma")
                                .font(.system(size: 12))
                                .foregroundColor(Color(hex: "#64748B"))
                                .multilineTextAlignment(.leading)
                                .fixedSize(horizontal: false, vertical: true)
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)

                        Spacer()

                        Image(systemName: "chevron.right")
                            .font(.system(size: 14, weight: .bold))
                            .foregroundColor(Color(hex: "#94A3B8"))
                    }
                    .padding(.vertical, 12)
                }
            }
            .padding(.horizontal, 16)
            .background(Color.white)
            .cornerRadius(20)
            .overlay(
                RoundedRectangle(cornerRadius: 20)
                    .stroke(Color(hex: "#E2E8F0"), lineWidth: 1)
            )
        }
    }

    // MARK: - 4. Unified Magic Text Playground & Batch Converter
    private var playgroundSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            sectionHeader("MATN MAYDONI VA SINOV")

            VStack(alignment: .leading, spacing: 12) {
                // Header
                HStack(spacing: 8) {
                    Text("🪄")
                        .font(.system(size: 18))
                    Text("Yozib ko‘ring yoki matnni o‘giring")
                        .font(.system(size: 16, weight: .bold))
                        .foregroundColor(Color(hex: "#0F172A"))
                }

                Text("Klaviaturangizda «sh, ch, o', g'» deb yozing yoki nusxalangan matnni qo‘ying:")
                    .font(.system(size: 13))
                    .foregroundColor(Color(hex: "#64748B"))

                // Multi-line Text Editor
                ZStack(alignment: .topLeading) {
                    if practiceText.isEmpty {
                        Text("Yozing (masalan: shahar, choy, o‘rik) yoki matn nusxasini qo‘ying...")
                            .font(.system(size: 15))
                            .foregroundColor(Color(hex: "#94A3B8"))
                            .padding(.horizontal, 14)
                            .padding(.vertical, 12)
                            .allowsHitTesting(false)
                    }

                    TextEditor(text: $practiceText)
                        .focused($isFieldFocused)
                        .font(.system(size: 15))
                        .foregroundColor(Color(hex: "#0F172A"))
                        .frame(minHeight: 90)
                        .padding(8)
                        .background(Color.clear)
                }
                .background(Color(hex: "#F8FAFC"))
                .cornerRadius(14)
                .overlay(
                    RoundedRectangle(cornerRadius: 14)
                        .stroke(Color(hex: "#E2E8F0"), lineWidth: 1)
                )

                // Buttons Row
                HStack(spacing: 8) {
                    // O'girish
                    Button(action: convertPracticeText) {
                        HStack(spacing: 4) {
                            Text("🪄")
                            Text("O‘girish")
                                .fontWeight(.bold)
                        }
                        .font(.system(size: 13))
                        .foregroundColor(.white)
                        .frame(maxWidth: .infinity)
                        .frame(height: 42)
                        .background(Color(hex: "#2563EB"))
                        .cornerRadius(12)
                    }

                    // Nusxa
                    Button(action: copyPracticeText) {
                        HStack(spacing: 4) {
                            Text("📋")
                            Text("Nusxa")
                                .fontWeight(.bold)
                        }
                        .font(.system(size: 13))
                        .foregroundColor(Color(hex: "#1E293B"))
                        .frame(maxWidth: .infinity)
                        .frame(height: 42)
                        .background(Color(hex: "#F1F5F9"))
                        .cornerRadius(12)
                        .overlay(
                            RoundedRectangle(cornerRadius: 12)
                                .stroke(Color(hex: "#E2E8F0"), lineWidth: 1)
                        )
                    }

                    // Tozalash
                    Button(action: { practiceText = "" }) {
                        Text("Tozalash")
                            .font(.system(size: 13, weight: .bold))
                            .foregroundColor(Color(hex: "#64748B"))
                            .frame(maxWidth: .infinity)
                            .frame(height: 42)
                            .background(Color(hex: "#F8FAFC"))
                            .cornerRadius(12)
                            .overlay(
                                RoundedRectangle(cornerRadius: 12)
                                    .stroke(Color(hex: "#E2E8F0"), lineWidth: 1)
                            )
                    }
                }
            }
            .padding(16)
            .background(Color.white)
            .cornerRadius(20)
            .overlay(
                RoundedRectangle(cornerRadius: 20)
                    .stroke(Color(hex: "#E2E8F0"), lineWidth: 1)
            )
        }
    }

    // MARK: - 5. Gamified Statistics & Impact
    private var statisticsSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            sectionHeader("SIZNING NATIJALARINGIZ")

            VStack(spacing: 14) {
                // Header
                HStack(spacing: 8) {
                    Text("📊")
                        .font(.system(size: 18))
                    Text("Harfiy bilan tejalgan vaqtingiz")
                        .font(.system(size: 16, weight: .bold))
                        .foregroundColor(Color(hex: "#0F172A"))
                    Spacer()
                }

                // Two Big Metric Cards
                HStack(spacing: 10) {
                    metricCard(
                        value: "\(settings.totalConversions) ta",
                        label: "To‘g‘rilangan harflar",
                        valueColor: "#2563EB",
                        bgColor: "#EFF6FF"
                    )

                    metricCard(
                        value: String(format: "%.1f soniya", settings.timeSavedSeconds),
                        label: "Tejalgan vaqt",
                        valueColor: "#059669",
                        bgColor: "#ECFDF5"
                    )
                }

                // Breakdown Pills
                HStack(spacing: 6) {
                    statPill(label: "ş", count: settings.shCount)
                    statPill(label: "ç", count: settings.chCount)
                    statPill(label: "ö", count: settings.oCount)
                    statPill(label: "ğ", count: settings.gCount)
                    statPill(label: "ñ", count: settings.ngCount)
                }

                // Clear Stats Button
                Button(action: { showingClearStatsAlert = true }) {
                    Text("Hisoblagichni tozalash")
                        .font(.system(size: 12))
                        .foregroundColor(Color(hex: "#94A3B8"))
                        .padding(.top, 2)
                }
            }
            .padding(16)
            .background(Color.white)
            .cornerRadius(20)
            .overlay(
                RoundedRectangle(cornerRadius: 20)
                    .stroke(Color(hex: "#E2E8F0"), lineWidth: 1)
            )
        }
    }

    // MARK: - 6. Project & Community Links
    private var communitySection: some View {
        VStack(alignment: .leading, spacing: 8) {
            sectionHeader("LOYIHA VA HAMJAMIYAT")

            VStack(spacing: 10) {
                linkCardRow(
                    emoji: "🌐",
                    badgeBg: "#EFF6FF",
                    title: "Rasmiy veb-sahifa",
                    subtitle: "harfiy.uz • Yangiliklar va qo‘llanma",
                    url: "https://abdumannon-shamsiyev.uz/harfiy"
                )

                linkCardRow(
                    emoji: "📱",
                    badgeBg: "#E0F2FE",
                    title: "Telegram kanal",
                    subtitle: "@abdumannon_notes • Muallif blogi",
                    url: "https://t.me/abdumannon_notes"
                )

                linkCardRow(
                    emoji: "💬",
                    badgeBg: "#ECFDF5",
                    title: "Savollar va takliflar",
                    subtitle: "@abdumannon_shamsiyev • To‘g‘ridan-to‘g‘ri aloqa",
                    url: "https://t.me/abdumannon_shamsiyev"
                )

                linkCardRow(
                    emoji: "🚀",
                    badgeBg: "#F5F3FF",
                    title: "Ilova versiyasi",
                    subtitle: "Harfiy iOS v1.0.1 • 100% Oflayn va bepul",
                    url: "https://abdumannon-shamsiyev.uz/harfiy"
                )
            }
        }
    }

    // MARK: - 7. Footer
    private var footerView: some View {
        VStack(spacing: 4) {
            Text("Harfiy • 100% oflayn va xavfsiz")
                .font(.system(size: 12))
                .foregroundColor(Color(hex: "#94A3B8"))

            Text("O‘zbekiston uchun mehr bilan yaratildi ❤️")
                .font(.system(size: 12))
                .foregroundColor(Color(hex: "#94A3B8"))
        }
        .padding(.vertical, 16)
        .frame(maxWidth: .infinity)
    }

    // MARK: - Helper Views & Actions

    private func sectionHeader(_ title: String) -> some View {
        Text(title)
            .font(.system(size: 12, weight: .bold))
            .foregroundColor(Color(hex: "#64748B"))
            .tracking(0.5)
            .padding(.leading, 4)
            .padding(.top, 4)
    }

    private var settingsDivider: some View {
        Divider()
            .padding(.leading, 48)
            .background(Color(hex: "#F1F5F9"))
    }

    private func badgeView(emoji: String, bgColor: String) -> some View {
        Text(emoji)
            .font(.system(size: 16))
            .frame(width: 36, height: 36)
            .background(Color(hex: bgColor))
            .cornerRadius(10)
    }

    private func settingToggleRow(
        icon: String,
        iconBg: String,
        title: String,
        subtitle: String,
        isOn: Binding<Bool>
    ) -> some View {
        HStack(spacing: 12) {
            badgeView(emoji: icon, bgColor: iconBg)

            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(.system(size: 15, weight: .bold))
                    .foregroundColor(Color(hex: "#0F172A"))

                Text(subtitle)
                    .font(.system(size: 12))
                    .foregroundColor(Color(hex: "#64748B"))
                    .fixedSize(horizontal: false, vertical: true)
            }

            Spacer()

            Toggle("", isOn: isOn)
                .labelsHidden()
        }
        .padding(.vertical, 10)
    }

    private func metricCard(value: String, label: String, valueColor: String, bgColor: String) -> some View {
        VStack(spacing: 4) {
            Text(value)
                .font(.system(size: 20, weight: .bold))
                .foregroundColor(Color(hex: valueColor))

            Text(label)
                .font(.system(size: 11.5))
                .foregroundColor(Color(hex: "#64748B"))
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 14)
        .background(Color(hex: bgColor))
        .cornerRadius(14)
    }

    private func statPill(label: String, count: Int) -> some View {
        HStack(spacing: 3) {
            Text(label)
                .font(.system(size: 12, weight: .bold))
                .foregroundColor(Color(hex: "#2563EB"))
            Text(":\(count)")
                .font(.system(size: 11, weight: .medium))
                .foregroundColor(Color(hex: "#64748B"))
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 4)
        .background(Color(hex: "#F1F5F9"))
        .cornerRadius(8)
    }

    private func linkCardRow(emoji: String, badgeBg: String, title: String, subtitle: String, url: String) -> some View {
        Button(action: {
            if let linkUrl = URL(string: url) {
                UIApplication.shared.open(linkUrl)
            }
        }) {
            HStack(spacing: 12) {
                badgeView(emoji: emoji, bgColor: badgeBg)

                VStack(alignment: .leading, spacing: 2) {
                    Text(title)
                        .font(.system(size: 15, weight: .bold))
                        .foregroundColor(Color(hex: "#0F172A"))
                        .multilineTextAlignment(.leading)

                    Text(subtitle)
                        .font(.system(size: 12))
                        .foregroundColor(Color(hex: "#64748B"))
                        .multilineTextAlignment(.leading)
                }
                .frame(maxWidth: .infinity, alignment: .leading)

                Spacer()

                Image(systemName: "chevron.right")
                    .font(.system(size: 14, weight: .bold))
                    .foregroundColor(Color(hex: "#94A3B8"))
            }
            .padding(14)
            .background(Color.white)
            .cornerRadius(16)
            .overlay(
                RoundedRectangle(cornerRadius: 16)
                    .stroke(Color(hex: "#E2E8F0"), lineWidth: 1)
            )
        }
    }

    private func toastView(message: String) -> some View {
        HStack(spacing: 8) {
            Image(systemName: "checkmark.circle.fill")
                .foregroundColor(Color(hex: "#10B981"))
            Text(message)
                .font(.system(size: 14, weight: .semibold))
                .foregroundColor(.white)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
        .background(Color(hex: "#0F172A").opacity(0.92))
        .cornerRadius(20)
        .shadow(color: Color.black.opacity(0.15), radius: 8, x: 0, y: 4)
    }

    private func showToast(_ text: String) {
        withAnimation {
            toastMessage = text
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 2.0) {
            withAnimation {
                toastMessage = nil
            }
        }
    }

    private func convertPracticeText() {
        guard !practiceText.isEmpty else { return }
        let (converted, count) = HarfiyEngineBridge.convertText(
            practiceText,
            enableNg: settings.enableNg,
            smartApostrophe: true
        )
        practiceText = converted
        if count > 0 {
            settings.recordBatchConversions(totalAdded: count)
        }
        showToast("Matn to‘liq o‘girildi!")
    }

    private func copyPracticeText() {
        guard !practiceText.isEmpty else { return }
        UIPasteboard.general.string = practiceText
        showToast("Nusxalandi!")
    }

    private func openSettings() {
        if let url = URL(string: UIApplication.openSettingsURLString) {
            UIApplication.shared.open(url)
        }
    }

    private func openAppStoreOrSite() {
        if let url = URL(string: "https://abdumannon-shamsiyev.uz/harfiy") {
            UIApplication.shared.open(url)
        }
    }

    private func checkKeyboardSystemStatus() {
        keyboardStatus.refresh()
        var enabled = keyboardStatus.isKeyboardEnabled || keyboardStatus.isKeyboardActive
        if !enabled {
            enabled = HarfiySettings.isKeyboardActiveInSystem()
        }
        withAnimation {
            self.isKeyboardActiveInSystem = enabled
        }
    }
}

#if DEBUG
struct ContentView_Previews: PreviewProvider {
    static var previews: some View {
        ContentView()
    }
}
#endif
