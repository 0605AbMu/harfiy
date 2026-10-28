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
///
/// The engine owns the typing state (e.g. "s" waiting for "h"). Only character and
/// backspace taps are routed through it; every other action that edits the document
/// or moves the cursor resets it, so a stale pending character can never make the
/// engine delete text the user did not just type.
open class HarfiyActionHandler: KeyboardAction.StandardHandler {

    private let settings = HarfiySettings.shared
    private let engine: HarfiyEngineBridge

    /// Text this handler inserted last; used to tell our own edits apart from external ones.
    private var lastInsertedText: String?

    public init(
        controller: KeyboardInputViewController
    ) {
        let state = controller.state
        let services = controller.services
        engine = HarfiyEngineBridge(enableNg: HarfiySettings.shared.enableNg)
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
    }

    /// Reset engine buffer state (e.g. on focus change or external cursor change).
    public func resetEngine() {
        lastInsertedText = nil
        engine.reset()
    }

    /// Reset the engine when the user moves the cursor or changes the selection.
    public func selectionDidChange(contextBeforeInput: String?) {
        if let last = lastInsertedText, let context = contextBeforeInput, context.hasSuffix(last) { return }
        resetEngine()
    }

    /// Trigger Instant Undo for recent automatic conversion (toolbar undo button).
    public func handleUndo() {
        deleteBackward(using: keyboardController)
    }

    open override func handle(_ suggestion: Autocomplete.Suggestion) {
        resetEngine()
        super.handle(suggestion)
    }

    open override func handle(
        _ gesture: Keyboard.Gesture,
        on action: KeyboardAction
    ) {
        if action != .none {
            HarfiySpaceState.shared.hideLocaleImmediately()
        }
        if action == .nextKeyboard && gesture == .release {
            resetEngine()
            (keyboardController as? UIInputViewController)?.advanceToNextInputMode()
            return
        }
        if action == .keyboardType(.emojis) {
            resetEngine()
        }
        if bypassesEngine(gesture, on: action) {
            resetEngine()
        }
        super.handle(gesture, on: action)
    }

    open override func action(
        for gesture: Keyboard.Gesture,
        on action: KeyboardAction
    ) -> KeyboardAction.GestureAction? {
        switch (gesture, action) {
        case (.release, .nextKeyboard):
            return { controller in
                (controller as? UIInputViewController)?.advanceToNextInputMode()
            }

        case (.release, .character(let text)):
            return { [weak self] controller in
                guard let self else {
                    controller?.insertText(text)
                    return
                }
                self.insertCharacter(text, using: controller)
            }

        case (.press, .backspace):
            return { [weak self] controller in
                guard let self else {
                    controller?.deleteBackward()
                    return
                }
                self.deleteBackward(using: controller)
            }

        default:
            return super.action(for: gesture, on: action)
        }
    }

    // MARK: - Engine routing

    private func insertCharacter(_ text: String, using controller: KeyboardController?) {
        guard let controller else { return }

        // Multi-character inputs (e.g. ".com" or emoji sequences) are not engine keystrokes.
        guard text.count == 1, let char = text.first else {
            resetEngine()
            controller.insertText(text)
            return
        }

        engine.setEnableNg(settings.enableNg)
        switch engine.processChar(char) {
        case .passThrough:
            controller.insertText(text)
            lastInsertedText = text
        case .suppress:
            lastInsertedText = nil
        case .transform(let backspacesCount, let replacement):
            if backspacesCount > 0 {
                controller.deleteBackward(times: backspacesCount)
            }
            controller.insertText(replacement)
            lastInsertedText = replacement
            settings.recordConversion(character: replacement)
        }
    }

    private func deleteBackward(using controller: KeyboardController?) {
        guard let controller else { return }

        guard settings.instantUndo else {
            resetEngine()
            controller.deleteBackward()
            return
        }

        switch engine.processBackspace() {
        case .passThrough, .suppress:
            controller.deleteBackward()
            lastInsertedText = nil
        case .transform(let backspacesCount, let replacement):
            if backspacesCount > 0 {
                controller.deleteBackward(times: backspacesCount)
            }
            controller.insertText(replacement)
            lastInsertedText = replacement
        }
    }

    /// Whether a gesture edits the document or moves the cursor without going through
    /// the engine (space, return, emoji, callout-free inserts, held backspace, etc.).
    private func bypassesEngine(_ gesture: Keyboard.Gesture, on action: KeyboardAction) -> Bool {
        switch action {
        case .character, .shift, .capsLock, .keyboardType, .none:
            return false
        case .backspace:
            return gesture == .repeatPress
        default:
            return gesture == .release || gesture == .repeatPress || gesture == .doubleTap
        }
    }
}
