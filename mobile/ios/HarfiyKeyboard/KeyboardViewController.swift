//
//  KeyboardViewController.swift
//  HarfiyKeyboard
//
//  Native iOS Keyboard Extension powered by KeyboardKit configured specifically
//  for Uzbek Latin orthography with instant zero-delay rendering and native settings.
//

import UIKit
import SwiftUI
import KeyboardKit

public class KeyboardViewController: KeyboardInputViewController {

    private var isKeyboardViewSetup = false

    public override func viewDidLoad() {
        super.viewDidLoad()

        // Mark keyboard extension active in shared settings
        UserDefaults(suiteName: HarfiySettings.appGroupIdentifier)?.set(true, forKey: HarfiySettings.keyKeyboardExtensionActive)
        UserDefaults(suiteName: HarfiySettings.appGroupIdentifier)?.set(Date().timeIntervalSince1970, forKey: HarfiySettings.keyKeyboardLastOpened)

        // Ensure extension view does not clip content bounds
        view.clipsToBounds = false

        // Lock keyboard specifically to Uzbek Latin
        state.keyboardContext.setLocale(.uzbek)
        state.keyboardContext.setLocales([.uzbek])

        // Enable classic mode for Next Keyboard button to bind directly to this controller
        Keyboard.NextKeyboardButtonControllerMode.current = .classic
        Keyboard.NextKeyboardButtonProxyMode.current = .classic

        // Apply native settings (haptic, audio)
        applyNativeSettings()

        // Inject custom layout service for Uzbek keyboard
        services.layoutService = HarfiyKeyboardLayoutService()

        // Inject Harfiy action handler into KeyboardKit services
        services.actionHandler = HarfiyActionHandler(controller: self)

        // Inject custom Harfiy style service to prevent callout bubbles from clipping
        services.styleService = HarfiyKeyboardStyleService(keyboardContext: state.keyboardContext)

        // Inject custom Harfiy callout service for Uzbek character long-press popups
        services.calloutService = HarfiyCalloutService(
            keyboardContext: state.keyboardContext,
            feedbackService: services.feedbackService
        )

        // Disable autocomplete for clean, fast native typing experience
        state.autocompleteContext.isAutocompleteEnabled = false
        services.autocompleteService = .disabled

        // Setup the keyboard view immediately in viewDidLoad so it is pre-rendered before appearing
        setupKeyboardIfNeeded()

        // Trigger space animation timer on initial load as well
        HarfiySpaceState.shared.triggerAnimation(isAlphabetic: state.keyboardContext.keyboardType.isAlphabetic)
    }

    public override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        // Mark keyboard extension active in shared settings
        UserDefaults(suiteName: HarfiySettings.appGroupIdentifier)?.set(true, forKey: HarfiySettings.keyKeyboardExtensionActive)
        UserDefaults(suiteName: HarfiySettings.appGroupIdentifier)?.set(Date().timeIntervalSince1970, forKey: HarfiySettings.keyKeyboardLastOpened)

        // Refresh preferences from shared app group (e.g. if changed in HarfiyApp)
        applyNativeSettings()

        // A new text field (or the same one after switching keyboards) starts clean.
        (services.actionHandler as? HarfiyActionHandler)?.resetEngine()

        // Ensure keyboard view is setup
        setupKeyboardIfNeeded()

        // Reset emoji keyboard to default alphabetic when keyboard appears/re-opens, matching native iOS
        let context = state.keyboardContext
        if context.keyboardType == .emojis {
            setKeyboardType(.alphabetic(.auto))
        } else if context.keyboardType.isAlphabetic {
            setKeyboardType(context.preferredKeyboardType)
        }

        // Trigger native space bar language animation ("Özbek tili" -> "böşliq")
        HarfiySpaceState.shared.triggerAnimation(isAlphabetic: context.keyboardType.isAlphabetic)
    }

    public override func viewWillSetupKeyboardView() {
        // Guarded: avoid tearing down and recreating UIHostingController on every appearance
        setupKeyboardIfNeeded()
    }

    private func setupKeyboardIfNeeded() {
        guard !isKeyboardViewSetup else { return }
        isKeyboardViewSetup = true

        setupKeyboardView { controller in
            KeyboardView(
                state: controller.state,
                services: controller.services,
                buttonContent: { params in
                    if params.item.action == .space {
                        HarfiySpaceButtonContent(
                            localeText: "Özbek tili",
                            spaceText: "böşliq"
                        )
                    } else {
                        params.view
                    }
                },
                buttonView: { params in
                    if params.item.action == .nextKeyboard {
                        params.view
                            .overlay(Keyboard.NextKeyboardButton(controller: controller) { Color.clear })
                    } else {
                        params.view
                    }
                },
                emojiKeyboard: { _ in
                    HarfiyEmojiKeyboard(controller: controller)
                },
                toolbar: { _ in
                    EmptyView()
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

            // If user changes focus to a secure/restricted field while in emoji mode, auto-close emoji
            if self.state.keyboardContext.keyboardType == .emojis {
                if self.textDocumentProxy.isSecureTextEntry == true {
                    self.setKeyboardType(.alphabetic(.auto))
                } else if let type = self.textDocumentProxy.keyboardType {
                    switch type {
                    case .emailAddress, .URL, .webSearch, .asciiCapable, .asciiCapableNumberPad, .numberPad, .phonePad, .namePhonePad, .decimalPad:
                        self.setKeyboardType(.alphabetic(.auto))
                    default:
                        break
                    }
                }
            }
        }
    }

    open override var isAutocompleteEnabled: Bool {
        return false
    }

    private func applyNativeSettings() {
        let settings = HarfiySettings.shared
        settings.reloadSettings()
        state.feedbackContext.isHapticFeedbackEnabled = settings.hapticFeedback
        state.feedbackContext.isAudioFeedbackEnabled = settings.audioFeedback
    }
}
