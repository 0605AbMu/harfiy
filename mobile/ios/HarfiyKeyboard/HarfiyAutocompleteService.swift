//
//  HarfiyAutocompleteService.swift
//  HarfiyKeyboard
//
//  Autocomplete service for Harfiy keyboard.
//  Provides native Apple-style 3-slot word suggestions with Harfiy Uzbek Latin orthography conversion.
//

import Foundation
import UIKit
import KeyboardKit

/// Native-styled Autocomplete Service providing real-time suggestions:
/// - Left slot: Typed verbatim text in quotes (“word”) - tapping preserves verbatim
/// - Center slot: Harfiy converted / spellchecked word with native autocorrect highlight
/// - Right slot: Predicted continuation / completion from UITextChecker
public class HarfiyAutocompleteService: AutocompleteService {

    public var locale: Locale = Locale(identifier: "uz")

    private let engine = HarfiyEngineBridge(enableNg: HarfiySettings.shared.enableNg)
    private let textChecker = UITextChecker()

    public init(locale: Locale = Locale(identifier: "uz")) {
        self.locale = locale
    }

    public func autocompleteSuggestions(
        for text: String
    ) async throws -> [Autocomplete.Suggestion] {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return [] }

        // Sync latest user settings
        let settings = HarfiySettings.shared
        engine.setEnableNg(settings.enableNg)

        var result: [Autocomplete.Suggestion] = []

        // 1. Raw typed word (left slot) - shown in quotes like Apple native keyboard
        let rawSuggestion = Autocomplete.Suggestion(
            text: trimmed,
            type: .unknown,
            title: "“\(trimmed)”"
        )
        result.append(rawSuggestion)

        // 2. Converted word via Harfiy engine (center slot)
        let converted = engine.convertText(trimmed, smartApostrophe: true).text
        let centerSuggestion = Autocomplete.Suggestion(
            text: converted,
            type: .autocorrect,
            title: converted
        )
        result.append(centerSuggestion)

        // 3. Check for completions/predictions (right slot)
        let range = NSRange(location: 0, length: trimmed.utf16.count)
        let langCode = locale.languageCode ?? "uz"
        let completions: [String] = await MainActor.run {
            let checker = textChecker
            return checker.completions(forPartialWordRange: range, in: trimmed, language: langCode)
                ?? checker.completions(forPartialWordRange: range, in: trimmed, language: "uz")
                ?? checker.completions(forPartialWordRange: range, in: trimmed, language: "en")
                ?? []
        }

        // Find best completion distinct from trimmed and converted
        if let completion = completions.first(where: {
            $0.caseInsensitiveCompare(trimmed) != .orderedSame &&
            $0.caseInsensitiveCompare(converted) != .orderedSame
        }) {
            let convertedCompletion = engine.convertText(completion, smartApostrophe: true).text
            result.append(
                Autocomplete.Suggestion(
                    text: convertedCompletion,
                    type: .regular,
                    title: convertedCompletion
                )
            )
        } else if converted.caseInsensitiveCompare(trimmed) == .orderedSame, let secondCompletion = completions.first {
            let convertedCompletion = engine.convertText(secondCompletion, smartApostrophe: true).text
            result.append(
                Autocomplete.Suggestion(
                    text: convertedCompletion,
                    type: .regular,
                    title: convertedCompletion
                )
            )
        }

        return result
    }

    public func nextCharacterPredictions(
        forText text: String,
        suggestions: [Autocomplete.Suggestion]
    ) async throws -> [Character: Double] {
        [:]
    }

    public var canIgnoreWords: Bool { false }
    public var canLearnWords: Bool { false }
    public var ignoredWords: [String] { [] }
    public var learnedWords: [String] { [] }

    public func hasIgnoredWord(_ word: String) -> Bool { false }
    public func hasLearnedWord(_ word: String) -> Bool { false }
    public func ignoreWord(_ word: String) {}
    public func learnWord(_ word: String) {}
    public func removeIgnoredWord(_ word: String) {}
    public func unlearnWord(_ word: String) {}
}
