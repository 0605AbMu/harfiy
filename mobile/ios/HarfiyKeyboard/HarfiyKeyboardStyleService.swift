//
//  HarfiyKeyboardStyleService.swift
//  HarfiyKeyboard
//
//  Custom KeyboardStyleService for Harfiy Keyboard.
//  Configures native-like callout proportions and sizes to prevent callout bubbles
//  from clipping at the top edge of the iOS keyboard extension window (UIRemoteKeyboardWindow).
//

import Foundation
import SwiftUI
import KeyboardKit

/// Custom keyboard style service providing perfected geometry and aesthetics for Harfiy
public class HarfiyKeyboardStyleService: KeyboardStyle.StandardService {

    public override init(keyboardContext: KeyboardContext) {
        super.init(keyboardContext: keyboardContext)
    }

    /// Override button text: return buttons are entirely icon-based (no text)
    public override func buttonText(for action: KeyboardAction) -> String? {
        switch action {
        case .space:
            return "böşliq"
        case .primary:
            return nil
        default:
            return super.buttonText(for: action)
        }
    }

    /// Override font weight so text and icons have native iOS regular thickness (not thin/light)
    public override func buttonFontWeight(for action: KeyboardAction) -> KeyboardFont.FontWeight? {
        if case .primary = action {
            return .medium
        }
        return .regular
    }

    /// All primary/return buttons use clean, native SF Symbol icons:
    /// - return / newLine: ⏎ (return)
    /// - search: 🔍 (magnifyingglass)
    /// - done / ok: ✔ (checkmark)
    /// - send: ⬆ (arrow.up.circle.fill)
    /// - go / continue: ➜ (arrow.right)
    /// - next: ⇥ (arrow.right.to.line)
    /// - join: 🔗 (link)
    /// - route: 🧭 (arrow.triangle.turn.up.right.diamond.fill)
    /// - emergencyCall: 📞 (phone.fill)
    public override func buttonImage(for action: KeyboardAction) -> Image? {
        switch action {
        case .primary(let type):
            switch type {
            case .return, .newLine:
                return Image(systemName: "return")
            case .search:
                return Image(systemName: "magnifyingglass")
            case .done, .ok:
                return Image(systemName: "checkmark")
            case .send:
                return Image(systemName: "arrow.up.circle.fill")
            case .go, .continue:
                return Image(systemName: "arrow.right")
            case .next:
                return Image(systemName: "arrow.right.to.line")
            case .join:
                return Image(systemName: "link")
            case .route:
                return Image(systemName: "arrow.triangle.turn.up.right.diamond.fill")
            case .emergencyCall:
                return Image(systemName: "phone.fill")
            case .custom:
                return Image(systemName: "return")
            }
        case .keyboardType(.emojis):
            return .keyboardEmoji
        default:
            return super.buttonImage(for: action)
        }
    }

    /// Scale factor for button icons
    public override func buttonImageScaleFactor(for action: KeyboardAction) -> CGFloat {
        switch action {
        case .primary(let type):
            switch type {
            case .return, .newLine:
                return 1.1
            case .search:
                return 1.05
            case .done, .ok:
                return 1.1
            case .send:
                return 1.15
            default:
                return 1.05
            }
        case .keyboardType(.emojis):
            return 1.15
        default:
            return super.buttonImageScaleFactor(for: action)
        }
    }
}

/// Custom callout service that provides specialized Uzbek Latin diacritics
/// and tutuq belgisi variants on long-press.
open class HarfiyCalloutService: Callouts.StandardService {

    public override init(
        keyboardContext: KeyboardContext,
        baseService: CalloutService = Callouts.BaseService(),
        localizedServices: [LocalizedCalloutService] = [],
        feedbackService: FeedbackService? = nil
    ) {
        super.init(
            keyboardContext: keyboardContext,
            baseService: baseService,
            localizedServices: localizedServices,
            feedbackService: feedbackService
        )
    }

    open override func calloutActions(for action: KeyboardAction) -> [KeyboardAction] {
        switch action {
        case .character(let char):
            if char == "'" || char == "’" || char == "ʼ" || char == "ʻ" {
                return ["'", "ʼ", "ʻ", "’"].map { .character($0) }
            }
            if char == "o" {
                return ["o", "ö", "oʻ", "ó", "ò", "ô"].map { .character($0) }
            }
            if char == "O" {
                return ["O", "Ö", "Oʻ", "Ó", "Ò", "Ô"].map { .character($0) }
            }
            if char == "g" {
                return ["g", "ğ", "gʻ"].map { .character($0) }
            }
            if char == "G" {
                return ["G", "Ğ", "Gʻ"].map { .character($0) }
            }
            if char == "s" {
                return ["s", "ş", "sh", "ß"].map { .character($0) }
            }
            if char == "S" {
                return ["S", "Ş", "Sh"].map { .character($0) }
            }
            if char == "c" {
                return ["c", "ç", "ch"].map { .character($0) }
            }
            if char == "C" {
                return ["C", "Ç", "Ch"].map { .character($0) }
            }
        default:
            break
        }
        return super.calloutActions(for: action)
    }
}
