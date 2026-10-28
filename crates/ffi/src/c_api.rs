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

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct HarfiyActionResult {
    pub action_type: HarfiyActionType,
    pub backspaces_count: u32,
    pub replacement_utf8: *const c_char,
}

impl Default for HarfiyActionResult {
    fn default() -> Self {
        Self {
            action_type: HarfiyActionType::PassThrough,
            backspaces_count: 0,
            replacement_utf8: ptr::null(),
        }
    }
}

fn map_action(action: KeyAction) -> HarfiyActionResult {
    match action {
        KeyAction::PassThrough => HarfiyActionResult {
            action_type: HarfiyActionType::PassThrough,
            backspaces_count: 0,
            replacement_utf8: ptr::null(),
        },
        KeyAction::Suppress => HarfiyActionResult {
            action_type: HarfiyActionType::Suppress,
            backspaces_count: 0,
            replacement_utf8: ptr::null(),
        },
        KeyAction::Transform {
            backspaces_count,
            replacement,
        } => {
            // Note: replacement is &'static str (e.g. "ş\0") so its pointer is statically valid.
            // But to guarantee null-termination in C, we map known static replacements to null-terminated C-strings.
            let c_str = match replacement {
                "ş" => c"ş".as_ptr(),
                "Ş" => c"Ş".as_ptr(),
                "ç" => c"ç".as_ptr(),
                "Ç" => c"Ç".as_ptr(),
                "ö" => c"ö".as_ptr(),
                "Ö" => c"Ö".as_ptr(),
                "ğ" => c"ğ".as_ptr(),
                "Ğ" => c"Ğ".as_ptr(),
                "ñ" => c"ñ".as_ptr(),
                "Ñ" => c"Ñ".as_ptr(),
                "sh" => c"sh".as_ptr(),
                "sH" => c"sH".as_ptr(),
                "Sh" => c"Sh".as_ptr(),
                "SH" => c"SH".as_ptr(),
                "ch" => c"ch".as_ptr(),
                "cH" => c"cH".as_ptr(),
                "Ch" => c"Ch".as_ptr(),
                "CH" => c"CH".as_ptr(),
                "o'" => c"o'".as_ptr(),
                "o`" => c"o`".as_ptr(),
                "o‘" => c"o‘".as_ptr(),
                "o’" => c"o’".as_ptr(),
                "oʻ" => c"oʻ".as_ptr(),
                "oʼ" => c"oʼ".as_ptr(),
                "o´" => c"o´".as_ptr(),
                "O'" => c"O'".as_ptr(),
                "O`" => c"O`".as_ptr(),
                "O‘" => c"O‘".as_ptr(),
                "O’" => c"O’".as_ptr(),
                "Oʻ" => c"Oʻ".as_ptr(),
                "Oʼ" => c"Oʼ".as_ptr(),
                "O´" => c"O´".as_ptr(),
                "g'" => c"g'".as_ptr(),
                "g`" => c"g`".as_ptr(),
                "g‘" => c"g‘".as_ptr(),
                "g’" => c"g’".as_ptr(),
                "gʻ" => c"gʻ".as_ptr(),
                "gʼ" => c"gʼ".as_ptr(),
                "g´" => c"g´".as_ptr(),
                "G'" => c"G'".as_ptr(),
                "G`" => c"G`".as_ptr(),
                "G‘" => c"G‘".as_ptr(),
                "G’" => c"G’".as_ptr(),
                "Gʻ" => c"Gʻ".as_ptr(),
                "Gʼ" => c"Gʼ".as_ptr(),
                "G´" => c"G´".as_ptr(),
                "ng" => c"ng".as_ptr(),
                "nG" => c"nG".as_ptr(),
                "Ng" => c"Ng".as_ptr(),
                "NG" => c"NG".as_ptr(),
                "õ" => c"õ".as_ptr(),
                "Õ" => c"Õ".as_ptr(),
                _ => c"".as_ptr(),
            };

            HarfiyActionResult {
                action_type: HarfiyActionType::Transform,
                backspaces_count: backspaces_count as u32,
                replacement_utf8: c_str,
            }
        }
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
            let repl = CStr::from_ptr(res_h.replacement_utf8).to_str().unwrap();
            assert_eq!(repl, "ş");

            // Instant Undo: Backspace -> should undo to "sh"
            let res_undo = harfiy_engine_process_backspace(engine);
            assert_eq!(res_undo.action_type, HarfiyActionType::Transform);
            assert_eq!(res_undo.backspaces_count, 1);
            let undo_repl = CStr::from_ptr(res_undo.replacement_utf8).to_str().unwrap();
            assert_eq!(undo_repl, "sh");

            harfiy_engine_destroy(engine);
        }
    }
}
