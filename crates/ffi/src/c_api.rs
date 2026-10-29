//! C-ABI implementation for Harfiy Engine (iOS & general C consumers).

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

use harfiy_engine::{EngineConfig, HarfiyEngine, InputEvent, KeyAction, SpecialKey};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarfiyActionType {
    PassThrough = 0,
    Suppress = 1,
    Transform = 2,
}

/// Result of processing one key event.
///
/// For `Transform`, `replacement_utf8` points to `replacement_len` bytes of UTF-8 that are
/// **not** NUL-terminated. They belong to a `&'static str`, so the pointer stays valid for the
/// lifetime of the process and must not be freed. For every other action the pointer is null
/// and the length is 0.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct HarfiyActionResult {
    pub action_type: HarfiyActionType,
    pub backspaces_count: u32,
    pub replacement_utf8: *const u8,
    pub replacement_len: usize,
}

impl Default for HarfiyActionResult {
    fn default() -> Self {
        Self {
            action_type: HarfiyActionType::PassThrough,
            backspaces_count: 0,
            replacement_utf8: ptr::null(),
            replacement_len: 0,
        }
    }
}

impl HarfiyActionResult {
    /// The replacement text of a `Transform` result (empty for other actions).
    ///
    /// # Safety
    /// `replacement_utf8`/`replacement_len` must come from this library (valid UTF-8, not modified).
    pub unsafe fn replacement(&self) -> &str {
        if self.replacement_utf8.is_null() {
            return "";
        }
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(
            self.replacement_utf8,
            self.replacement_len,
        ))
    }
}

fn map_action(action: KeyAction) -> HarfiyActionResult {
    match action {
        KeyAction::PassThrough => HarfiyActionResult::default(),
        KeyAction::Suppress => HarfiyActionResult {
            action_type: HarfiyActionType::Suppress,
            ..HarfiyActionResult::default()
        },
        KeyAction::Transform {
            backspaces_count,
            replacement,
        } => HarfiyActionResult {
            action_type: HarfiyActionType::Transform,
            backspaces_count: backspaces_count as u32,
            replacement_utf8: replacement.as_ptr(),
            replacement_len: replacement.len(),
        },
    }
}

/// Create a new HarfiyEngine instance.
#[no_mangle]
pub unsafe extern "C" fn harfiy_engine_create(enable_ng: bool) -> *mut HarfiyEngine {
    let engine = HarfiyEngine::new(EngineConfig {
        enable_ng,
        enabled: true,
    });
    Box::into_raw(Box::new(engine))
}

/// Destroy an engine instance previously created by `harfiy_engine_create`.
#[no_mangle]
pub unsafe extern "C" fn harfiy_engine_destroy(handle: *mut HarfiyEngine) {
    if !handle.is_null() {
        drop(Box::from_raw(handle));
    }
}

/// Reset the engine back to Idle state (e.g. on cursor tap, text selection, or focus change).
#[no_mangle]
pub unsafe extern "C" fn harfiy_engine_reset(handle: *mut HarfiyEngine) {
    if let Some(engine) = handle.as_mut() {
        engine.reset();
    }
}

/// Dynamically update 'ng' rule configuration.
#[no_mangle]
pub unsafe extern "C" fn harfiy_engine_set_enable_ng(handle: *mut HarfiyEngine, enable_ng: bool) {
    if let Some(engine) = handle.as_mut() {
        engine.set_enable_ng(enable_ng);
    }
}

/// Query if 'ng' rule is enabled.
#[no_mangle]
pub unsafe extern "C" fn harfiy_engine_is_enable_ng(handle: *mut HarfiyEngine) -> bool {
    if let Some(engine) = handle.as_ref() {
        engine.is_enable_ng()
    } else {
        false
    }
}

/// Process a single Unicode scalar character.
#[no_mangle]
pub unsafe extern "C" fn harfiy_engine_process_char(
    handle: *mut HarfiyEngine,
    unicode_char: u32,
) -> HarfiyActionResult {
    let Some(engine) = handle.as_mut() else {
        return HarfiyActionResult::default();
    };

    let Some(ch) = char::from_u32(unicode_char) else {
        return HarfiyActionResult::default();
    };

    let action = engine.process(InputEvent::Char(ch));
    map_action(action)
}

/// Process Backspace event (supporting Instant Undo).
#[no_mangle]
pub unsafe extern "C" fn harfiy_engine_process_backspace(
    handle: *mut HarfiyEngine,
) -> HarfiyActionResult {
    let Some(engine) = handle.as_mut() else {
        return HarfiyActionResult::default();
    };

    let action = engine.process(InputEvent::Special(SpecialKey::Backspace));
    map_action(action)
}

/// Process a word boundary delimiter (e.g. space, punctuation) which resets word buffers.
#[no_mangle]
pub unsafe extern "C" fn harfiy_engine_process_delimiter(handle: *mut HarfiyEngine) {
    if let Some(engine) = handle.as_mut() {
        let _ = engine.process(InputEvent::Char(' '));
    }
}

/// Convert an entire string to the new Uzbek Latin orthography in batch mode.
#[no_mangle]
pub unsafe extern "C" fn harfiy_convert_text(
    input_utf8: *const c_char,
    enable_ng: bool,
    smart_apostrophe: bool,
    out_replacements_count: *mut u32,
) -> *mut c_char {
    if input_utf8.is_null() {
        return ptr::null_mut();
    }

    let Ok(input_str) = CStr::from_ptr(input_utf8).to_str() else {
        return ptr::null_mut();
    };

    let (converted, count) = harfiy_engine::convert_text(input_str, enable_ng, smart_apostrophe);

    if !out_replacements_count.is_null() {
        *out_replacements_count = count as u32;
    }

    match CString::new(converted) {
        Ok(c_str) => c_str.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// Free a string allocated by `harfiy_convert_text`.
#[no_mangle]
pub unsafe extern "C" fn harfiy_free_string(s: *mut c_char) {
    if !s.is_null() {
        drop(CString::from_raw(s));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_c_api_lifecycle_and_conversion() {
        unsafe {
            let engine = harfiy_engine_create(false);
            assert!(!engine.is_null());

            // Type 's'
            let res_s = harfiy_engine_process_char(engine, 's' as u32);
            assert_eq!(res_s.action_type, HarfiyActionType::PassThrough);

            // Type 'h' -> should transform to "ş"
            let res_h = harfiy_engine_process_char(engine, 'h' as u32);
            assert_eq!(res_h.action_type, HarfiyActionType::Transform);
            assert_eq!(res_h.backspaces_count, 1);
            assert_eq!(res_h.replacement(), "ş");

            // Instant Undo: Backspace -> should undo to "sh"
            let res_undo = harfiy_engine_process_backspace(engine);
            assert_eq!(res_undo.action_type, HarfiyActionType::Transform);
            assert_eq!(res_undo.backspaces_count, 1);
            assert_eq!(res_undo.replacement(), "sh");

            harfiy_engine_destroy(engine);
        }
    }

    /// Every transform the engine can produce (and every undo of it) must reach C intact.
    /// Regression: the old NUL-terminated string table silently returned "" for unlisted
    /// replacements (e.g. the undo of `o` + a rare apostrophe variant).
    #[test]
    fn ffi_replacements_match_engine_for_all_trigger_pairs() {
        fn engine_replacement(action: KeyAction) -> Option<(usize, &'static str)> {
            match action {
                KeyAction::Transform {
                    backspaces_count,
                    replacement,
                } => Some((backspaces_count, replacement)),
                _ => None,
            }
        }

        let firsts = "sScCoOgGnN";
        let seconds = "hHgG'`‘’ʻʼ´‛ʽʹ′＇ˊˋõÕ";
        let mut transforms = 0;

        for enable_ng in [false, true] {
            for a in firsts.chars() {
                for b in seconds.chars() {
                    let mut reference = HarfiyEngine::new(EngineConfig {
                        enable_ng,
                        enabled: true,
                    });
                    reference.process(InputEvent::Char(a));
                    let expected = engine_replacement(reference.process(InputEvent::Char(b)));
                    let expected_undo = engine_replacement(
                        reference.process(InputEvent::Special(SpecialKey::Backspace)),
                    );

                    unsafe {
                        let engine = harfiy_engine_create(enable_ng);
                        harfiy_engine_process_char(engine, a as u32);
                        let res = harfiy_engine_process_char(engine, b as u32);
                        let undo = harfiy_engine_process_backspace(engine);
                        harfiy_engine_destroy(engine);

                        for (res, expected) in [(res, expected), (undo, expected_undo)] {
                            match expected {
                                Some((backspaces, text)) => {
                                    transforms += 1;
                                    assert_eq!(
                                        res.action_type,
                                        HarfiyActionType::Transform,
                                        "{a}{b}"
                                    );
                                    assert_eq!(res.backspaces_count as usize, backspaces, "{a}{b}");
                                    assert!(!text.is_empty(), "{a}{b}");
                                    assert_eq!(res.replacement(), text, "{a}{b}");
                                }
                                None => assert_ne!(
                                    res.action_type,
                                    HarfiyActionType::Transform,
                                    "{a}{b}"
                                ),
                            }
                        }
                    }
                }
            }
        }
        assert!(
            transforms > 100,
            "probe produced too few transforms: {transforms}"
        );
    }
}
