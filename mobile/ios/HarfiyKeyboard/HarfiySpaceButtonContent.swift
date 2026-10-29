//
//  HarfiySpaceButtonContent.swift
//  HarfiyKeyboard
//
//  Native Apple-style animated Space bar content with a smooth vertical Push-Up transition:
//  starts with Harfiy tile logo + "Özbek tili", then smoothly pushes up and dissolves into "böşliq".
//

import SwiftUI
import UIKit
import KeyboardKit

/// Shared state manager controlling space bar language name vertical push animation
public class HarfiySpaceState: ObservableObject {
    public static let shared = HarfiySpaceState()

    @Published public var showLocale: Bool = true
    private var workItem: DispatchWorkItem?

    private init() {}

    /// Trigger the native language vertical push animation (e.g. on keyboard appearance)
    public func triggerAnimation(isAlphabetic: Bool) {
        workItem?.cancel()
        guard isAlphabetic else {
            showLocale = false
            return
        }

        showLocale = true
        let item = DispatchWorkItem { [weak self] in
            withAnimation(.spring(response: 0.42, dampingFraction: 0.85, blendDuration: 0)) {
                self?.showLocale = false
            }
        }
        workItem = item
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.8, execute: item)
    }

    /// Immediately push to "böşliq" when user taps any key or switches away
    public func hideLocaleImmediately() {
        workItem?.cancel()
        if showLocale {
            withAnimation(.spring(response: 0.25, dampingFraction: 0.85, blendDuration: 0)) {
                showLocale = false
            }
        }
    }
}

/// Native iOS-style Space bar content with vertical Push-Up animation:
/// [Harfiy Tile Logo] + "Özbek tili" slides up and fades out while "böşliq" slides up from below.
public struct HarfiySpaceButtonContent: View {
    public let localeText: String
    public let spaceText: String
    @ObservedObject private var spaceState = HarfiySpaceState.shared

    // Base64-encoded monochrome Harfiy "H•" tile logo glyph (tight bounds 49x69)
    private static let tileLogoBase64 = "iVBORw0KGgoAAAANSUhEUgAAADEAAABFCAYAAADjLw7LAAAAAXNSR0IArs4c6QAAAERlWElmTU0AKgAAAAgAAYdpAAQAAAABAAAAGgAAAAAAA6ABAAMAAAABAAEAAKACAAQAAAABAAAAMaADAAQAAAABAAAARQAAAABto8jwAAACDElEQVRoBe1Z623CYAxMqv6HDZoNygZ0A9iAbsAKHYEN0g2aTsAIZQPSCQoTpGcJRIgojv09ZFW2ZCUBP+7O+ciDojBqXdfN4Xv40Lb44Mko7AssgKzh9+wHX64vGcb2AG57D/3gO3tEAHA+AMkd7hEwKS0NAoC+gGcmxLQRxqcLB4EVXGUP6WCJK1fijFOCJRJaDoUlEq2WhZmFjcUwAQkiMhWSaYTxacNBZCFc2XTRs3f1BqitgMgqrawB1UGiHkFkEdAiTypI0Kn1MSBDp08Nf+6jMLOw+6D6+wA8x/EL/ABvyrL8xvb/WQmma9BaCqmRKq9Q5SjM+zMcOOgndgOv4O1piw1rTYFkrdGYoxlArLVALF2xp1pFLJHQcjB17+Qk1ApYSfQ14ZOIqICfThHFDCrlkwiSL2KyTyKimEGlfBJB8kVM9klEFJNKtdp6j9rEMXl43KRXK9wTW4UY8iVcZSEkpgB5fp0yQ/cz2Ar75NmM3nZ02bolauQLO5Gw4rI+CbFkiRJ8EomEFZf1SYglS5RgZRIt+JGrLOS2Y4eOzakr7R9uINiN/Q8DNw4r5L/fqMF+FELiDQA/2Q7jA6rxodeRIafTLeWvq2c6CiGRCSLfxknwGuWJ8Enk0Znv4pPgNcoT4ZPIozPfxSfBa5QnwieRR2e+i0+C1yhPhE8ij858F0uToOd0jbW/VSY8PgpeYmIAAAAASUVORK5CYII="

    public static let tileLogoImage: UIImage? = {
        guard let data = Data(base64Encoded: tileLogoBase64) else {
            #if DEBUG
            print("[Harfiy] Failed to decode tile logo base64")
            #endif
            return nil
        }
        return UIImage(data: data)?.withRenderingMode(.alwaysTemplate)
    }()

    public init(
        localeText: String = "Özbek tili",
        spaceText: String = "böşliq"
    ) {
        self.localeText = localeText
        self.spaceText = spaceText
    }

    public var body: some View {
        ZStack {
            if spaceState.showLocale {
                HStack(alignment: .center, spacing: 6.5) {
                    if let logo = Self.tileLogoImage {
                        Image(uiImage: logo)
                            .renderingMode(.template)
                            .resizable()
                            .scaledToFit()
                            .frame(height: 16.5)
                            .offset(y: -2)
                    }

                    Keyboard.ButtonTitle(
                        text: localeText,
                        action: .space
                    )
                }
                .contentShape(Rectangle())
                .transition(
                    .asymmetric(
                        insertion: .move(edge: .bottom).combined(with: .opacity),
                        removal: .move(edge: .top).combined(with: .opacity)
                    )
                )
                .id("locale-\(localeText)")
            } else {
                Keyboard.ButtonTitle(
                    text: spaceText,
                    action: .space
                )
                .transition(
                    .asymmetric(
                        insertion: .move(edge: .bottom).combined(with: .opacity),
                        removal: .move(edge: .top).combined(with: .opacity)
                    )
                )
                .id("space-\(spaceText)")
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .clipped()
        .contentShape(Rectangle())
    }
}
