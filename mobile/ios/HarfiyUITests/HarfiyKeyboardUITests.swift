//
//  HarfiyKeyboardUITests.swift
//  HarfiyUITests
//
//  End-to-end tests for the Harfiy keyboard extension.
//
//  Harfiy's own screens convert text in-app, so they would pass even with a broken
//  keyboard. These tests type into a plain system text field (Contacts), where only
//  the keyboard extension can perform the conversion.
//
//  Prerequisite (simulator): the Harfiy keyboard must be enabled, e.g.
//    xcrun simctl spawn <udid> defaults write -g AppleKeyboards -array \
//      "uz.harfiy.keyboard.HarfiyKeyboard" "en_US@sw=QWERTY;hw=Automatic"
//

import XCTest

final class HarfiyKeyboardUITests: XCTestCase {

    private let contacts = XCUIApplication(bundleIdentifier: "com.apple.MobileAddressBook")

    override func setUpWithError() throws {
        continueAfterFailure = false
        contacts.terminate()
        contacts.launch()
    }

    override func tearDownWithError() throws {
        contacts.terminate()
    }

    func testDigraphsConvertWhileTyping() throws {
        let field = try focusPlainTextField()
        type("shahar choy", into: field)
        XCTAssertEqual(value(of: field), "Şahar Çoy")
    }

    func testApostropheLettersConvertWhileTyping() throws {
        let field = try focusPlainTextField()
        type("o'rik g'isht", into: field)
        XCTAssertEqual(value(of: field), "Örik Ğişt")
    }

    func testBackspaceRightAfterConversionRestoresOriginalPair() throws {
        let field = try focusPlainTextField()
        type("sh", into: field)
        tapKey(named: "Backspace")
        XCTAssertEqual(value(of: field), "Sh")
    }

    func testDeletedFirstLetterDoesNotConvertLater() throws {
        let field = try focusPlainTextField()
        type("as", into: field)
        tapKey(named: "Backspace")
        type("h", into: field)
        XCTAssertEqual(value(of: field), "Ah")
    }

    // MARK: - Helpers

    private func focusPlainTextField() throws -> XCUIElement {
        let add = contacts.buttons["Add"]
        XCTAssertTrue(add.waitForExistence(timeout: 10), "Contacts 'Add' button not found")
        add.tap()

        let field = contacts.textFields["First name"]
        XCTAssertTrue(field.waitForExistence(timeout: 10), "Contacts 'First name' field not found")
        field.tap()
        try selectHarfiyKeyboard()

        // "First name" auto-capitalizes words: wait for the keyboard to settle into
        // its shifted state before typing, instead of racing its first layout pass.
        XCTAssertTrue(contacts.buttons["A"].waitForExistence(timeout: 5), "Keyboard did not auto-capitalize")
        return field
    }

    private func selectHarfiyKeyboard() throws {
        let marker = contacts.staticTexts["Harfiy"]
        // A freshly installed extension can take several seconds to cold-start.
        for attempt in 0..<4 {
            if marker.waitForExistence(timeout: attempt == 0 ? 10 : 6) { return }
            let globe = contacts.buttons["Next keyboard"]
            guard globe.exists else { break }
            globe.tap()
        }
        XCTFail("Harfiy keyboard is not active.\n\(contacts.debugDescription)")
    }

    private func type(_ text: String, into field: XCUIElement) {
        for character in text {
            switch character {
            case " ": tapKey(named: "space")
            case "'", "’": tapSymbol("’") // the numeric layer offers the typographic apostrophe
            default: tapLetter(character)
            }
        }
    }

    /// Letter keys are labelled by their current case ("s" or "S" depending on shift state).
    private func tapLetter(_ letter: Character) {
        let names = [String(letter).lowercased(), String(letter).uppercased()]
        for _ in 0..<2 {
            if let key = names.lazy.map({ self.contacts.buttons[$0] }).first(where: { $0.exists }) {
                key.tap()
                return
            }
            // Still on the numeric layer (e.g. after typing an apostrophe): switch back.
            let alphabetic = contacts.buttons.matching(NSPredicate(format: "label == 'Keyboard Type - auto' OR label BEGINSWITH 'Keyboard Type - alphabetic'")).firstMatch
            if alphabetic.waitForExistence(timeout: 1) { alphabetic.tap() }
        }
        XCTFail("Letter key '\(letter)' not found.\n\(contacts.debugDescription)")
    }

    private func tapSymbol(_ symbol: String) {
        let key = contacts.buttons[symbol]
        if !key.exists { tapKey(named: "Keyboard Type - numeric") }
        XCTAssertTrue(key.waitForExistence(timeout: 2), "Symbol key '\(symbol)' not found.\n\(contacts.debugDescription)")
        key.tap()
    }

    private func tapKey(named name: String) {
        let key = contacts.buttons[name]
        XCTAssertTrue(key.waitForExistence(timeout: 2), "Key '\(name)' not found.\n\(contacts.debugDescription)")
        key.tap()
    }

    private func value(of field: XCUIElement) -> String {
        (field.value as? String) ?? ""
    }
}
