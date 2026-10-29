//
//  HarfiyEngineBridge.swift
//  Harfiy
//
//  Thin Swift bridge to the Harfiy Rust FSM engine (HarfiyCore.xcframework).
//  Build the framework first with `mobile/scripts/build_ios.sh`.
//

import Foundation
import HarfiyCore

/// Result returned from Harfiy engine after processing a key event.
public enum HarfiyAction {
    case passThrough
    case suppress
    case transform(backspacesCount: Int, replacement: String)
}

/// Swift interface wrapping the Rust HarfiyEngine instance.
///
/// Not thread-safe: each instance must be used from a single thread (the main thread in practice).
public final class HarfiyEngineBridge {
    private let handle: HarfiyEngineHandle
    public private(set) var isEnableNg: Bool

    public init(enableNg: Bool = false) {
        self.isEnableNg = enableNg
        self.handle = harfiy_engine_create(enableNg)
    }

    deinit {
        harfiy_engine_destroy(handle)
    }

    /// Reset engine state (e.g. on cursor tap, text selection, or focus change).
    public func reset() {
        harfiy_engine_reset(handle)
    }

    /// Dynamically enable or disable 'ng' -> 'ñ' conversion.
    public func setEnableNg(_ enable: Bool) {
        // Callers re-apply settings before every keystroke; skip no-op updates so the
        // engine never drops a pending first character (e.g. "s" waiting for "h").
        guard enable != isEnableNg else { return }
        isEnableNg = enable
        harfiy_engine_set_enable_ng(handle, enable)
    }

    /// Process a typed character.
    public func processChar(_ char: Character) -> HarfiyAction {
        guard let scalar = char.unicodeScalars.first else {
            return .passThrough
        }
        return mapResult(harfiy_engine_process_char(handle, scalar.value))
    }

    /// Process a Backspace press (supporting Instant Undo).
    public func processBackspace() -> HarfiyAction {
        mapResult(harfiy_engine_process_backspace(handle))
    }

    /// Notify engine of a word delimiter (space, return, punctuation).
    public func processDelimiter() {
        harfiy_engine_process_delimiter(handle)
    }

    /// Convert entire text in batch mode. Stateless, so no engine instance is needed.
    public static func convertText(
        _ input: String,
        enableNg: Bool,
        smartApostrophe: Bool = true
    ) -> (text: String, count: Int) {
        var count: UInt32 = 0
        guard let cStr = harfiy_convert_text(input, enableNg, smartApostrophe, &count) else {
            return (input, 0)
        }
        defer { harfiy_free_string(cStr) }
        return (String(cString: cStr), Int(count))
    }

    private func mapResult(_ res: HarfiyActionResult) -> HarfiyAction {
        switch res.action_type {
        case HARFIY_ACTION_PASS_THROUGH:
            return .passThrough
        case HARFIY_ACTION_SUPPRESS:
            return .suppress
        case HARFIY_ACTION_TRANSFORM:
            return .transform(backspacesCount: Int(res.backspaces_count), replacement: Self.replacement(of: res))
        default:
            return .passThrough
        }
    }

    /// The engine hands back UTF-8 bytes (pointer + length) with process lifetime, not a C string.
    private static func replacement(of res: HarfiyActionResult) -> String {
        guard let bytes = res.replacement_utf8, res.replacement_len > 0 else { return "" }
        return String(decoding: UnsafeBufferPointer(start: bytes, count: res.replacement_len), as: UTF8.self)
    }
}
