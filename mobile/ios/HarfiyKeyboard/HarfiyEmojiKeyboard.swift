//
//  HarfiyEmojiKeyboard.swift
//  HarfiyKeyboard
//
//  Native-Performance Emoji Keyboard for Harfiy on iOS.
//  Provides categorized Unicode emojis, frequent emojis, quick category jump tabs,
//  an ABC return button, and backspace key using native KeyboardKit feedback.
//

import SwiftUI
import KeyboardKit

/// Maximum number of emojis kept in the persisted "frequent" category.
private let harfiyFrequentEmojiLimit = 32

/// A category together with the emojis it shows.
private struct HarfiyEmojiSection: Identifiable {
    let category: EmojiCategory
    let emojis: [Emoji]

    var id: String { category.id }
}

/// Emoji sections of one emoji-keyboard session, built once.
///
/// Only `.frequent` lives in UserDefaults (every other category is a static list), so reading the
/// categories in the SwiftUI `body` would repeat that decode on every render pass.
private final class HarfiyEmojiSections: ObservableObject {
    let sections: [HarfiyEmojiSection]

    init(categories: [EmojiCategory]) {
        // Trim lists persisted before the limit existed (KeyboardKit defaults to 10 000 entries).
        if EmojiCategory.frequentEmojis.count > harfiyFrequentEmojiLimit {
            EmojiCategory.frequentEmojis = Array(EmojiCategory.frequentEmojis.prefix(harfiyFrequentEmojiLimit))
        }
        sections = categories.compactMap { category in
            let emojis = category.emojis
            return emojis.isEmpty ? nil : HarfiyEmojiSection(category: category, emojis: emojis)
        }
    }
}

public struct HarfiyEmojiKeyboard: View {
    public weak var controller: KeyboardInputViewController?
    @ObservedObject var keyboardContext: KeyboardContext

    @State private var selectedCategory: EmojiCategory = .smileysAndPeople
    @StateObject private var store = HarfiyEmojiSections(categories: HarfiyEmojiKeyboard.categories)

    private static let categories: [EmojiCategory] = [
        .frequent,
        .smileysAndPeople,
        .animalsAndNature,
        .foodAndDrink,
        .activity,
        .travelAndPlaces,
        .objects,
        .symbols,
        .flags
    ]

    private let columns = Array(repeating: GridItem(.flexible(), spacing: 4), count: 8)

    public init(controller: KeyboardInputViewController) {
        self.controller = controller
        self._keyboardContext = ObservedObject(wrappedValue: controller.state.keyboardContext)
    }

    public var body: some View {
        VStack(spacing: 0) {
            // 1. Emojis Scroll View
            ScrollViewReader { proxy in
                ScrollView(.vertical, showsIndicators: false) {
                    LazyVStack(alignment: .leading, spacing: 14) {
                        ForEach(store.sections) { section in
                            VStack(alignment: .leading, spacing: 6) {
                                Text(categoryTitle(section.category))
                                    .font(.system(size: 11, weight: .bold))
                                    .foregroundColor(.secondary)
                                    .padding(.horizontal, 14)
                                    .id(section.id)

                                LazyVGrid(columns: columns, spacing: 8) {
                                    ForEach(section.emojis, id: \.char) { emoji in
                                        Button(action: { selectEmoji(emoji) }) {
                                            Text(emoji.char)
                                                .font(.system(size: 28))
                                                .frame(width: 38, height: 38)
                                        }
                                        .buttonStyle(PlainButtonStyle())
                                    }
                                }
                                .padding(.horizontal, 8)
                            }
                        }
                    }
                    .padding(.top, 8)
                    .padding(.bottom, 6)
                }
                .onChange(of: selectedCategory) { newCategory in
                    withAnimation(.easeInOut(duration: 0.25)) {
                        proxy.scrollTo(newCategory.id, anchor: .top)
                    }
                }
            }

            Divider()

            // 2. Bottom Navigation & Category Switcher Bar
            HStack(spacing: 4) {
                // Return to Harfiy Text Keyboard
                Button(action: returnToTextKeyboard) {
                    Text("ABC")
                        .font(.system(size: 14, weight: .bold))
                        .foregroundColor(.primary)
                        .frame(width: 46, height: 36)
                        .background(Color(UIColor.systemGray4))
                        .cornerRadius(8)
                }
                .buttonStyle(PlainButtonStyle())

                Spacer(minLength: 2)

                // Category Tabs
                ForEach(Self.categories, id: \.id) { cat in
                    Button(action: { selectedCategory = cat }) {
                        Text(cat.emojiIcon)
                            .font(.system(size: 17))
                            .frame(maxWidth: .infinity)
                            .frame(height: 36)
                            .background(selectedCategory == cat ? Color(UIColor.systemGray4).opacity(0.85) : Color.clear)
                            .cornerRadius(8)
                    }
                    .buttonStyle(PlainButtonStyle())
                }

                Spacer(minLength: 2)

                // Backspace button while in emoji mode
                Button(action: deleteBackward) {
                    Image(systemName: "delete.backward.fill")
                        .font(.system(size: 15))
                        .foregroundColor(.primary)
                        .frame(width: 44, height: 36)
                        .background(Color(UIColor.systemGray4))
                        .cornerRadius(8)
                }
                .buttonStyle(PlainButtonStyle())
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 6)
            .background(Color(UIColor.secondarySystemBackground))
        }
        .frame(height: keyboardContext.interfaceOrientation.isLandscape ? 160 : 255)
    }

    private func categoryTitle(_ category: EmojiCategory) -> String {
        switch category {
        case .frequent: return "TEZ-TEZ ISHLATILADIGANLAR"
        case .smileysAndPeople: return "KULGICHLAR VA ODAMLAR"
        case .animalsAndNature: return "HAYVONOT VA TABIAT"
        case .foodAndDrink: return "YEMAK VA ICHIMLIKLAR"
        case .activity: return "SPORT VA FAOLIYAT"
        case .travelAndPlaces: return "SAYOHAT VA MANZILLAR"
        case .objects: return "BUYUMLAR"
        case .symbols: return "BELGILAR"
        case .flags: return "BAYROQLAR"
        default: return category.id.uppercased()
        }
    }

    private func selectEmoji(_ emoji: Emoji) {
        controller?.textDocumentProxy.insertText(emoji.char)
        EmojiCategory.addEmoji(emoji, to: .frequent, maxCount: harfiyFrequentEmojiLimit)
        controller?.services.feedbackService.triggerHapticFeedback(.lightImpact)
    }

    private func deleteBackward() {
        controller?.textDocumentProxy.deleteBackward()
        controller?.services.feedbackService.triggerHapticFeedback(.lightImpact)
    }

    private func returnToTextKeyboard() {
        (controller?.services.actionHandler as? HarfiyActionHandler)?.resetEngine()
        controller?.state.keyboardContext.keyboardType = .alphabetic(.auto)
        controller?.setKeyboardType(.alphabetic(.auto))
    }
}
