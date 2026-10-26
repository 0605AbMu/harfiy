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

        // Inject Harfiy action handler into KeyboardKit services
        services.actionHandler = HarfiyActionHandler(controller: self)
    }

    public override func viewWillSetupKeyboardView() {
        super.viewWillSetupKeyboardView()

        // Setup KeyboardKit native-style keyboard view
        setupKeyboardView { controller in
            KeyboardView(
                state: controller.state,
                services: controller.services,
                buttonContent: { $0.view },
                buttonView: { $0.view },
                emojiKeyboard: { $0.view },
                toolbar: { _ in EmptyView() }
            )
        }
    }

    public override func textDidChange(_ textInput: UITextInput?) {
        super.textDidChange(textInput)
        (services.actionHandler as? HarfiyActionHandler)?.resetEngine()
    }
}
