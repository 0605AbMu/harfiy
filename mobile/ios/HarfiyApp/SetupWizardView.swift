//
//  SetupWizardView.swift
//  HarfiyApp
//
//  Interactive Step-by-Step Onboarding Wizard for Harfiy on iOS.
//

import SwiftUI
import UIKit

public struct SetupWizardView: View {
    @ObservedObject var settings = HarfiySettings.shared
    @Binding var isPresented: Bool

    @State private var currentStep: Int = 1
    @State private var testInput: String = ""
    @State private var hasTestedSuccessfully: Bool = false

    private let totalSteps = 4

    public init(isPresented: Binding<Bool>, initialStep: Int = 1) {
        self._isPresented = isPresented
        self._currentStep = State(initialValue: initialStep)
    }

    public var body: some View {
        NavigationView {
            ZStack {
                Color(hex: "#F8FAFC")
                    .ignoresSafeArea()

                VStack(spacing: 0) {
                    // Top Progress & Step Badge
                    HStack {
                        // Progress indicators
                        HStack(spacing: 6) {
                            ForEach(1...totalSteps, id: \.self) { step in
                                Capsule()
                                    .fill(stepColor(for: step))
                                    .frame(width: step == currentStep ? 24 : 8, height: 8)
                                    .animation(.spring(), value: currentStep)
                            }
                        }

                        Spacer()

                        // Step badge
                        Text("\(currentStep) / \(totalSteps)")
                            .font(.system(size: 12, weight: .bold))
                            .foregroundColor(Color(hex: "#2563EB"))
                            .padding(.horizontal, 10)
                            .padding(.vertical, 4)
                            .background(Color(hex: "#EFF6FF"))
                            .cornerRadius(8)

                        if settings.hasCompletedWizard {
                            Button(action: { isPresented = false }) {
                                Image(systemName: "xmark.circle.fill")
                                    .font(.title3)
                                    .foregroundColor(Color(hex: "#94A3B8"))
                            }
                            .padding(.leading, 8)
                        }
                    }
                    .padding(.horizontal, 20)
                    .padding(.top, 16)
                    .padding(.bottom, 12)

                    Divider()

                    // Step Content
                    ScrollView {
                        VStack(spacing: 20) {
                            switch currentStep {
                            case 1:
                                step1Welcome
                            case 2:
                                step2Activation
                            case 3:
                                step3UsageTips
                            case 4:
                                step4Practice
                            default:
                                step1Welcome
                            }
                        }
                        .padding(20)
                    }

                    // Bottom Navigation Buttons
                    bottomButtonBar
                }
            }
            .navigationBarHidden(true)
        }
    }

    private func stepColor(for step: Int) -> Color {
        if step == currentStep {
            return Color(hex: "#2563EB")
        } else if step < currentStep {
            return Color(hex: "#93C5FD")
        } else {
            return Color(hex: "#E2E8F0")
        }
    }

    // MARK: - Step 1: Welcome
    private var step1Welcome: some View {
        VStack(spacing: 18) {
            ZStack {
                Circle()
                    .fill(Color(hex: "#EFF6FF"))
                    .frame(width: 80, height: 80)
                Image(systemName: "sparkles")
                    .font(.system(size: 38))
                    .foregroundColor(Color(hex: "#2563EB"))
            }
            .padding(.top, 10)

            VStack(spacing: 8) {
                Text("Harfiy'ga xush kelibsiz!")
                    .font(.system(size: 24, weight: .bold))
                    .foregroundColor(Color(hex: "#0F172A"))
                    .multilineTextAlignment(.center)

                Text("O‘zbek lotin yozuvidagi qo‘shharflarni avtomatik to‘g‘rilovchi aqlli yordamchi.")
                    .font(.system(size: 14))
                    .foregroundColor(Color(hex: "#64748B"))
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 10)
            }

            VStack(spacing: 12) {
                featureRow(
                    icon: "⚡",
                    title: "Tez va tabiiy yozish",
                    subtitle: "«sh» yozsangiz ➔ «ş», «ch» ➔ «ç», «o'» ➔ «ö», «g'» ➔ «ğ» ga aylanadi"
                )
                featureRow(
                    icon: "🔒",
                    title: "100% Oflayn va Xavfsiz",
                    subtitle: "Internetga ulanmaydi, barcha harflar qurilmangiz ichida ishlanadi"
                )
                featureRow(
                    icon: "↩️",
                    title: "Xatoni qaytarish (Instant Undo)",
                    subtitle: "Harf noto‘g‘ri o‘zgarsa, Backspace bosilganda darhol asl holiga qaytadi"
                )
            }
            .padding(.top, 10)
        }
    }

    // MARK: - Step 2: System Activation
    private var step2Activation: some View {
        VStack(spacing: 18) {
            ZStack {
                Circle()
                    .fill(Color(hex: "#EFF6FF"))
                    .frame(width: 80, height: 80)
                Image(systemName: "keyboard.badge.ellipsis")
                    .font(.system(size: 36))
                    .foregroundColor(Color(hex: "#2563EB"))
            }
            .padding(.top, 10)

            VStack(spacing: 8) {
                Text("Klaviaturani tizimda yoqing")
                    .font(.system(size: 22, weight: .bold))
                    .foregroundColor(Color(hex: "#0F172A"))
                    .multilineTextAlignment(.center)

                Text("Harfiy ishlashi uchun iOS Sozlamalarida klaviaturani bir marta yoqib qo‘yishingiz kerak:")
                    .font(.system(size: 14))
                    .foregroundColor(Color(hex: "#64748B"))
                    .multilineTextAlignment(.center)
            }

            // Steps Guide Card
            VStack(alignment: .leading, spacing: 14) {
                wizardGuideStep(number: "1", title: "Sozlamalarga kiring", desc: "Telefoningiz Sozlamalari (Settings) ilovasi")
                wizardGuideStep(number: "2", title: "Asosiy ➔ Klaviatura", desc: "General ➔ Keyboard ➔ Keyboards bo‘limi")
                wizardGuideStep(number: "3", title: "Harfiy'ni tanlang", desc: "Add New Keyboard ➔ «Harfiy» ni bosing")
            }
            .padding(16)
            .background(Color.white)
            .cornerRadius(16)
            .overlay(
                RoundedRectangle(cornerRadius: 16)
                    .stroke(Color(hex: "#E2E8F0"), lineWidth: 1)
            )

            Button(action: openSystemSettings) {
                HStack(spacing: 8) {
                    Image(systemName: "gearshape.fill")
                    Text("Sozlamalarni ochish")
                        .fontWeight(.bold)
                }
                .font(.system(size: 15))
                .foregroundColor(.white)
                .frame(maxWidth: .infinity)
                .frame(height: 48)
                .background(Color(hex: "#2563EB"))
                .cornerRadius(12)
            }
            .padding(.top, 4)
        }
    }

    // MARK: - Step 3: Usage Tips
    private var step3UsageTips: some View {
        VStack(spacing: 18) {
            ZStack {
                Circle()
                    .fill(Color(hex: "#FEF3C7"))
                    .frame(width: 80, height: 80)
                Image(systemName: "globe")
                    .font(.system(size: 38))
                    .foregroundColor(Color(hex: "#D97706"))
            }
            .padding(.top, 10)

            VStack(spacing: 8) {
                Text("Qanday ishlatiladi?")
                    .font(.system(size: 22, weight: .bold))
                    .foregroundColor(Color(hex: "#0F172A"))
                    .multilineTextAlignment(.center)

                Text("Harfiy klaviaturasini boshqarish bo‘yicha muhim eslatmalar:")
                    .font(.system(size: 14))
                    .foregroundColor(Color(hex: "#64748B"))
                    .multilineTextAlignment(.center)
            }

            VStack(spacing: 12) {
                featureRow(
                    icon: "🌐",
                    title: "Klaviatura almashtirish",
                    subtitle: "Klaviaturadagi Dunyo (Globe) belgisini bosib, Harfiy va boshqa tillar orasida almashishingiz mumkin"
                )
                featureRow(
                    icon: "⌫",
                    title: "Bir zumda qaytarish",
                    subtitle: "Noto‘g‘ri o‘zgargan harf bo‘lsa, orqaga (Backspace) bosing — so‘z asl holiga qaytadi"
                )
                featureRow(
                    icon: "🔤",
                    title: "«ng» va maxsus sozlamalar",
                    subtitle: "Ilova sozlamalaridan «ng» harfini almashtirish, haptic tebranish va tovushlarni o‘zingizga moslang"
                )
            }
        }
    }

    // MARK: - Step 4: Practice & Finish
    private var step4Practice: some View {
        VStack(spacing: 18) {
            ZStack {
                Circle()
                    .fill(Color(hex: "#ECFDF5"))
                    .frame(width: 80, height: 80)
                Image(systemName: "checkmark.seal.fill")
                    .font(.system(size: 40))
                    .foregroundColor(Color(hex: "#059669"))
            }
            .padding(.top, 10)

            VStack(spacing: 8) {
                Text("Deyarli tayyormiz!")
                    .font(.system(size: 22, weight: .bold))
                    .foregroundColor(Color(hex: "#0F172A"))
                    .multilineTextAlignment(.center)

                Text("Quyidagi maydonda sinab ko‘ring (masalan: «shahar», «choy», «o'rik»):")
                    .font(.system(size: 14))
                    .foregroundColor(Color(hex: "#64748B"))
                    .multilineTextAlignment(.center)
            }

            VStack(alignment: .leading, spacing: 8) {
                TextField("Shu yerda yozib ko‘ring...", text: $testInput)
                    .padding(14)
                    .background(Color.white)
                    .cornerRadius(12)
                    .overlay(
                        RoundedRectangle(cornerRadius: 12)
                            .stroke(Color(hex: "#CBD5E1"), lineWidth: 1)
                    )
                    .onChange(of: testInput) { val in
                        let lower = val.lowercased()
                        if lower.contains("ş") || lower.contains("ç") || lower.contains("ö") || lower.contains("ğ") || lower.contains("ñ") {
                            hasTestedSuccessfully = true
                        }
                    }

                if hasTestedSuccessfully {
                    HStack(spacing: 6) {
                        Image(systemName: "checkmark.circle.fill")
                            .foregroundColor(Color(hex: "#059669"))
                        Text("Ajoyib! Harfiy avtomatik to‘g‘riladi.")
                            .font(.system(size: 13, weight: .medium))
                            .foregroundColor(Color(hex: "#059669"))
                    }
                    .padding(.top, 4)
                }
            }

            VStack(spacing: 8) {
                HStack(spacing: 8) {
                    practicePill("shahar ➔ şahar")
                    practicePill("choy ➔ çoy")
                }
                HStack(spacing: 8) {
                    practicePill("o'rik ➔ örik")
                    practicePill("g'isht ➔ ğişt")
                }
            }
            .padding(.top, 6)
        }
    }

    private func practicePill(_ text: String) -> some View {
        Text(text)
            .font(.system(size: 12, weight: .medium))
            .foregroundColor(Color(hex: "#475569"))
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(Color(hex: "#F1F5F9"))
            .cornerRadius(8)
    }

    // MARK: - Bottom Buttons
    private var bottomButtonBar: some View {
        VStack(spacing: 0) {
            Divider()
            HStack(spacing: 12) {
                if currentStep > 1 {
                    Button(action: {
                        withAnimation { currentStep -= 1 }
                    }) {
                        Text("Orqaga")
                            .font(.system(size: 15, weight: .semibold))
                            .foregroundColor(Color(hex: "#475569"))
                            .frame(maxWidth: .infinity)
                            .frame(height: 48)
                            .background(Color(hex: "#F1F5F9"))
                            .cornerRadius(12)
                    }
                }

                Button(action: {
                    if currentStep < totalSteps {
                        withAnimation { currentStep += 1 }
                    } else {
                        settings.hasCompletedWizard = true
                        isPresented = false
                    }
                }) {
                    Text(currentStep == totalSteps ? "Tayyor va Boshlash!" : "Davom etish ➔")
                        .font(.system(size: 15, weight: .bold))
                        .foregroundColor(.white)
                        .frame(maxWidth: .infinity)
                        .frame(height: 48)
                        .background(Color(hex: "#2563EB"))
                        .cornerRadius(12)
                }
            }
            .padding(16)
            .background(Color.white)
        }
    }

    private func featureRow(icon: String, title: String, subtitle: String) -> some View {
        HStack(alignment: .top, spacing: 14) {
            Text(icon)
                .font(.system(size: 20))
                .frame(width: 38, height: 38)
                .background(Color(hex: "#F8FAFC"))
                .cornerRadius(10)

            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(.system(size: 15, weight: .semibold))
                    .foregroundColor(Color(hex: "#0F172A"))
                Text(subtitle)
                    .font(.system(size: 12))
                    .foregroundColor(Color(hex: "#64748B"))
                    .fixedSize(horizontal: false, vertical: true)
            }
            Spacer()
        }
        .padding(12)
        .background(Color.white)
        .cornerRadius(14)
        .overlay(
            RoundedRectangle(cornerRadius: 14)
                .stroke(Color(hex: "#E2E8F0"), lineWidth: 1)
        )
    }

    private func wizardGuideStep(number: String, title: String, desc: String) -> some View {
        HStack(alignment: .top, spacing: 12) {
            ZStack {
                Circle()
                    .fill(Color(hex: "#2563EB"))
                    .frame(width: 24, height: 24)
                Text(number)
                    .font(.system(size: 12, weight: .bold))
                    .foregroundColor(.white)
            }
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(.system(size: 14, weight: .semibold))
                    .foregroundColor(Color(hex: "#0F172A"))
                Text(desc)
                    .font(.system(size: 12))
                    .foregroundColor(Color(hex: "#64748B"))
            }
            Spacer()
        }
    }

    private func openSystemSettings() {
        if let url = URL(string: UIApplication.openSettingsURLString) {
            UIApplication.shared.open(url)
        }
    }
}
