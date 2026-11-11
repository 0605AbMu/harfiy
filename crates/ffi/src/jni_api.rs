//! JNI bindings for Harfiy Engine on Android (`uz.harfiy.keyboard.HarfiyEngine`).
//!
//! The Android accessibility service observes text changes rather than key events: it
//! drives the edit [`Session`] below; the keystroke FSM is not exposed here.

use std::ptr;
use std::sync::Mutex;

use jni::objects::{JClass, JIntArray, JString};
use jni::sys::{jboolean, jint, jintArray, jlong, jstring, JNI_TRUE};
use jni::JNIEnv;

use crate::session::{Session, SnapshotResult, Tick};

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

// ---------------------------------------------------------------------------------------
// Edit session (`uz.harfiy.keyboard.HarfiySession`).
//
// One accessibility service runs per process, so the session is a process-wide singleton.
// Every call comes from the service's main thread; the mutex only guards against misuse.

static SESSION: Mutex<Option<Session>> = Mutex::new(None);

fn with_session<R>(f: impl FnOnce(&mut Session) -> R) -> R {
    let mut guard = SESSION.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(Session::new))
}

/// `Tick` as a `long`: -1 idle, -2 snapshot now, otherwise the delay in milliseconds.
fn tick_code(tick: Tick) -> jlong {
    match tick {
        Tick::Idle => -1,
        Tick::Snapshot => -2,
        Tick::After(ms) => ms.max(0),
    }
}

fn opt_string(env: &mut JNIEnv, s: &JString) -> Option<String> {
    if s.is_null() {
        return None;
    }
    env.get_string(s)
        .ok()
        .map(|s| s.to_string_lossy().into_owned())
}

#[no_mangle]
pub extern "system" fn Java_uz_harfiy_keyboard_HarfiySession_configure(
    _env: JNIEnv,
    _class: JClass,
    enable_ng: jboolean,
    instant_undo: jboolean,
) {
    with_session(|s| s.configure(enable_ng == JNI_TRUE, instant_undo == JNI_TRUE));
}

#[no_mangle]
pub extern "system" fn Java_uz_harfiy_keyboard_HarfiySession_startInput(
    _env: JNIEnv,
    _class: JClass,
    enabled: jboolean,
    sel_start: jint,
    sel_end: jint,
    now: jlong,
) {
    with_session(|s| s.start_input(enabled == JNI_TRUE, sel_start, sel_end, now));
}

#[no_mangle]
pub extern "system" fn Java_uz_harfiy_keyboard_HarfiySession_finishInput(
    _env: JNIEnv,
    _class: JClass,
    now: jlong,
) {
    with_session(|s| s.finish_input(now));
}

#[no_mangle]
pub extern "system" fn Java_uz_harfiy_keyboard_HarfiySession_isFieldOn(
    _env: JNIEnv,
    _class: JClass,
) -> jboolean {
    with_session(|s| s.is_field_on()) as jboolean
}

#[no_mangle]
pub extern "system" fn Java_uz_harfiy_keyboard_HarfiySession_keyboardMode(
    _env: JNIEnv,
    _class: JClass,
) -> jint {
    with_session(|s| s.keyboard_mode())
}

#[no_mangle]
pub extern "system" fn Java_uz_harfiy_keyboard_HarfiySession_onSelection(
    _env: JNIEnv,
    _class: JClass,
    sel_start: jint,
    sel_end: jint,
    comp_start: jint,
    comp_end: jint,
    now: jlong,
) -> jlong {
    tick_code(with_session(|s| {
        s.on_selection(sel_start, sel_end, comp_start, comp_end, now)
    }))
}

#[no_mangle]
pub extern "system" fn Java_uz_harfiy_keyboard_HarfiySession_onTextChanged(
    mut env: JNIEnv,
    _class: JClass,
    from: jint,
    removed: jint,
    added: jint,
    tail: JString,
    now: jlong,
) -> jlong {
    let tail = opt_string(&mut env, &tail).unwrap_or_default();
    tick_code(with_session(|s| {
        s.on_text_changed(from, removed, added, &tail, now)
    }))
}

#[no_mangle]
pub extern "system" fn Java_uz_harfiy_keyboard_HarfiySession_onTick(
    _env: JNIEnv,
    _class: JClass,
    now: jlong,
) -> jlong {
    tick_code(with_session(|s| s.on_tick(now)))
}

/// Length of `out_meta` expected by [`Java_uz_harfiy_keyboard_HarfiySession_onSnapshot`].
pub const SNAPSHOT_META_LEN: usize = 12;

/// Returns the replacement to write, or null. `out_meta` receives
/// `[result, mode, start, end, verifyLen, retryMs, expectedLen, k0..k4]` where `result`
/// is 0 (done), 1 (write) or 2 (retry after `retryMs`).
#[no_mangle]
pub unsafe extern "system" fn Java_uz_harfiy_keyboard_HarfiySession_onSnapshot(
    mut env: JNIEnv,
    _class: JClass,
    before: JString,
    collapsed: jboolean,
    abs_cursor: jint,
    now: jlong,
    out_meta: jintArray,
) -> jstring {
    let before = opt_string(&mut env, &before).unwrap_or_default();
    let result = with_session(|s| s.on_snapshot(&before, collapsed == JNI_TRUE, abs_cursor, now));

    let mut meta = [0 as jint; SNAPSHOT_META_LEN];
    let mut replacement = None;
    match result {
        SnapshotResult::Done => {}
        SnapshotResult::Retry(ms) => {
            meta[0] = 2;
            meta[5] = ms as jint;
        }
        SnapshotResult::Write(plan) => {
            meta[0] = 1;
            meta[1] = plan.mode as jint;
            meta[2] = plan.start;
            meta[3] = plan.end;
            meta[4] = plan.verify_len;
            meta[6] = plan.expected.encode_utf16().count() as jint;
            for (i, k) in plan.kinds.iter().enumerate() {
                meta[7 + i] = *k as jint;
            }
            replacement = Some(plan.replacement);
        }
    }
    if env
        .set_int_array_region(JIntArray::from_raw(out_meta), 0, &meta)
        .is_err()
    {
        return ptr::null_mut();
    }
    match replacement.map(|r| env.new_string(r)) {
        Some(Ok(js)) => js.into_raw(),
        _ => ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "system" fn Java_uz_harfiy_keyboard_HarfiySession_onWritten(
    mut env: JNIEnv,
    _class: JClass,
    readback: JString,
    abs_cursor: jint,
    now: jlong,
) -> jboolean {
    let readback = opt_string(&mut env, &readback);
    with_session(|s| s.on_written(readback.as_deref(), abs_cursor, now)) as jboolean
}

#[no_mangle]
pub extern "system" fn Java_uz_harfiy_keyboard_HarfiySession_diagnostics(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    let text = with_session(|s| s.diagnostics());
    match env.new_string(text) {
        Ok(js) => js.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}
