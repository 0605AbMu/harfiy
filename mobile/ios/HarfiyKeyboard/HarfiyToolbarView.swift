//
//  HarfiyToolbarView.swift
//  HarfiyKeyboard
//
//  Native-style Top Toolbar positioned directly above the keyboard keys.
//  Displays the compact Harfiy app logo, live engine status pill,
//  smart conversion preview, and quick utility buttons (Undo, Dismiss Keyboard).
//

import SwiftUI
import UIKit
import KeyboardKit

/// Native-styled utility bar above Harfiy Keyboard
public struct HarfiyToolbarView: View {
    public weak var controller: KeyboardInputViewController?
    @ObservedObject private var settings = HarfiySettings.shared

    public init(controller: KeyboardInputViewController?) {
        self.controller = controller
    }

    public var body: some View {
        HStack(spacing: 8) {
            // 1. Compact App Logo & Brand Badge
            HStack(spacing: 5) {
                HarfiyLogoView(size: 20)

                Text("Harfiy")
                    .font(.system(size: 13, weight: .bold, design: .rounded))
                    .foregroundColor(Color.primary.opacity(0.85))

                Circle()
                    .fill(Color.green)
                    .frame(width: 5, height: 5)
            }
            .padding(.horizontal, 7)
            .padding(.vertical, 4)
            .background(Color.primary.opacity(0.04))
            .cornerRadius(8)

            Spacer()

            // 2. Center Smart Engine Status Pill
            HStack(spacing: 4) {
                Image(systemName: "sparkles")
                    .font(.system(size: 10, weight: .bold))
                    .foregroundColor(Color.blue)

                Text("sh➔ş  ch➔ç  oʻ➔ö  gʻ➔ğ")
                    .font(.system(size: 11, weight: .semibold, design: .rounded))
                    .foregroundColor(Color.secondary)
                    .lineLimit(1)
                    .minimumScaleFactor(0.75)
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(Color.primary.opacity(0.04))
            .cornerRadius(8)

            Spacer()

            // 3. Quick Action Buttons
            HStack(spacing: 5) {
                // Instant Undo Button
                Button(action: handleUndo) {
                    Image(systemName: "arrow.uturn.backward")
                        .font(.system(size: 12, weight: .medium))
                        .foregroundColor(Color.primary.opacity(0.75))
                        .frame(width: 30, height: 28)
                        .background(Color.primary.opacity(0.05))
                        .cornerRadius(7)
                }

                // Dismiss Keyboard Button
                Button(action: handleDismiss) {
                    Image(systemName: "keyboard.chevron.compact.down")
                        .font(.system(size: 13, weight: .medium))
                        .foregroundColor(Color.primary.opacity(0.75))
                        .frame(width: 30, height: 28)
                        .background(Color.primary.opacity(0.05))
                        .cornerRadius(7)
                }
            }
        }
        .padding(.horizontal, 8)
        .frame(height: 38)
        .background(Color.clear)
    }

    private func handleUndo() {
        controller?.services.feedbackService.triggerHapticFeedback(.lightImpact)
        (controller?.services.actionHandler as? HarfiyActionHandler)?.handleUndo()
    }

    private func handleDismiss() {
        controller?.services.feedbackService.triggerHapticFeedback(.lightImpact)
        controller?.dismissKeyboard()
    }
}

/// Compact Harfiy Logo Component
public struct HarfiyLogoView: View {
    public var size: CGFloat = 20

    public init(size: CGFloat = 20) {
        self.size = size
    }

    public var body: some View {
        if let image = HarfiyLogoAsset.image {
            Image(uiImage: image)
                .resizable()
                .scaledToFit()
                .frame(width: size, height: size)
                .cornerRadius(size * 0.22)
        } else {
            ZStack {
                RoundedRectangle(cornerRadius: size * 0.22)
                    .fill(
                        LinearGradient(
                            colors: [Color.blue, Color(red: 0.1, green: 0.4, blue: 0.8)],
                            startPoint: .topLeading,
                            endPoint: .bottomTrailing
                        )
                    )
                Text("H")
                    .font(.system(size: size * 0.6, weight: .black, design: .rounded))
                    .foregroundColor(.white)
            }
            .frame(width: size, height: size)
        }
    }
}

/// Cached Harfiy Logo Image Provider
public enum HarfiyLogoAsset {
    public static let image: UIImage? = {
        // Try to load bundle asset if present
        if let bundleImage = UIImage(named: "HarfiyLogo", in: Bundle.main, with: nil) {
            return bundleImage
        }

        // 100% reliable embedded base64 fallback
        let base64 = "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"
        guard let data = Data(base64Encoded: base64) else { return nil }
        return UIImage(data: data)
    }()
}
