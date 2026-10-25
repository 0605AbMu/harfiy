//! JNI bindings for Harfiy Engine for Android (InputMethodService).

use std::ptr;

use harfiy_engine::{EngineConfig, HarfiyEngine, InputEvent, KeyAction, SpecialKey};
use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jint, jlong, jstring, JNI_FALSE, JNI_TRUE};
use jni::JNIEnv;

/// Helper to cast jlong handle to &mut HarfiyEngine.
unsafe fn to_engine<'a>(handle: jlong) -> Option<&'a mut HarfiyEngine> {
    if handle == 0 {
        None
    } else {
        (handle as *mut HarfiyEngine).as_mut()
    }
}

/// Create a new HarfiyEngine instance and return its raw pointer as jlong.
#[no_mangle]
pub unsafe extern "system" fn Java_uz_harfiy_keyboard_HarfiyEngineBridge_createEngine(
    _env: JNIEnv,
    _class: JClass,
    enable_ng: jboolean,
) -> jlong {
    let engine = HarfiyEngine::new(EngineConfig {
        enable_ng: enable_ng == JNI_TRUE,
        enabled: true,
    });
    Box::into_raw(Box::new(engine)) as jlong
}

/// Destroy the HarfiyEngine instance.
#[no_mangle]
pub unsafe extern "system" fn Java_uz_harfiy_keyboard_HarfiyEngineBridge_destroyEngine(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    if handle != 0 {
        drop(Box::from_raw(handle as *mut HarfiyEngine));
    }
}

/// Reset the engine state back to Idle.
#[no_mangle]
pub unsafe extern "system" fn Java_uz_harfiy_keyboard_HarfiyEngineBridge_resetEngine(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    if let Some(engine) = to_engine(handle) {
        engine.reset();
    }
}

/// Dynamically update 'ng' rule configuration.
#[no_mangle]
pub unsafe extern "system" fn Java_uz_harfiy_keyboard_HarfiyEngineBridge_setEnableNg(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    enable_ng: jboolean,
) {
    if let Some(engine) = to_engine(handle) {
        engine.set_enable_ng(enable_ng == JNI_TRUE);
    }
}

/// Query if 'ng' rule is enabled.
#[no_mangle]
pub unsafe extern "system" fn Java_uz_harfiy_keyboard_HarfiyEngineBridge_isEnableNg(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jboolean {
    if let Some(engine) = to_engine(handle) {
        if engine.is_enable_ng() {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    } else {
        JNI_FALSE
    }
}

/// Encode Harfiy action into a single 64-bit integer for zero-allocation JNI interop:
/// - Bits 0..7: ActionType (0: PassThrough, 1: Suppress, 2: Transform)
/// - Bits 8..15: Backspaces count (0..255)
/// - Bits 16..63: Reserved
fn encode_action(action: KeyAction) -> (jlong, Option<&'static str>) {
    match action {
        KeyAction::PassThrough => (0, None),
        KeyAction::Suppress => (1, None),
        KeyAction::Transform {
            backspaces_count,
            replacement,
        } => {
            let encoded = 2 | ((backspaces_count as jlong & 0xFF) << 8);
            (encoded, Some(replacement))
        }
    }
}

/// Process a single character and return:
/// - If Transform: replacement String.
/// - If PassThrough or Suppress: null.
/// The `out_meta` array of length 2 receives:
/// - out_meta[0]: action_type (0: PassThrough, 1: Suppress, 2: Transform)
/// - out_meta[1]: backspaces_count
#[no_mangle]
pub unsafe extern "system" fn Java_uz_harfiy_keyboard_HarfiyEngineBridge_processChar(
    env: JNIEnv,
    _class: JClass,
    handle: jlong,
    unicode_char: jint,
    out_meta: jni::sys::jintArray,
) -> jstring {
    let Some(engine) = to_engine(handle) else {
        return ptr::null_mut();
    };

    let Some(ch) = char::from_u32(unicode_char as u32) else {
        return ptr::null_mut();
    };

    let action = engine.process(InputEvent::Char(ch));
    let (encoded, replacement_opt) = encode_action(action);

    let action_type = (encoded & 0xFF) as i32;
    let backspaces = ((encoded >> 8) & 0xFF) as i32;

    if !out_meta.is_null() {
        let meta_buf = [action_type, backspaces];
        let _ = env.set_int_array_region(
            unsafe { jni::objects::JIntArray::from_raw(out_meta) },
            0,
            &meta_buf,
        );
    }

    if let Some(repl) = replacement_opt {
        match env.new_string(repl) {
            Ok(js) => js.into_raw(),
            Err(_) => ptr::null_mut(),
        }
    } else {
        ptr::null_mut()
    }
}

/// Process Backspace event (supporting Instant Undo).
#[no_mangle]
pub unsafe extern "system" fn Java_uz_harfiy_keyboard_HarfiyEngineBridge_processBackspace(
    env: JNIEnv,
    _class: JClass,
    handle: jlong,
    out_meta: jni::sys::jintArray,
) -> jstring {
    let Some(engine) = to_engine(handle) else {
        return ptr::null_mut();
    };

    let action = engine.process(InputEvent::Special(SpecialKey::Backspace));
    let (encoded, replacement_opt) = encode_action(action);

    let action_type = (encoded & 0xFF) as i32;
    let backspaces = ((encoded >> 8) & 0xFF) as i32;

    if !out_meta.is_null() {
        let meta_buf = [action_type, backspaces];
        let _ = env.set_int_array_region(
            unsafe { jni::objects::JIntArray::from_raw(out_meta) },
            0,
            &meta_buf,
        );
    }

    if let Some(repl) = replacement_opt {
        match env.new_string(repl) {
            Ok(js) => js.into_raw(),
            Err(_) => ptr::null_mut(),
        }
    } else {
        ptr::null_mut()
    }
}

/// Convert an entire text in batch mode.
#[no_mangle]
pub unsafe extern "system" fn Java_uz_harfiy_keyboard_HarfiyEngineBridge_convertText(
    mut env: JNIEnv,
    _class: JClass,
    input_text: JString,
    enable_ng: jboolean,
    smart_apostrophe: jboolean,
) -> jstring {
    let Ok(input_str) = env.get_string(&input_text) else {
        return ptr::null_mut();
    };

    let input_slice = input_str.to_string_lossy();
    let (converted, _) = harfiy_engine::convert_text(
        &input_slice,
        enable_ng == JNI_TRUE,
        smart_apostrophe == JNI_TRUE,
    );

    match env.new_string(converted) {
        Ok(js) => js.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}
