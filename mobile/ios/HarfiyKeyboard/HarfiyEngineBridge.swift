//
//  HarfiyEngineBridge.swift
//  HarfiyKeyboard
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

    public init(enableNg: Bool = false) {
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
        guard let h = handle else { return .passThrough }
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

    /// Pure Swift fallback in case binary framework is not yet compiled in mock preview mode.
    private func fallbackProcess(_ char: Character) -> HarfiyAction {
        return .passThrough
    }
}
