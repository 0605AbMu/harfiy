//
//  ContentView.swift
//  HarfiyApp
//
//  Main container app interface for Harfiy on iOS.
//

import SwiftUI

public struct ContentView: View {
    @State private var practiceText: String = ""
    @State private var enableNg: Bool = false

    public init() {}

    public var body: some View {
        NavigationView {
            ScrollView {
                VStack(spacing: 24) {
                    // Logo and Hero Title
                    VStack(spacing: 8) {
                        Image(systemName: "character.book.closed.fill")
                            .resizable()
                            .scaledToFit()
                            .frame(width: 64, height: 64)
                            .foregroundColor(.blue)

                        Text("Harfiy")
                            .font(.system(size: 32, weight: .bold))

                        Text("O'zbek alifbosi avto-konvertatsiya tizimi")
                            .font(.subheadline)
                            .foregroundColor(.secondary)
                            .multilineTextAlignment(.center)
                    }
                    .padding(.top, 16)

                    // Step-by-step Setup Guide
                    VStack(alignment: .leading, spacing: 14) {
                        Text("Klaviaturani yoqish tartibi:")
                            .font(.headline)

                        SetupStepView(number: "1", text: "Telefoningiz Sozlamalari (Settings) ga kiring")
                        SetupStepView(number: "2", text: "Asosiy (General) -> Klaviatura (Keyboard) ga o'ting")
                        SetupStepView(number: "3", text: "Yangi klaviatura qo'shish (Add New Keyboard) -> Harfiy ni tanlang")
                    }
                    .padding()
                    .background(Color(.secondarySystemBackground))
                    .cornerRadius(12)

                    // Practice Typing Playground
                    VStack(alignment: .leading, spacing: 10) {
                        Text("Klaviaturani sinab ko'rish:")
                            .font(.headline)

                        TextEditor(text: $practiceText)
                            .frame(height: 100)
                            .padding(8)
                            .background(Color(.systemBackground))
                            .cornerRadius(8)
                            .overlay(
                                RoundedRectangle(cornerRadius: 8)
                                    .stroke(Color.secondary.opacity(0.3), lineWidth: 1)
                            )

                        Text("Masalan: shahar (şahar), choy (çoy), o'rik (örik), g'isht (ğişt)")
                            .font(.caption)
                            .foregroundColor(.secondary)
                    }
                    .padding()
                    .background(Color(.secondarySystemBackground))
                    .cornerRadius(12)

                    // Privacy Note
                    HStack(spacing: 12) {
                        Image(systemName: "lock.shield.fill")
                            .font(.title2)
                            .foregroundColor(.green)

                        VStack(alignment: .leading, spacing: 2) {
                            Text("100% Oflayn va Maxfiy")
                                .font(.subheadline)
                                .fontWeight(.semibold)

                            Text("Harfiy hech qanday internet ruxsatini (Allow Full Access) talab qilmaydi. Yozganlaringiz faqat telefoningizda qoladi.")
                                .font(.caption)
                                .foregroundColor(.secondary)
                        }
                    }
                    .padding()
                    .background(Color.green.opacity(0.1))
                    .cornerRadius(12)

                    Spacer(minLength: 20)
                }
                .padding(.horizontal)
            }
            .navigationTitle("Harfiy")
            .navigationBarTitleDisplayMode(.inline)
        }
    }
}

struct SetupStepView: View {
    let number: String
    let text: String

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            ZStack {
                Circle()
                    .fill(Color.blue)
                    .frame(width: 24, height: 24)
                Text(number)
                    .font(.caption)
                    .fontWeight(.bold)
                    .foregroundColor(.white)
            }
            Text(text)
                .font(.subheadline)
                .fixedSize(horizontal: false, vertical: true)
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
