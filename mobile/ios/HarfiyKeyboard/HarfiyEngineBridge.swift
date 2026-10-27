//
//  HarfiyEngineBridge.swift
//  Harfiy
//
//  High-performance Swift bridge to the Harfiy Rust FSM engine.
//

import Foundation
#if canImport(HarfiyCore)
import HarfiyCore
#endif

/// Result returned from Harfiy engine after processing a key event.
public enum HarfiyAction {
    case passThrough
    case suppress
    case transform(backspacesCount: Int, replacement: String)
}

/// Thread-safe Swift interface wrapping the Rust HarfiyEngine instance.
public final class HarfiyEngineBridge {
    private var handle: HarfiyEngineHandle?
    public private(set) var isEnableNg: Bool = false

    // State machine for pure Swift fallback (when HarfiyCore is not linked)
    private var fallbackPrevChar: Character?
    private var fallbackJustTransformedUndo: String?
    private var fallbackHistory: [Character] = []

    public init(enableNg: Bool = false) {
        self.isEnableNg = enableNg
        #if canImport(HarfiyCore)
        self.handle = harfiy_engine_create(enableNg)
        #endif
    }

    deinit {
        #if canImport(HarfiyCore)
        if let h = handle {
            harfiy_engine_destroy(h)
        }
        #endif
    }

    /// Reset engine state (e.g. on cursor tap, text selection, or focus change).
    public func reset() {
        fallbackReset()
        #if canImport(HarfiyCore)
        if let h = handle {
            harfiy_engine_reset(h)
        }
        #endif
    }

    private func fallbackReset() {
        fallbackPrevChar = nil
        fallbackJustTransformedUndo = nil
        fallbackHistory.removeAll(keepingCapacity: true)
    }

    /// Dynamically enable or disable 'ng' -> 'ñ' conversion.
    public func setEnableNg(_ enable: Bool) {
        // Callers re-apply settings before every keystroke; skip no-op updates so the
        // engine never drops a pending first character (e.g. "s" waiting for "h").
        guard enable != isEnableNg else { return }
        self.isEnableNg = enable
        #if canImport(HarfiyCore)
        if let h = handle {
            harfiy_engine_set_enable_ng(h, enable)
        }
        #endif
    }

    /// Process a typed character.
    public func processChar(_ char: Character) -> HarfiyAction {
        guard let scalar = char.unicodeScalars.first else {
            return .passThrough
        }

        #if canImport(HarfiyCore)
        guard let h = handle else { return fallbackProcess(char) }
        let res = harfiy_engine_process_char(h, scalar.value)
        return mapResult(res)
        #else
        return fallbackProcess(char)
        #endif
    }

    /// Process a Backspace press (supporting Instant Undo).
    public func processBackspace() -> HarfiyAction {
        #if canImport(HarfiyCore)
        guard let h = handle else { return fallbackProcessBackspace() }
        let res = harfiy_engine_process_backspace(h)
        return mapResult(res)
        #else
        return fallbackProcessBackspace()
        #endif
    }

    /// Notify engine of a word delimiter (space, return, punctuation).
    public func processDelimiter() {
        fallbackReset()
        #if canImport(HarfiyCore)
        if let h = handle {
            harfiy_engine_process_delimiter(h)
        }
        #endif
    }

    /// Convert entire text in batch mode.
    public func convertText(_ input: String, smartApostrophe: Bool = true) -> (text: String, count: Int) {
        #if canImport(HarfiyCore)
        var count: UInt32 = 0
        guard let cStr = harfiy_convert_text(input, isEnableNg, smartApostrophe, &count) else {
            return fallbackConvertText(input, enableNg: isEnableNg, smartApostrophe: smartApostrophe)
        }
        let result = String(cString: cStr)
        harfiy_free_string(cStr)
        return (result, Int(count))
        #else
        return fallbackConvertText(input, enableNg: isEnableNg, smartApostrophe: smartApostrophe)
        #endif
    }

    #if canImport(HarfiyCore)
    private func mapResult(_ res: HarfiyActionResult) -> HarfiyAction {
        switch res.action_type {
        case HARFIY_ACTION_PASS_THROUGH:
            return .passThrough
        case HARFIY_ACTION_SUPPRESS:
            return .suppress
        case HARFIY_ACTION_TRANSFORM:
            let str: String
            if let ptr = res.replacement_utf8 {
                str = String(cString: ptr)
            } else {
                str = ""
            }
            return .transform(backspacesCount: Int(res.backspaces_count), replacement: str)
        default:
            return .passThrough
        }
    }
    #endif

    private static let apostrophes: Set<Character> = [
        "'", "`", "‘", "’", "ʻ", "ʼ", "´", "‛", "ʽ", "ʹ", "′", "＇", "ˊ", "ˋ"
    ]

    private static func isApostrophe(_ c: Character) -> Bool {
        return apostrophes.contains(c)
    }

    private static func isWordDelimiter(_ c: Character) -> Bool {
        if isApostrophe(c) { return false }
        return c.isWhitespace || c.isPunctuation || "—–…«»“”„•№".contains(c)
    }

    private static func isFirstTrigger(_ c: Character, enableNg: Bool) -> Bool {
        switch c {
        case "s", "S", "c", "C", "o", "O", "g", "G":
            return true
        case "n", "N" where enableNg:
            return true
        default:
            return false
        }
    }

    private static func matchRule(prev: Character, curr: Character, enableNg: Bool) -> (replacement: String, originalPair: String)? {
        switch (prev, curr) {
        // sh -> ş
        case ("s", "h"), ("s", "H"): return ("ş", "sh")
        case ("S", "h"), ("S", "H"): return ("Ş", "Sh")

        // ch -> ç
        case ("c", "h"), ("c", "H"): return ("ç", "ch")
        case ("C", "h"), ("C", "H"): return ("Ç", "Ch")

        // o' -> ö
        case ("o", let c) where isApostrophe(c): return ("ö", "o\(c)")
        case ("O", let c) where isApostrophe(c): return ("Ö", "O\(c)")

        // g' -> ğ
        case ("g", let c) where isApostrophe(c): return ("ğ", "g\(c)")
        case ("G", let c) where isApostrophe(c): return ("Ğ", "G\(c)")

        // ng -> ñ
        case ("n", "g"), ("n", "G") where enableNg: return ("ñ", "ng")
        case ("N", "g"), ("N", "G") where enableNg: return ("Ñ", "Ng")

        default: return nil
        }
    }

    /// Pure Swift fallback in case binary framework is not yet linked or in preview mode.
    private func fallbackProcess(_ char: Character) -> HarfiyAction {
        // Reset undo state if user typed any character
        fallbackJustTransformedUndo = nil

        if Self.isWordDelimiter(char) {
            fallbackReset()
            return .passThrough
        }

        if let prev = fallbackPrevChar {
            if let matched = Self.matchRule(prev: prev, curr: char, enableNg: isEnableNg) {
                fallbackPrevChar = nil
                fallbackJustTransformedUndo = matched.originalPair
                if !fallbackHistory.isEmpty {
                    fallbackHistory.removeLast()
                }
                for ch in matched.replacement {
                    fallbackPushHistory(ch)
                }
                return .transform(backspacesCount: 1, replacement: matched.replacement)
            } else {
                fallbackPushHistory(char)
                if Self.isFirstTrigger(char, enableNg: isEnableNg) {
                    fallbackPrevChar = char
                } else {
                    fallbackPrevChar = nil
                }
                return .passThrough
            }
        } else {
            fallbackPushHistory(char)
            if Self.isFirstTrigger(char, enableNg: isEnableNg) {
                fallbackPrevChar = char
            }
            return .passThrough
        }
    }

    /// Pure Swift fallback for Backspace supporting Instant Undo and smart history recovery.
    private func fallbackProcessBackspace() -> HarfiyAction {
        if let undo = fallbackJustTransformedUndo {
            fallbackJustTransformedUndo = nil
            fallbackPrevChar = nil
            if !fallbackHistory.isEmpty {
                fallbackHistory.removeLast()
            }
            for ch in undo {
                fallbackPushHistory(ch)
            }
            return .transform(backspacesCount: 1, replacement: undo)
        }

        if !fallbackHistory.isEmpty {
            fallbackHistory.removeLast()
        }

        if let last = fallbackHistory.last, Self.isFirstTrigger(last, enableNg: isEnableNg) {
            fallbackPrevChar = last
        } else {
            fallbackPrevChar = nil
        }

        return .passThrough
    }

    private func fallbackPushHistory(_ c: Character) {
        if fallbackHistory.count >= 32 {
            fallbackHistory.removeFirst()
        }
        fallbackHistory.append(c)
    }

    /// Pure Swift batch conversion fallback matching Rust convert_text.
    private func fallbackConvertText(_ input: String, enableNg: Bool, smartApostrophe: Bool) -> (String, Int) {
        var result = ""
        var count = 0
        let chars = Array(input)
        let len = chars.count
        var i = 0

        while i < len {
            let curr = chars[i]
            let next: Character? = (i + 1 < len) ? chars[i + 1] : nil

            var matched = false
            if let n = next {
                if let rule = Self.matchRule(prev: curr, curr: n, enableNg: enableNg) {
                    result.append(rule.replacement)
                    count += 1
                    i += 2
                    matched = true
                }
            }

            if !matched {
                // Smart apostrophe (hamza / tutuq belgisi between letters, e.g. ma'no -> maʻno)
                if smartApostrophe && (curr == "'" || curr == "`") {
                    let prevIsLetter = (i > 0) && chars[i - 1].isLetter
                    let nextIsLetter = next?.isLetter ?? false
                    if prevIsLetter && nextIsLetter {
                        result.append("ʻ")
                        count += 1
                        i += 1
                        continue
                    }
                }
                result.append(curr)
                i += 1
            }
        }

        return (result, count)
    }
}
