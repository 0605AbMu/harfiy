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
        #if canImport(HarfiyCore)
        if let h = handle {
            harfiy_engine_reset(h)
        }
        #endif
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
        guard let h = handle else { return .passThrough }
        let res = harfiy_engine_process_backspace(h)
        return mapResult(res)
        #else
        return .passThrough
        #endif
    }

    /// Notify engine of a word delimiter (space, return, punctuation).
    public func processDelimiter() {
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

    /// Pure Swift fallback in case binary framework is not yet linked or in preview mode.
    private func fallbackProcess(_ char: Character) -> HarfiyAction {
        return .passThrough
    }

    /// Pure Swift batch conversion fallback matching Android's fallback engine.
    private func fallbackConvertText(_ input: String, enableNg: Bool, smartApostrophe: Bool) -> (String, Int) {
        var res = input
        var count = 0

        func countMatches(_ str: String, target: String) -> Int {
            return (str.components(separatedBy: target).count - 1)
        }

        let pairs: [(String, String)] = [
            ("sh", "ş"), ("Sh", "Ş"), ("SH", "Ş"),
            ("ch", "ç"), ("Ch", "Ç"), ("CH", "Ç")
        ]
        for (pattern, repl) in pairs {
            let occurrences = countMatches(res, target: pattern)
            if occurrences > 0 {
                count += occurrences
                res = res.replacingOccurrences(of: pattern, with: repl)
            }
        }

        let apos: [Character] = ["'", "`", "‘", "’", "ʻ", "ʼ", "´"]
        for a in apos {
            let oRepl = smartApostrophe ? "oʻ" : "ö"
            let ORepl = smartApostrophe ? "Oʻ" : "Ö"
            let gRepl = smartApostrophe ? "gʻ" : "ğ"
            let GRepl = smartApostrophe ? "Gʻ" : "Ğ"

            let patterns: [(String, String)] = [
                ("o\(a)", oRepl), ("O\(a)", ORepl),
                ("g\(a)", gRepl), ("G\(a)", GRepl)
            ]
            for (p, r) in patterns {
                let occurrences = countMatches(res, target: p)
                if occurrences > 0 {
                    count += occurrences
                    res = res.replacingOccurrences(of: p, with: r)
                }
            }
        }

        if enableNg {
            let ngPairs: [(String, String)] = [
                ("ng", "ñ"), ("Ng", "Ñ"), ("NG", "Ñ")
            ]
            for (p, r) in ngPairs {
                let occurrences = countMatches(res, target: p)
                if occurrences > 0 {
                    count += occurrences
                    res = res.replacingOccurrences(of: p, with: r)
                }
            }
        }

        return (res, count)
    }
}
