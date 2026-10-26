//
//  HarfiyKeyboardLayoutService.swift
//  HarfiyKeyboard
//
//  Custom keyboard layout service for Harfiy Keyboard.
//  Ensures that the bottom row ALWAYS includes:
//  - Keyboard Switcher (123 / ABC / #+=)
//  - Next Keyboard (Globe 🌐) for switching languages/keyboards
//  - Emoji button (😀) for opening the rich native emoji keyboard
//  - Flexible Space bar
//  - Primary Return / Done / Go action
//

import Foundation
import UIKit
import KeyboardKit

/// Custom Layout Service that guarantees flexible system buttons (Globe 🌐 and Emoji 😀)
/// on the bottom row of Harfiy Keyboard across all device types and keyboard modes.
open class HarfiyKeyboardLayoutService: KeyboardLayout.StandardService {

    open override func keyboardLayout(for context: KeyboardContext) -> KeyboardLayout {
        let layout = super.keyboardLayout(for: context)
        customizeBottomRow(layout: layout, context: context)
        return layout
    }

    private func customizeBottomRow(layout: KeyboardLayout, context: KeyboardContext) {
        // Emoji keyboard layout is handled by HarfiyEmojiKeyboard
        if context.keyboardType == .emojis { return }

        guard var bottomRow = layout.bottomRow, !bottomRow.isEmpty else { return }

        // Remove any existing duplicate switcher/globe/emoji items
        bottomRow.removeAll { $0.action == .nextKeyboard || $0.action == .custom(named: "emoji") }

        // Use bottom row system item as template for height & insets
        let systemItemTemplate = layout.bottomRowSystemItems?.first ?? bottomRow.first
        guard let template = systemItemTemplate else { return }

        let isPad = context.deviceType == .pad
        let isLandscape = context.interfaceOrientation.isLandscape
        let buttonWidth: KeyboardLayout.ItemWidth = isPad ? .input : .percentage(isLandscape ? 0.085 : 0.115)
        let iconSize = KeyboardLayout.ItemSize(width: buttonWidth, height: template.size.height)

        var itemsToInsert = [KeyboardLayout.Item]()

        // If the system already shows the Globe button below the keyboard (on modern iPhones with home indicator),
        // we do NOT show redundant Globe button inside the keyboard row.
        // We only insert Globe inside the keyboard if the system requires it (needsInputModeSwitchKey == true, e.g. iPhone SE).
        if context.needsInputModeSwitchKey {
            let nextKeyboardItem = KeyboardLayout.Item(
                action: .nextKeyboard,
                size: iconSize,
                edgeInsets: template.edgeInsets
            )
            itemsToInsert.append(nextKeyboardItem)
        }

        // Add native Emoji picker button (😀)
        let emojiItem = KeyboardLayout.Item(
            action: .custom(named: "emoji"),
            size: iconSize,
            edgeInsets: template.edgeInsets
        )
        itemsToInsert.append(emojiItem)

        // Find the space bar in the bottom row
        if let spaceIndex = bottomRow.firstIndex(where: { $0.action == .space }) {
            // Insert before the Space bar
            bottomRow.insert(contentsOf: itemsToInsert, at: spaceIndex)
        } else {
            // In cases without a space bar (e.g. numeric pad or special URL inputs)
            if let returnIndex = bottomRow.firstIndex(where: { $0.action.isPrimaryAction }) {
                bottomRow.insert(contentsOf: itemsToInsert, at: returnIndex)
            } else {
                bottomRow.append(contentsOf: itemsToInsert)
            }
        }

        layout.itemRows[layout.bottomRowIndex] = bottomRow
    }
}
