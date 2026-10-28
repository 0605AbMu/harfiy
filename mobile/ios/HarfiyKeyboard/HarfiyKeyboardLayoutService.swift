//
//  HarfiyKeyboardLayoutService.swift
//  HarfiyKeyboard
//
//  Native keyboard layout service specifically tailored for Uzbek Latin on iOS.
//  Configures native QWERTY layout matching standard Apple iOS keyboard proportions,
//  geometry, and key heights, with flexible bottom row and contextual emoji picker
//  ([123] [😀] [böşliq] [qaytiş]).
//

import Foundation
import UIKit
import KeyboardKit

open class HarfiyKeyboardLayoutService: KeyboardLayout.StandardService {

    open override func keyboardLayout(for context: KeyboardContext) -> KeyboardLayout {
        let layout = super.keyboardLayout(for: context)
        customizeBottomRow(layout: layout, context: context)
        return layout
    }

    private func customizeBottomRow(layout: KeyboardLayout, context: KeyboardContext) {
        // Emoji keyboard layout is handled separately
        if context.keyboardType == .emojis { return }

        guard var bottomRow = layout.bottomRow, !bottomRow.isEmpty else { return }

        // Find system item template for height & edgeInsets
        guard let template = layout.bottomRowSystemItems?.first ?? bottomRow.first else { return }

        let isPad = context.deviceType == .pad
        if isPad { return }

        let isPortrait = context.interfaceOrientation.isPortrait
        let systemButtonWidth: KeyboardLayout.ItemWidth = .percentage(isPortrait ? 0.25 : 0.195)
        let splitButtonWidth: KeyboardLayout.ItemWidth = .percentage(isPortrait ? 0.123 : 0.095)
        let compactButtonWidth: KeyboardLayout.ItemWidth = .percentage(isPortrait ? 0.09 : 0.07)

        // Find the switcher action (123 on alphabetic, ABC on numeric/symbolic)
        let switchAction = bottomRow.first(where: {
            if case .keyboardType = $0.action { return true }
            return false
        })?.action ?? (context.keyboardType.isAlphabetic ? .keyboardType(.numeric) : .keyboardType(.alphabetic(.auto)))

        // Find primary return action
        let returnItem = bottomRow.first(where: { $0.action.isPrimaryAction })
        let returnAction = returnItem?.action ?? .primary(.return)

        // Keep any special middle input actions (e.g. '@', '.' in email/URL modes)
        let middleInputItems = bottomRow.filter {
            $0.action != switchAction &&
            !$0.action.isPrimaryAction &&
            $0.action != .nextKeyboard &&
            $0.action != .keyboardType(.emojis) &&
            $0.action != .custom(named: "emoji") &&
            $0.action != .space &&
            $0.action != context.keyboardDictationReplacement
        }

        var newBottomRow = [KeyboardLayout.Item]()

        let shouldShowEmoji = shouldShowEmojiButton(context: context, middleInputItems: middleInputItems)

        if context.needsInputModeSwitchKey {
            // Devices with physical home buttons that require a Globe key in-layout
            let btnWidth = shouldShowEmoji ? compactButtonWidth : splitButtonWidth
            let switchItem = KeyboardLayout.Item(
                action: switchAction,
                size: KeyboardLayout.ItemSize(width: btnWidth, height: template.size.height),
                edgeInsets: template.edgeInsets
            )
            let nextKeyboardItem = KeyboardLayout.Item(
                action: .nextKeyboard,
                size: KeyboardLayout.ItemSize(width: btnWidth, height: template.size.height),
                edgeInsets: template.edgeInsets
            )
            newBottomRow.append(switchItem)
            newBottomRow.append(nextKeyboardItem)

            if shouldShowEmoji {
                let emojiItem = KeyboardLayout.Item(
                    action: .keyboardType(.emojis),
                    size: KeyboardLayout.ItemSize(width: btnWidth, height: template.size.height),
                    edgeInsets: template.edgeInsets
                )
                newBottomRow.append(emojiItem)
            }
        } else {
            // Modern iPhones (Face ID / Home indicator devices):
            // In alphabetic inputs, always display emoji picker next to 123 ([123] [😀] [böşliq] [return])
            if shouldShowEmoji {
                let switchItem = KeyboardLayout.Item(
                    action: switchAction,
                    size: KeyboardLayout.ItemSize(width: splitButtonWidth, height: template.size.height),
                    edgeInsets: template.edgeInsets
                )
                let emojiItem = KeyboardLayout.Item(
                    action: .keyboardType(.emojis),
                    size: KeyboardLayout.ItemSize(width: splitButtonWidth, height: template.size.height),
                    edgeInsets: template.edgeInsets
                )
                newBottomRow.append(switchItem)
                newBottomRow.append(emojiItem)
            } else {
                let switchItem = KeyboardLayout.Item(
                    action: switchAction,
                    size: KeyboardLayout.ItemSize(width: systemButtonWidth, height: template.size.height),
                    edgeInsets: template.edgeInsets
                )
                newBottomRow.append(switchItem)
            }
        }

        // Center space bar (stretching across the remaining middle space)
        let spaceItem = KeyboardLayout.Item(
            action: .space,
            size: KeyboardLayout.ItemSize(width: .available, height: template.size.height),
            edgeInsets: template.edgeInsets
        )
        newBottomRow.append(spaceItem)

        // Append any middle input items (for URL / email layouts)
        for item in middleInputItems {
            newBottomRow.append(item)
        }

        // Right return button
        let primaryItem = KeyboardLayout.Item(
            action: returnAction,
            size: KeyboardLayout.ItemSize(width: systemButtonWidth, height: template.size.height),
            edgeInsets: template.edgeInsets
        )
        newBottomRow.append(primaryItem)

        layout.itemRows[layout.bottomRowIndex] = newBottomRow
    }

    /// Determines whether the emoji button should be displayed matching Apple's native keyboard logic.
    /// In native iOS keyboards, emoji is shown for standard text & search inputs, but hidden for
    /// email addresses, web URLs, passwords, and ASCII-restricted fields.
    private func shouldShowEmojiButton(context: KeyboardContext, middleInputItems: [KeyboardLayout.Item]) -> Bool {
        // 1. Emoji is only ever available on the alphabetic keyboard layer
        guard context.keyboardType.isAlphabetic else { return false }

        // 2. Do not show emoji in secure / password fields
        if context.textDocumentProxy.isSecureTextEntry == true { return false }

        #if os(iOS) || os(tvOS) || os(visionOS)
        if let contentType = context.textDocumentProxy.textContentType {
            if contentType == .password || contentType == .newPassword || contentType == .oneTimeCode || contentType == .emailAddress || contentType == .telephoneNumber || contentType == .creditCardNumber {
                return false
            }
        }

        // 3. Check UIKeyboardType of the target input
        if let type = context.textDocumentProxy.keyboardType {
            switch type {
            case .emailAddress, .URL, .webSearch, .asciiCapable, .asciiCapableNumberPad, .numberPad, .phonePad, .namePhonePad, .decimalPad:
                return false
            default:
                break
            }
        }
        #endif

        // 4. Do not show emoji if special middle input items exist (e.g. email '@', '.' or URL '.', '/', '.com')
        if !middleInputItems.isEmpty { return false }

        return true
    }
}
