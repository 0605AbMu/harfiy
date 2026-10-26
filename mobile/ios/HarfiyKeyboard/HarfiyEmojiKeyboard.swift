//
//  HarfiyEmojiKeyboard.swift
//  HarfiyKeyboard
//
//  Full-Featured, Native-Performance Emoji Keyboard for Harfiy on iOS.
//  Provides categorized Unicode 15+ emojis, quick category jump tabs,
//  an ABC return button, and backspace key with haptic feedback.
//

import SwiftUI
import KeyboardKit

public struct HarfiyEmojiKeyboard: View {
    public weak var controller: KeyboardInputViewController?
    @ObservedObject var keyboardContext: KeyboardContext

    @State private var selectedCategory: EmojiCategory = .smileysAndPeople

    private let categories: [EmojiCategory] = [
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
                        ForEach(categories, id: \.id) { category in
                            VStack(alignment: .leading, spacing: 6) {
                                Text(categoryTitle(category))
                                    .font(.system(size: 11, weight: .bold))
                                    .foregroundColor(.secondary)
                                    .padding(.horizontal, 14)
                                    .id(category.id)

                                LazyVGrid(columns: columns, spacing: 8) {
                                    ForEach(category.emojis, id: \.char) { emoji in
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
                ForEach(categories, id: \.id) { cat in
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
        .frame(height: 255)
    }

    private func categoryTitle(_ category: EmojiCategory) -> String {
        switch category {
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
        EmojiCategory.addEmoji(emoji, to: .frequent)
        #if os(iOS)
        UIImpactFeedbackGenerator(style: .light).impactOccurred()
        #endif
    }

    private func deleteBackward() {
        controller?.textDocumentProxy.deleteBackward()
        #if os(iOS)
        UIImpactFeedbackGenerator(style: .light).impactOccurred()
        #endif
    }

    private func returnToTextKeyboard() {
        controller?.state.keyboardContext.keyboardType = .alphabetic(.auto)
        controller?.setKeyboardType(.alphabetic(.auto))
    }
}
