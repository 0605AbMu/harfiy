//! JNI bindings for Harfiy Engine on Android (`uz.harfiy.keyboard.HarfiyEngine`).
//!
//! The Android accessibility service observes text changes rather than key events, so it
//! uses the stateless rule API; the keystroke FSM is not exposed here.

use std::ptr;

use harfiy_engine::plan_tail;
use jni::objects::{JClass, JIntArray, JString};
use jni::sys::{jboolean, jint, jintArray, jsize, jstring, JNI_TRUE};
use jni::JNIEnv;

use crate::edits::plan_edits_utf16;

/// Plan the conversion triggered by typing `curr` right after `prev` (`prev < 0` when
/// `curr` starts the field).
///
/// Returns the replacement string, or null when nothing converts. On a match `out_meta`
/// (length 2) receives `[delete_count, rule_kind]`.
#[no_mangle]
pub unsafe extern "system" fn Java_uz_harfiy_keyboard_HarfiyEngine_planTail(
    env: JNIEnv,
    _class: JClass,
    prev: jint,
    curr: jint,
    enable_ng: jboolean,
    out_meta: jintArray,
) -> jstring {
    let prev = u32::try_from(prev).ok().and_then(char::from_u32);
    let Some(curr) = u32::try_from(curr).ok().and_then(char::from_u32) else {
        return ptr::null_mut();
    };
    let Some(edit) = plan_tail(prev, curr, enable_ng == JNI_TRUE) else {
        return ptr::null_mut();
    };

    let meta = [edit.delete_count as jint, edit.kind as jint];
    if env
        .set_int_array_region(JIntArray::from_raw(out_meta), 0, &meta)
        .is_err()
    {
        return ptr::null_mut();
    }

    match env.new_string(edit.replacement) {
        Ok(js) => js.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// Plan every live-typing edit in `text`, for inserts of several characters at once
/// (glide typing, autocorrect). Returns the flat `[start, end, kind, replacement]`
/// groups described by [`plan_edits_utf16`], empty when nothing converts.
#[no_mangle]
pub unsafe extern "system" fn Java_uz_harfiy_keyboard_HarfiyEngine_planEditsNative(
    mut env: JNIEnv,
    _class: JClass,
    text: JString,
    enable_ng: jboolean,
) -> jintArray {
    let Ok(text) = env.get_string(&text) else {
        return ptr::null_mut();
    };
    let flat = plan_edits_utf16(&text.to_string_lossy(), enable_ng == JNI_TRUE);

    let Ok(array) = env.new_int_array(flat.len() as jsize) else {
        return ptr::null_mut();
    };
    if env.set_int_array_region(&array, 0, &flat).is_err() {
        return ptr::null_mut();
    }
    array.into_raw()
}

/// Convert an entire text in batch mode.
#[no_mangle]
pub unsafe extern "system" fn Java_uz_harfiy_keyboard_HarfiyEngine_convertText(
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
