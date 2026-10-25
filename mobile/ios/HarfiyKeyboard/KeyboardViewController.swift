//
//  KeyboardViewController.swift
//  HarfiyKeyboard
//
//  Custom iOS Keyboard Extension providing real-time Uzbek Latin conversion.
//

import UIKit

public class KeyboardViewController: UIInputViewController {

    private let engine = HarfiyEngineBridge(enableNg: false)
    private var isShiftEnabled = false
    private let haptic = UIImpactFeedbackGenerator(style: .light)

    private var rowsStackView: UIStackView!
    private var shiftButton: UIButton?

    public override func viewDidLoad() {
        super.viewDidLoad()
        haptic.prepare()
        setupKeyboardUI()
    }

    public override func textWillChange(_ textInput: UITextInput?) {
        super.textWillChange(textInput)
    }

    public override func textDidChange(_ textInput: UITextInput?) {
        super.textDidChange(textInput)
        // User may have tapped to move cursor or selected new text; reset conversion buffer
        engine.reset()
    }

    // MARK: - Keyboard UI Setup

    private func setupKeyboardUI() {
        view.backgroundColor = UIColor(named: "KeyboardBackground") ?? UIColor(white: 0.82, alpha: 1.0)

        rowsStackView = UIStackView()
        rowsStackView.axis = .vertical
        rowsStackView.distribution = .fillEqually
        rowsStackView.spacing = 8
        rowsStackView.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(rowsStackView)

        NSLayoutConstraint.activate([
            rowsStackView.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 4),
            rowsStackView.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -4),
            rowsStackView.topAnchor.constraint(equalTo: view.topAnchor, constant: 8),
            rowsStackView.bottomAnchor.constraint(equalTo: view.bottomAnchor, constant: -8)
        ])

        buildKeyboardRows()
    }

    private func buildKeyboardRows() {
        rowsStackView.arrangedSubviews.forEach { $0.removeFromSuperview() }

        let row1 = ["q", "w", "e", "r", "t", "y", "u", "i", "o", "p"]
        let row2 = ["a", "s", "d", "f", "g", "h", "j", "k", "l"]
        let row3 = ["z", "x", "c", "v", "b", "n", "m"]

        // Row 1
        rowsStackView.addArrangedSubview(createRow(letters: row1))

        // Row 2 (with side padding)
        let row2Stack = createRow(letters: row2)
        row2Stack.layoutMargins = UIEdgeInsets(top: 0, left: 16, bottom: 0, right: 16)
        row2Stack.isLayoutMarginsRelativeArrangement = true
        rowsStackView.addArrangedSubview(row2Stack)

        // Row 3 (Shift + Letters + Backspace)
        let row3Stack = UIStackView()
        row3Stack.axis = .horizontal
        row3Stack.distribution = .fillProportionally
        row3Stack.spacing = 6

        // Shift key
        let shiftBtn = createSpecialKey(icon: isShiftEnabled ? "shift.fill" : "shift") { [weak self] in
            self?.toggleShift()
        }
        self.shiftButton = shiftBtn
        row3Stack.addArrangedSubview(shiftBtn)

        // Row 3 Letters
        let letters3 = createRow(letters: row3)
        row3Stack.addArrangedSubview(letters3)

        // Backspace key
        let backspaceBtn = createSpecialKey(icon: "delete.left") { [weak self] in
            self?.handleBackspace()
        }
        row3Stack.addArrangedSubview(backspaceBtn)

        rowsStackView.addArrangedSubview(row3Stack)

        // Row 4 (123 + Globe / Next Keyboard + Space + Return)
        let row4Stack = UIStackView()
        row4Stack.axis = .horizontal
        row4Stack.spacing = 6
        row4Stack.distribution = .fill

        // 123 / Symbols toggle
        let numBtn = createSpecialKey(title: "123") { [weak self] in
            // Reserved for symbol mode
        }
        numBtn.widthAnchor.constraint(equalToConstant: 44).isActive = true
        row4Stack.addArrangedSubview(numBtn)

        // Globe button (switch keyboards)
        let globeBtn = createSpecialKey(icon: "globe") { }
        globeBtn.addTarget(self, action: #selector(handleInputModeList(from:with:)), for: .allTouchEvents)
        globeBtn.widthAnchor.constraint(equalToConstant: 44).isActive = true
        row4Stack.addArrangedSubview(globeBtn)

        // Spacebar
        let spaceBtn = createKeyButton(title: "bo'sh joy") { [weak self] in
            self?.handleSpace()
        }
        spaceBtn.setContentHuggingPriority(.defaultLow, for: .horizontal)
        row4Stack.addArrangedSubview(spaceBtn)

        // Return key
        let returnBtn = createSpecialKey(title: "kirish") { [weak self] in
            self?.handleReturn()
        }
        returnBtn.widthAnchor.constraint(equalToConstant: 74).isActive = true
        row4Stack.addArrangedSubview(returnBtn)

        rowsStackView.addArrangedSubview(row4Stack)
    }

    private func createRow(letters: [String]) -> UIStackView {
        let stack = UIStackView()
        stack.axis = .horizontal
        stack.distribution = .fillEqually
        stack.spacing = 6

        for letter in letters {
            let displayLetter = isShiftEnabled ? letter.uppercased() : letter.lowercased()
            let btn = createKeyButton(title: displayLetter) { [weak self] in
                self?.handleLetterTap(displayLetter)
            }
            stack.addArrangedSubview(btn)
        }
        return stack
    }

    private func createKeyButton(title: String, action: @escaping () -> Void) -> UIButton {
        let btn = UIButton(type: .custom)
        btn.setTitle(title, for: .normal)
        btn.setTitleColor(UIColor.label, for: .normal)
        btn.titleLabel?.font = UIFont.systemFont(ofSize: 22, weight: .regular)
        btn.backgroundColor = UIColor.white
        btn.layer.cornerRadius = 5.0
        btn.layer.shadowColor = UIColor.black.cgColor
        btn.layer.shadowOffset = CGSize(width: 0, height: 1.0)
        btn.layer.shadowOpacity = 0.25
        btn.layer.shadowRadius = 0.5

        btn.addAction(UIAction { [weak self] _ in
            self?.haptic.impactOccurred()
            UIDevice.current.playInputClick()
            action()
        }, for: .touchUpInside)

        return btn
    }

    private func createSpecialKey(title: String? = nil, icon: String? = nil, action: @escaping () -> Void) -> UIButton {
        let btn = UIButton(type: .custom)
        if let title = title {
            btn.setTitle(title, for: .normal)
            btn.setTitleColor(UIColor.label, for: .normal)
            btn.titleLabel?.font = UIFont.systemFont(ofSize: 16, weight: .regular)
        }
        if let icon = icon {
            let img = UIImage(systemName: icon)
            btn.setImage(img, for: .normal)
            btn.tintColor = UIColor.label
        }

        btn.backgroundColor = UIColor(white: 0.68, alpha: 1.0)
        btn.layer.cornerRadius = 5.0
        btn.layer.shadowColor = UIColor.black.cgColor
        btn.layer.shadowOffset = CGSize(width: 0, height: 1.0)
        btn.layer.shadowOpacity = 0.25
        btn.layer.shadowRadius = 0.5

        btn.addAction(UIAction { [weak self] _ in
            self?.haptic.impactOccurred()
            UIDevice.current.playInputClick()
            action()
        }, for: .touchUpInside)

        return btn
    }

    // MARK: - Actions

    private func handleLetterTap(_ letter: String) {
        guard let char = letter.first else { return }

        let action = engine.processChar(char)
        switch action {
        case .passThrough:
            textDocumentProxy.insertText(letter)
        case .suppress:
            break
        case .transform(let backspacesCount, let replacement):
            for _ in 0..<backspacesCount {
                textDocumentProxy.deleteBackward()
            }
            textDocumentProxy.insertText(replacement)
        }

        if isShiftEnabled {
            toggleShift()
        }
    }

    private func handleBackspace() {
        let action = engine.processBackspace()
        switch action {
        case .passThrough, .suppress:
            textDocumentProxy.deleteBackward()
        case .transform(let backspacesCount, let replacement):
            // Instant Undo: erase the converted char and restore original digraph
            for _ in 0..<backspacesCount {
                textDocumentProxy.deleteBackward()
            }
            textDocumentProxy.insertText(replacement)
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

    private func toggleShift() {
        isShiftEnabled.toggle()
        buildKeyboardRows()
    }
}

// MARK: - UIInputViewAudioFeedback
extension KeyboardViewController: UIInputViewAudioFeedback {
    public var enableInputClicksWhenVisible: Bool {
        return true
    }
}
