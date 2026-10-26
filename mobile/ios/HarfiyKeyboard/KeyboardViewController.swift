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

        // Enable classic mode for Next Keyboard button to bind directly to this controller
        Keyboard.NextKeyboardButtonControllerMode.current = .classic
        Keyboard.NextKeyboardButtonProxyMode.current = .classic

        // Inject custom layout service to guarantee Next Keyboard (Globe 🌐) and Emoji (😀) buttons
        services.layoutService = HarfiyKeyboardLayoutService()

        // Inject Harfiy action handler into KeyboardKit services
        services.actionHandler = HarfiyActionHandler(controller: self)
    }

    public override func viewWillSetupKeyboardView() {
        super.viewWillSetupKeyboardView()

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
                toolbar: { _ in EmptyView() }
            )
        }
    }

    public override func textDidChange(_ textInput: UITextInput?) {
        super.textDidChange(textInput)
        (services.actionHandler as? HarfiyActionHandler)?.resetEngine()
    }
}
