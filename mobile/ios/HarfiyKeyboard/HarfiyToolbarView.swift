//
//  HarfiyToolbarView.swift
//  HarfiyKeyboard
//
//  Native Apple-style Top Toolbar positioned directly above the keyboard keys.
//  Displays the monochrome Harfiy tile icon on the left, and native 3-slot
//  autocomplete suggestions next to it.
//

import SwiftUI
import UIKit
import KeyboardKit

/// Native-styled utility bar above Harfiy Keyboard
public struct HarfiyToolbarView<AutocompleteView: View>: View {
    public let autocompleteView: AutocompleteView
    public weak var controller: KeyboardInputViewController?

    public init(
        autocompleteView: AutocompleteView,
        controller: KeyboardInputViewController?
    ) {
        self.autocompleteView = autocompleteView
        self.controller = controller
    }

    public var body: some View {
        HStack(spacing: 0) {
            // 1. Monochrome Harfiy Tile Icon (matching Android Quick Settings tile icon)
            HarfiyTileIconView(size: 20)
                .frame(width: 38, height: 44)
                .contentShape(Rectangle())

            // 2. Native Autocomplete Toolbar (3 suggestion slots with native separators)
            autocompleteView
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .frame(height: 44)
        .background(Color.clear)
    }
}

/// Monochrome Harfiy Tile Icon Component (Black in Light Mode, White in Dark Mode)
public struct HarfiyTileIconView: View {
    public var size: CGFloat = 20

    public init(size: CGFloat = 20) {
        self.size = size
    }

    public var body: some View {
        if let image = HarfiyTileImageAsset.image {
            Image(uiImage: image)
                .renderingMode(.template)
                .resizable()
                .scaledToFit()
                .frame(height: size)
                .foregroundColor(Color.primary.opacity(0.85))
        } else {
            // Elegant text fallback
            Text("H")
                .font(.system(size: size * 0.8, weight: .bold, design: .rounded))
                .foregroundColor(Color.primary.opacity(0.85))
        }
    }
}

/// Cached Harfiy Tile Image Asset (Monochrome silhouette PNG from Android Quick Settings)
public enum HarfiyTileImageAsset {
    public static let image: UIImage? = {
        let base64 = "iVBORw0KGgoAAAANSUhEUgAAACQAAAAzCAYAAAD/06q7AAAA6UlEQVR4nO3Z4Q2DIBAFYDfoCI7SERiBERyBERjBERzBERzBERzhCoGmKYFwSL206XvJJf4w3hcip8ZhEAwR3VzN9J7dlZJ0PDFjbF6KkQalK5PLKIW5MzA+qxTIMEH0t6CJ6TmkQH67HwyQ3E5zzVQFJXNDZ1B7BrOIYxKY9itCYTbJzJ7TKSxhLlNHD83ssQ3ME31O7wBqmUMAAQQQQAABBBALNF8CovD1qeKxL0vhRatUW8P1m0EiAagWgGoBqJZW0JrUQq+ZVCodz/04yHZMansFqOfR8XUgAxBAAAEEEEA/DmL/63gAnXmszd2mxhYAAAAASUVORK5CYII="
        guard let data = Data(base64Encoded: base64) else { return nil }
        return UIImage(data: data)?.withRenderingMode(.alwaysTemplate)
    }()
}
