//
//  HarfiyActionHandler.swift
//  HarfiyKeyboard
//
//  Custom KeyboardKit Action Handler integrating Harfiy FSM conversion engine.
//

import Foundation
import UIKit
import KeyboardKit

/// Custom action handler that intercepts character typing and backspaces
/// to apply real-time Uzbek Latin orthography conversion using the Harfiy Rust engine.
open class HarfiyActionHandler: KeyboardAction.StandardHandler {

    private let engine = HarfiyEngineBridge()
    private let settings = HarfiySettings.shared

    public init(
        controller: KeyboardInputViewController
    ) {
        let state = controller.state
        let services = controller.services
        super.init(
            controller: controller,
            keyboardContext: state.keyboardContext,
            keyboardBehavior: services.keyboardBehavior,
            autocompleteContext: state.autocompleteContext,
            autocompleteService: services.autocompleteService,
            feedbackContext: state.feedbackContext,
            feedbackService: services.feedbackService,
            spaceDragGestureHandler: services.spaceDragGestureHandler
        )
        engine.setEnableNg(settings.enableNg)
    }

    /// Reset engine buffer state (e.g. on external cursor change)
    public func resetEngine() {
        engine.reset()
    }

    /// Trigger Instant Undo for recent automatic conversion
    public func handleUndo() {
        guard let controller = keyboardController else { return }
        if settings.instantUndo {
            let result = engine.processBackspace()
            switch result {
            case .passThrough, .suppress:
                controller.deleteBackward()
            case .transform(let backspacesCount, let replacement):
                for _ in 0..<backspacesCount {
                    controller.deleteBackward()
                }
                controller.insertText(replacement)
            }
        } else {
            controller.deleteBackward()
        }
    }

    open override func handle(
        _ gesture: Keyboard.Gesture,
        on action: KeyboardAction
    ) {
        if action == .custom(named: "emoji") {
            if gesture == .release || gesture == .press {
                keyboardContext.keyboardType = .emojis
                keyboardController?.setKeyboardType(.emojis)
                return
            }
        }
        if action == .nextKeyboard && gesture == .release {
            (keyboardController as? UIInputViewController)?.advanceToNextInputMode()
            return
        }
        super.handle(gesture, on: action)
    }

    open override func action(
        for gesture: Keyboard.Gesture,
        on action: KeyboardAction
    ) -> KeyboardAction.GestureAction? {
        if gesture == .release {
            switch action {
            case .nextKeyboard:
                return { controller in
                    (controller as? UIInputViewController)?.advanceToNextInputMode()
                }

            case .custom(named: "emoji"):
                return { [weak self] controller in
                    self?.keyboardContext.keyboardType = .emojis
                    controller?.setKeyboardType(.emojis)
                }

            case .character(let charStr):
                return { [weak self] controller in
                    guard let self = self, let char = charStr.first else {
                        controller?.insertText(charStr)
                        return
                    }

                    self.engine.setEnableNg(self.settings.enableNg)
                    let result = self.engine.processChar(char)

                    switch result {
                    case .passThrough:
                        controller?.insertText(charStr)
                    case .suppress:
                        break
                    case .transform(let backspacesCount, let replacement):
                        for _ in 0..<backspacesCount {
                            controller?.deleteBackward()
                        }
                        controller?.insertText(replacement)
                        self.settings.recordConversion(character: replacement)
                    }
                }

            case .space:
                return { [weak self] controller in
                    self?.engine.processDelimiter()
                    controller?.insertText(" ")
                }

            case .primary:
                return { [weak self] controller in
                    self?.engine.processDelimiter()
                    controller?.insertText("\n")
                }

            default:
                break
            }
        } else if gesture == .press {
            if action == .backspace {
                return { [weak self] controller in
                    guard let self = self else {
                        controller?.deleteBackward()
                        return
                    }

                    if self.settings.instantUndo {
                        let result = self.engine.processBackspace()
                        switch result {
                        case .passThrough, .suppress:
                            controller?.deleteBackward()
                        case .transform(let backspacesCount, let replacement):
                            for _ in 0..<backspacesCount {
                                controller?.deleteBackward()
                            }
                            controller?.insertText(replacement)
                        }
                    } else {
                        controller?.deleteBackward()
                    }
                }
            }
        }

        return super.action(for: gesture, on: action)
    }
}
