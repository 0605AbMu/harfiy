//
//  KeyboardViewController.swift
//  HarfiyKeyboard
//
//  Ultra-Modern, Full-Featured iOS Keyboard Extension powered by KeyboardKit
//  providing native iOS keyboard layouts, real-time Uzbek Latin conversion,
//  Instant Undo, Haptic Feedback, and Live Statistics logging.
//

import UIKit
import SwiftUI
import KeyboardKit

public class KeyboardViewController: KeyboardInputViewController {

    public override func viewDidLoad() {
        super.viewDidLoad()

        // Ensure extension view does not clip content bounds
        view.clipsToBounds = false

        // Enable classic mode for Next Keyboard button to bind directly to this controller
        Keyboard.NextKeyboardButtonControllerMode.current = .classic
        Keyboard.NextKeyboardButtonProxyMode.current = .classic

        // Inject custom layout service to guarantee Next Keyboard (Globe 🌐) and Emoji (😀) buttons
        services.layoutService = HarfiyKeyboardLayoutService()

        // Inject Harfiy action handler into KeyboardKit services
        services.actionHandler = HarfiyActionHandler(controller: self)

        // Inject custom Harfiy style service to prevent callout bubbles from clipping
        services.styleService = HarfiyKeyboardStyleService(keyboardContext: state.keyboardContext)
    }

    public override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        // A new text field (or the same one after switching keyboards) starts clean.
        (services.actionHandler as? HarfiyActionHandler)?.resetEngine()

        // KeyboardKit only applies auto-capitalization after a text change, so the very
        // first appearance started lowercase. super.viewWillAppear queues an async context
        // sync; this block runs after it, once the proxy's autocapitalization is known.
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            let context = self.state.keyboardContext
            guard context.keyboardType.isAlphabetic else { return }
            self.setKeyboardType(context.preferredKeyboardType)
        }
    }

    public override func viewWillSetupKeyboardView() {
        // Not calling super: it would build KeyboardKit's default view first, only to be replaced.

        // Setup KeyboardKit native-style keyboard view with custom emoji & globe support
        setupKeyboardView { controller in
            KeyboardView(
                state: controller.state,
                services: controller.services,
                buttonContent: { params in
                    if params.item.action == KeyboardAction.custom(named: "emoji") {
                        Image(systemName: "face.smiling")
                            .font(.system(size: 20))
                    } else {
                        params.view
                    }
                },
                buttonView: { params in
                    if params.item.action == .nextKeyboard {
                        params.view
                            .overlay(HarfiyNextKeyboardButton(controller: controller))
                    } else {
                        params.view
                    }
                },
                emojiKeyboard: { _ in
                    HarfiyEmojiKeyboard(controller: controller)
                },
                toolbar: { _ in
                    HarfiyToolbarView(controller: controller)
                }
            )
        }
    }

    public override func selectionDidChange(_ textInput: UITextInput?) {
        super.selectionDidChange(textInput)
        // Read the context on the next run loop, once the proxy has caught up.
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            (self.services.actionHandler as? HarfiyActionHandler)?
                .selectionDidChange(contextBeforeInput: self.textDocumentProxy.documentContextBeforeInput)
        }
    }
}
