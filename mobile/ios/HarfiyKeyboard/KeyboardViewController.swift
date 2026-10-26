//
//  KeyboardViewController.swift
//  HarfiyKeyboard
//
//  Ultra-Modern, Full-Featured iOS Keyboard Extension providing real-time
//  Uzbek Latin conversion, Numbers & Symbols modes, Instant Undo, Haptic Feedback,
//  and Live Statistics logging.
//

import UIKit

public class KeyboardViewController: UIInputViewController {

    private enum KeyboardMode {
        case letters
        case numbers
        case symbols
    }

    private let engine = HarfiyEngineBridge()
    private let settings = HarfiySettings.shared
    private let haptic = UIImpactFeedbackGenerator(style: .light)

    private var currentMode: KeyboardMode = .letters
    private var isShiftEnabled = false
    private var isCapsLock = false

    private var mainStackView: UIStackView!
    private var shiftButton: UIButton?

    public override func viewDidLoad() {
        super.viewDidLoad()
        haptic.prepare()
        setupEngine()
        setupKeyboardContainer()
    }

    public override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        setupEngine()
        buildKeyboardUI()
    }

    public override func textWillChange(_ textInput: UITextInput?) {
        super.textWillChange(textInput)
    }

    public override func textDidChange(_ textInput: UITextInput?) {
        super.textDidChange(textInput)
        // Reset engine buffer on external cursor changes
        engine.reset()
    }

    private func setupEngine() {
        engine.setEnableNg(settings.enableNg)
    }

    // MARK: - Layout Container Setup

    private func setupKeyboardContainer() {
        view.backgroundColor = UIColor { trait in
            trait.userInterfaceStyle == .dark
                ? UIColor(red: 0.16, green: 0.16, blue: 0.17, alpha: 1.0)
                : UIColor(red: 0.82, green: 0.84, blue: 0.86, alpha: 1.0)
        }

        mainStackView = UIStackView()
        mainStackView.axis = .vertical
        mainStackView.distribution = .fillEqually
        mainStackView.spacing = 8
        mainStackView.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(mainStackView)

        // Standard keyboard safe area constraints
        let safeArea = view.safeAreaLayoutGuide
        NSLayoutConstraint.activate([
            mainStackView.leadingAnchor.constraint(equalTo: safeArea.leadingAnchor, constant: 4),
            mainStackView.trailingAnchor.constraint(equalTo: safeArea.trailingAnchor, constant: -4),
            mainStackView.topAnchor.constraint(equalTo: safeArea.topAnchor, constant: 6),
            mainStackView.bottomAnchor.constraint(equalTo: safeArea.bottomAnchor, constant: -6),
            view.heightAnchor.constraint(greaterThanOrEqualToConstant: 216)
        ])
    }

    // MARK: - UI Builder

    private func buildKeyboardUI() {
        mainStackView.arrangedSubviews.forEach { $0.removeFromSuperview() }

        switch currentMode {
        case .letters:
            buildLettersKeyboard()
        case .numbers:
            buildNumbersKeyboard()
        case .symbols:
            buildSymbolsKeyboard()
        }
    }

    // MARK: - Letters Layout
    private func buildLettersKeyboard() {
        let row1 = ["q", "w", "e", "r", "t", "y", "u", "i", "o", "p"]
        let row2 = ["a", "s", "d", "f", "g", "h", "j", "k", "l"]
        let row3 = ["z", "x", "c", "v", "b", "n", "m"]

        // Row 1
        mainStackView.addArrangedSubview(createKeyRow(items: row1))

        // Row 2 (Indented)
        let row2Stack = createKeyRow(items: row2)
        row2Stack.layoutMargins = UIEdgeInsets(top: 0, left: 16, bottom: 0, right: 16)
        row2Stack.isLayoutMarginsRelativeArrangement = true
        mainStackView.addArrangedSubview(row2Stack)

        // Row 3: Shift + Letters + Backspace
        let row3Stack = UIStackView()
        row3Stack.axis = .horizontal
        row3Stack.distribution = .fill
        row3Stack.spacing = 6

        // Shift Key
        let shiftIcon = isShiftEnabled ? (isCapsLock ? "capslock.fill" : "shift.fill") : "shift"
        let shiftBtn = createSpecialKey(icon: shiftIcon, minWidth: 44) { [weak self] in
            self?.handleShiftTap()
        }
        self.shiftButton = shiftBtn
        row3Stack.addArrangedSubview(shiftBtn)

        // Row 3 Letters
        let letters3 = createKeyRow(items: row3)
        letters3.setContentHuggingPriority(.defaultLow, for: .horizontal)
        row3Stack.addArrangedSubview(letters3)

        // Backspace Key
        let backspaceBtn = createSpecialKey(icon: "delete.left", minWidth: 44) { [weak self] in
            self?.handleBackspace()
        }
        row3Stack.addArrangedSubview(backspaceBtn)

        mainStackView.addArrangedSubview(row3Stack)

        // Row 4: 123 + Globe + Space + Return
        mainStackView.addArrangedSubview(buildBottomRow(modeSwitchTitle: "123") { [weak self] in
            self?.currentMode = .numbers
            self?.buildKeyboardUI()
        })
    }

    // MARK: - Numbers Layout (123)
    private func buildNumbersKeyboard() {
        let row1 = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"]
        let row2 = ["-", "/", ":", ";", "(", ")", "$", "&", "@", "\""]
        let row3 = [".", ",", "?", "!", "'"]

        // Row 1
        mainStackView.addArrangedSubview(createKeyRow(items: row1))

        // Row 2
        mainStackView.addArrangedSubview(createKeyRow(items: row2))

        // Row 3: #+= + Symbols + Backspace
        let row3Stack = UIStackView()
        row3Stack.axis = .horizontal
        row3Stack.distribution = .fill
        row3Stack.spacing = 6

        let altSymBtn = createSpecialKey(title: "#+=", minWidth: 48) { [weak self] in
            self?.currentMode = .symbols
            self?.buildKeyboardUI()
        }
        row3Stack.addArrangedSubview(altSymBtn)

        let symRow = createKeyRow(items: row3)
        symRow.setContentHuggingPriority(.defaultLow, for: .horizontal)
        row3Stack.addArrangedSubview(symRow)

        let backspaceBtn = createSpecialKey(icon: "delete.left", minWidth: 44) { [weak self] in
            self?.handleBackspace()
        }
        row3Stack.addArrangedSubview(backspaceBtn)

        mainStackView.addArrangedSubview(row3Stack)

        // Row 4: ABC + Globe + Space + Return
        mainStackView.addArrangedSubview(buildBottomRow(modeSwitchTitle: "ABC") { [weak self] in
            self?.currentMode = .letters
            self?.buildKeyboardUI()
        })
    }

    // MARK: - Symbols Layout (#+=)
    private func buildSymbolsKeyboard() {
        let row1 = ["[", "]", "{", "}", "#", "%", "^", "*", "+", "="]
        let row2 = ["_", "\\", "|", "~", "<", ">", "€", "£", "¥", "•"]
        let row3 = [".", ",", "?", "!", "'"]

        // Row 1
        mainStackView.addArrangedSubview(createKeyRow(items: row1))

        // Row 2
        mainStackView.addArrangedSubview(createKeyRow(items: row2))

        // Row 3: 123 + Symbols + Backspace
        let row3Stack = UIStackView()
        row3Stack.axis = .horizontal
        row3Stack.distribution = .fill
        row3Stack.spacing = 6

        let numBtn = createSpecialKey(title: "123", minWidth: 48) { [weak self] in
            self?.currentMode = .numbers
            self?.buildKeyboardUI()
        }
        row3Stack.addArrangedSubview(numBtn)

        let symRow = createKeyRow(items: row3)
        symRow.setContentHuggingPriority(.defaultLow, for: .horizontal)
        row3Stack.addArrangedSubview(symRow)

        let backspaceBtn = createSpecialKey(icon: "delete.left", minWidth: 44) { [weak self] in
            self?.handleBackspace()
        }
        row3Stack.addArrangedSubview(backspaceBtn)

        mainStackView.addArrangedSubview(row3Stack)

        // Row 4: ABC + Globe + Space + Return
        mainStackView.addArrangedSubview(buildBottomRow(modeSwitchTitle: "ABC") { [weak self] in
            self?.currentMode = .letters
            self?.buildKeyboardUI()
        })
    }

    // MARK: - Bottom Control Row Builder
    private func buildBottomRow(modeSwitchTitle: String, modeSwitchAction: @escaping () -> Void) -> UIStackView {
        let row4Stack = UIStackView()
        row4Stack.axis = .horizontal
        row4Stack.spacing = 6
        row4Stack.distribution = .fill

        // Mode Switch (123 / ABC)
        let modeBtn = createSpecialKey(title: modeSwitchTitle, minWidth: 48, action: modeSwitchAction)
        row4Stack.addArrangedSubview(modeBtn)

        // Globe / Next Keyboard (if needed by system)
        if self.needsInputModeSwitchKey {
            let globeBtn = createSpecialKey(icon: "globe", minWidth: 44) { }
            globeBtn.addTarget(self, action: #selector(handleInputModeList(from:with:)), for: .allTouchEvents)
            row4Stack.addArrangedSubview(globeBtn)
        }

        // Spacebar
        let spaceBtn = createKeyButton(title: "bo‘sh joy", isSpace: true) { [weak self] in
            self?.handleSpace()
        }
        spaceBtn.setContentHuggingPriority(.defaultLow, for: .horizontal)
        row4Stack.addArrangedSubview(spaceBtn)

        // Return Key
        let returnBtn = createSpecialKey(title: "kirish", minWidth: 74) { [weak self] in
            self?.handleReturn()
        }
        row4Stack.addArrangedSubview(returnBtn)

        return row4Stack
    }

    // MARK: - Key Creation Helpers

    private func createKeyRow(items: [String]) -> UIStackView {
        let stack = UIStackView()
        stack.axis = .horizontal
        stack.distribution = .fillEqually
        stack.spacing = 6

        for item in items {
            let displayText = isShiftEnabled ? item.uppercased() : item.lowercased()
            let btn = createKeyButton(title: displayText) { [weak self] in
                self?.handleKeyTap(displayText)
            }
            stack.addArrangedSubview(btn)
        }
        return stack
    }

    private func createKeyButton(title: String, isSpace: Bool = false, action: @escaping () -> Void) -> UIButton {
        let btn = UIButton(type: .custom)
        btn.setTitle(title, for: .normal)
        btn.setTitleColor(UIColor.label, for: .normal)

        if isSpace {
            btn.titleLabel?.font = UIFont.systemFont(ofSize: 15, weight: .regular)
        } else {
            btn.titleLabel?.font = UIFont.systemFont(ofSize: 22, weight: .regular)
        }

        btn.backgroundColor = UIColor { trait in
            trait.userInterfaceStyle == .dark
                ? UIColor(white: 0.38, alpha: 1.0)
                : UIColor.white
        }

        btn.layer.cornerRadius = 5.0
        btn.layer.shadowColor = UIColor.black.cgColor
        btn.layer.shadowOffset = CGSize(width: 0, height: 1.0)
        btn.layer.shadowOpacity = 0.20
        btn.layer.shadowRadius = 0.5

        btn.addAction(UIAction { [weak self] _ in
            self?.playFeedback()
            action()
        }, for: .touchUpInside)

        return btn
    }

    private func createSpecialKey(title: String? = nil, icon: String? = nil, minWidth: CGFloat = 44, action: @escaping () -> Void) -> UIButton {
        let btn = UIButton(type: .custom)

        if let title = title {
            btn.setTitle(title, for: .normal)
            btn.setTitleColor(UIColor.label, for: .normal)
            btn.titleLabel?.font = UIFont.systemFont(ofSize: 16, weight: .medium)
        }

        if let icon = icon {
            let img = UIImage(systemName: icon)
            btn.setImage(img, for: .normal)
            btn.tintColor = UIColor.label
        }

        btn.backgroundColor = UIColor { trait in
            trait.userInterfaceStyle == .dark
                ? UIColor(white: 0.24, alpha: 1.0)
                : UIColor(red: 0.68, green: 0.71, blue: 0.74, alpha: 1.0)
        }

        btn.layer.cornerRadius = 5.0
        btn.layer.shadowColor = UIColor.black.cgColor
        btn.layer.shadowOffset = CGSize(width: 0, height: 1.0)
        btn.layer.shadowOpacity = 0.20
        btn.layer.shadowRadius = 0.5

        btn.widthAnchor.constraint(equalToConstant: minWidth).isActive = true

        btn.addAction(UIAction { [weak self] _ in
            self?.playFeedback()
            action()
        }, for: .touchUpInside)

        return btn
    }

    private func playFeedback() {
        if settings.hapticFeedback {
            haptic.impactOccurred()
        }
        if settings.audioFeedback {
            UIDevice.current.playInputClick()
        }
    }

    // MARK: - Key Event Handlers

    private func handleKeyTap(_ text: String) {
        guard let char = text.first else { return }

        if currentMode == .letters {
            let action = engine.processChar(char)
            switch action {
            case .passThrough:
                textDocumentProxy.insertText(text)
            case .suppress:
                break
            case .transform(let backspacesCount, let replacement):
                for _ in 0..<backspacesCount {
                    textDocumentProxy.deleteBackward()
                }
                textDocumentProxy.insertText(replacement)
                settings.recordConversion(character: replacement)
            }

            if isShiftEnabled && !isCapsLock {
                isShiftEnabled = false
                buildKeyboardUI()
            }
        } else {
            textDocumentProxy.insertText(text)
        }
    }

    private func handleBackspace() {
        if settings.instantUndo {
            let action = engine.processBackspace()
            switch action {
            case .passThrough, .suppress:
                textDocumentProxy.deleteBackward()
            case .transform(let backspacesCount, let replacement):
                for _ in 0..<backspacesCount {
                    textDocumentProxy.deleteBackward()
                }
                textDocumentProxy.insertText(replacement)
            }
        } else {
            textDocumentProxy.deleteBackward()
        }
    }

    private func handleSpace() {
        engine.processDelimiter()
        textDocumentProxy.insertText(" ")
    }

    private func handleReturn() {
        engine.processDelimiter()
        textDocumentProxy.insertText("\n")
    }

    private func handleShiftTap() {
        if isShiftEnabled {
            if isCapsLock {
                isShiftEnabled = false
                isCapsLock = false
            } else {
                isCapsLock = true
            }
        } else {
            isShiftEnabled = true
        }
        buildKeyboardUI()
    }
}

// MARK: - Audio Feedback Protocol
extension KeyboardViewController: UIInputViewAudioFeedback {
    public var enableInputClicksWhenVisible: Bool {
        return true
    }
}
