//
//  HarfiyNextKeyboardButton.swift
//  HarfiyKeyboard
//
//  Custom native Next Keyboard (Globe 🌐) button for Harfiy.
//  Connects directly to the primary UIInputViewController:
//  - Long-press / Touch event: triggers native iOS input mode popup menu (handleInputModeList).
//  - Tap: advances to the next keyboard in the user's active keyboard list (advanceToNextInputMode).
//

import SwiftUI
import UIKit

/// A reliable native UIKit switcher overlay that handles both tap and long-press
/// for switching keyboards in iOS custom keyboard extensions.
public struct HarfiyNextKeyboardButton: UIViewRepresentable {
    public weak var controller: UIInputViewController?

    public init(controller: UIInputViewController?) {
        self.controller = controller
    }

    public func makeUIView(context: Context) -> UIButton {
        let button = UIButton(type: .custom)
        button.backgroundColor = .clear

        if let controller = controller {
            // Native system input mode switcher (shows keyboard list popover on long-press)
            button.addTarget(
                controller,
                action: #selector(UIInputViewController.handleInputModeList(from:with:)),
                for: .allTouchEvents
            )
        }

        // Tap fallback: guarantees switching to the next keyboard on quick tap
        button.addTarget(
            context.coordinator,
            action: #selector(Coordinator.handleTap),
            for: .touchUpInside
        )

        return button
    }

    public func updateUIView(_ uiView: UIButton, context: Context) {
        context.coordinator.controller = controller
    }

    public func makeCoordinator() -> Coordinator {
        Coordinator(controller: controller)
    }

    public class Coordinator: NSObject {
        weak var controller: UIInputViewController?

        init(controller: UIInputViewController?) {
            self.controller = controller
        }

        @objc func handleTap() {
            controller?.advanceToNextInputMode()
        }
    }

    @available(iOS 16.0, *)
    public func sizeThatFits(_ proposal: ProposedViewSize, uiView: UIButton, context: Context) -> CGSize? {
        proposal.replacingUnspecifiedDimensions()
    }
}
