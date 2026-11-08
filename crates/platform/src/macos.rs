//! macOS implementation of keyboard interceptor and emitter using CoreGraphics and AppKit.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::thread::ThreadId;
use std::time::{Duration, Instant};

use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;
use core_foundation_sys::base::{kCFAllocatorDefault, CFIndex, CFRelease};
use core_foundation_sys::date::CFAbsoluteTimeGetCurrent;
use core_foundation_sys::runloop::{
    kCFRunLoopCommonModes, CFRunLoopAddSource, CFRunLoopAddTimer, CFRunLoopGetCurrent,
    CFRunLoopRef, CFRunLoopRemoveSource, CFRunLoopRun, CFRunLoopSourceRef, CFRunLoopStop,
    CFRunLoopTimerCreate, CFRunLoopTimerInvalidate, CFRunLoopTimerRef,
    CFRunLoopTimerSetNextFireDate, CFRunLoopWakeUp,
};

use objc2::rc::autoreleasepool;
use objc2_foundation::{NSActivityOptions, NSProcessInfo, NSString};

use crate::trigger_buffer::{BufferAction, Held, TriggerBuffer};
use crate::{KeyboardEmitter, KeyboardInterceptor, PlatformError};
use harfiy_engine::{InputEvent, KeyAction, SpecialKey};

/// Magic tag attached to synthetic keystrokes to prevent infinite interception loops.
/// ASCII value for "HARF".
pub const HARFIY_MAGIC_FLAG: usize = 0x48415246;

type CGEventRef = *mut libc::c_void;
type CGEventSourceRef = *mut libc::c_void;
type CFMachPortRef = *mut libc::c_void;
type CGEventTapProxy = *mut libc::c_void;

type CGEventTapCallBack = unsafe extern "C" fn(
    proxy: CGEventTapProxy,
    r#type: u32,
    event: CGEventRef,
    user_info: *mut libc::c_void,
) -> CGEventRef;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrustedWithOptions(
        options: core_foundation_sys::dictionary::CFDictionaryRef,
    ) -> bool;
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: CGEventTapCallBack,
        user_info: *mut libc::c_void,
    ) -> CFMachPortRef;

    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);

    fn CGEventCreateKeyboardEvent(
        source: CGEventSourceRef,
        virtual_key: u16,
        key_down: bool,
    ) -> CGEventRef;

    fn CGEventKeyboardGetUnicodeString(
        event: CGEventRef,
        max_string_length: libc::c_ulong,
        actual_string_length: *mut libc::c_ulong,
        unicode_string: *mut u16,
    );

    fn CGEventKeyboardSetUnicodeString(
        event: CGEventRef,
        string_length: libc::c_ulong,
        unicode_string: *const u16,
    );

    fn CGEventCreateCopy(event: CGEventRef) -> CGEventRef;

    fn CGEventGetFlags(event: CGEventRef) -> u64;
    fn CGEventSetFlags(event: CGEventRef, flags: u64);

    fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
    fn CGEventSetIntegerValueField(event: CGEventRef, field: u32, value: i64);

    fn CGEventPost(tap: u32, event: CGEventRef);

    fn CGEventSourceCreate(state_id: i32) -> CGEventSourceRef;

    static kCGWindowOwnerPID: *const libc::c_void;
    static kCGWindowLayer: *const libc::c_void;
    static kCGWindowBounds: *const libc::c_void;

    fn CGMainDisplayID() -> u32;
    fn CGDisplayBounds(display: u32) -> core_graphics::geometry::CGRect;
    fn CGDisplayIsCaptured(display: u32) -> i32;
    fn CGWindowListCopyWindowInfo(
        status_options: u32,
        relative_to_window: u32,
    ) -> core_foundation_sys::array::CFArrayRef;
    fn CGRectMakeWithDictionaryRepresentation(
        dict: core_foundation_sys::dictionary::CFDictionaryRef,
        rect: *mut core_graphics::geometry::CGRect,
    ) -> bool;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFMachPortCreateRunLoopSource(
        allocator: core_foundation_sys::base::CFAllocatorRef,
        port: CFMachPortRef,
        order: CFIndex,
    ) -> CFRunLoopSourceRef;
}

#[allow(non_upper_case_globals)]
const kCGSessionEventTap: u32 = 1;
#[allow(non_upper_case_globals)]
const kCGHeadInsertEventTap: u32 = 0;
#[allow(non_upper_case_globals)]
const kCGEventTapOptionDefault: u32 = 0;

#[allow(non_upper_case_globals)]
const kCGEventKeyDown: u32 = 10;
#[allow(non_upper_case_globals)]
const kCGEventTapDisabledByTimeout: u32 = 0xFFFFFFFE;
#[allow(non_upper_case_globals)]
const kCGEventTapDisabledByUserInput: u32 = 0xFFFFFFFF;

#[allow(non_upper_case_globals)]
const kCGKeyboardEventKeycode: u32 = 9;
#[allow(non_upper_case_globals)]
const kCGEventSourceUserData: u32 = 42;
#[allow(non_upper_case_globals)]
const kCGEventSourceStateHIDSystem: i32 = 1;

#[allow(non_upper_case_globals)]
const kCGWindowListOptionOnScreenOnly: u32 = 1;
#[allow(non_upper_case_globals)]
const kCGWindowListExcludeDesktopElements: u32 = 16;
#[allow(non_upper_case_globals)]
const kCGNullWindowID: u32 = 0;

// Modifier masks
#[allow(non_upper_case_globals)]
const kCGEventFlagMaskCommand: u64 = 0x00100000;
#[allow(non_upper_case_globals)]
const kCGEventFlagMaskControl: u64 = 0x00040000;
#[allow(non_upper_case_globals)]
const kCGEventFlagMaskAlternate: u64 = 0x00080000;

/// Synthetic keyboard event emitter for macOS using CoreGraphics events.
#[derive(Debug, Clone, Default)]
pub struct MacOsEmitter;

impl MacOsEmitter {
    pub fn new() -> Self {
        Self
    }
}

impl KeyboardEmitter for MacOsEmitter {
    fn emit_backspace(&mut self, count: usize) -> Result<(), PlatformError> {
        if count == 0 {
            return Ok(());
        }

        unsafe {
            let source = CGEventSourceCreate(kCGEventSourceStateHIDSystem);
            for _ in 0..count {
                // Key Down (Virtual key 51 is Backspace on macOS)
                let down = CGEventCreateKeyboardEvent(source, 51, true);
                if !down.is_null() {
                    CGEventSetIntegerValueField(
                        down,
                        kCGEventSourceUserData,
                        HARFIY_MAGIC_FLAG as i64,
                    );
                    CGEventPost(kCGSessionEventTap, down);
                    CFRelease(down as _);
                }

                // Key Up
                let up = CGEventCreateKeyboardEvent(source, 51, false);
                if !up.is_null() {
                    CGEventSetIntegerValueField(
                        up,
                        kCGEventSourceUserData,
                        HARFIY_MAGIC_FLAG as i64,
                    );
                    CGEventPost(kCGSessionEventTap, up);
                    CFRelease(up as _);
                }
            }

            if !source.is_null() {
                CFRelease(source as _);
            }
        }

        Ok(())
    }

    fn emit_unicode(&mut self, text: &str) -> Result<(), PlatformError> {
        let code_units: Vec<u16> = text.encode_utf16().collect();
        if code_units.is_empty() {
            return Ok(());
        }

        unsafe {
            let source = CGEventSourceCreate(kCGEventSourceStateHIDSystem);

            // Chunk in slices of up to 20 code units (macOS CGEventKeyboardSetUnicodeString maximum)
            for chunk in code_units.chunks(20) {
                // Key Down
                let down = CGEventCreateKeyboardEvent(source, 0, true);
                if !down.is_null() {
                    CGEventKeyboardSetUnicodeString(down, chunk.len() as _, chunk.as_ptr());
                    CGEventSetIntegerValueField(
                        down,
                        kCGEventSourceUserData,
                        HARFIY_MAGIC_FLAG as i64,
                    );
                    CGEventPost(kCGSessionEventTap, down);
                    CFRelease(down as _);
                }

                // Key Up
                let up = CGEventCreateKeyboardEvent(source, 0, false);
                if !up.is_null() {
                    CGEventKeyboardSetUnicodeString(up, chunk.len() as _, chunk.as_ptr());
                    CGEventSetIntegerValueField(
                        up,
                        kCGEventSourceUserData,
                        HARFIY_MAGIC_FLAG as i64,
                    );
                    CGEventPost(kCGSessionEventTap, up);
                    CFRelease(up as _);
                }
            }

            if !source.is_null() {
                CFRelease(source as _);
            }
        }

        Ok(())
    }
}

/// Global state held during the active macOS keyboard tap.
struct HookState {
    handler: Box<dyn FnMut(InputEvent) -> KeyAction + Send>,
    tap_port: CFMachPortRef,
    run_loop: CFRunLoopRef,
}

unsafe impl Send for HookState {}

static HOOK_STATE: Mutex<Option<HookState>> = Mutex::new(None);
static STOP_REQUESTED: AtomicBool = AtomicBool::new(false);

/// macOS keyboard interceptor using CGEventTap.
#[derive(Default)]
pub struct MacOsInterceptor;

impl MacOsInterceptor {
    pub fn new() -> Self {
        Self
    }

    /// Check if the application has Accessibility permissions (AXIsProcessTrustedWithOptions).
    /// Uses AXTrustedCheckOptionPrompt: false to query live status without using stale cached process state.
    pub fn is_accessibility_trusted() -> bool {
        unsafe {
            let key = CFString::from_static_string("AXTrustedCheckOptionPrompt");
            let value = CFBoolean::false_value();
            let dict = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), value.as_CFType())]);
            AXIsProcessTrustedWithOptions(dict.as_concrete_TypeRef())
        }
    }

    /// Prompt system to show Accessibility settings dialog if not already trusted.
    pub fn prompt_accessibility() -> bool {
        unsafe {
            let key = CFString::from_static_string("AXTrustedCheckOptionPrompt");
            let value = CFBoolean::true_value();
            let dict = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), value.as_CFType())]);
            AXIsProcessTrustedWithOptions(dict.as_concrete_TypeRef())
        }
    }

    pub fn is_stop_requested() -> bool {
        STOP_REQUESTED.load(Ordering::SeqCst)
    }

    /// Stop the interceptor hook and stop CFRunLoop from any thread.
    pub fn stop_hook() -> Result<(), PlatformError> {
        STOP_REQUESTED.store(true, Ordering::SeqCst);
        // Whatever the run loop does not get to before it stops is typed when `run` exits
        flush_pending_trigger();
        let guard = HOOK_STATE.lock().unwrap();
        if let Some(state) = guard.as_ref() {
            unsafe {
                CFRunLoopStop(state.run_loop);
            }
        }
        Ok(())
    }
}

impl KeyboardInterceptor for MacOsInterceptor {
    fn run<F>(&mut self, handler: F) -> Result<(), PlatformError>
    where
        F: FnMut(InputEvent) -> KeyAction + Send + 'static,
    {
        if STOP_REQUESTED.load(Ordering::SeqCst) {
            return Ok(());
        }

        let mask = 1u64 << kCGEventKeyDown;

        // 2. Create the event tap on current login session (wait if accessibility permission pending)
        let mut showed_prompt = false;
        let tap = loop {
            if STOP_REQUESTED.load(Ordering::SeqCst) {
                return Ok(());
            }

            let t = unsafe {
                CGEventTapCreate(
                    kCGSessionEventTap,
                    kCGHeadInsertEventTap,
                    kCGEventTapOptionDefault,
                    mask,
                    macos_event_tap_callback,
                    std::ptr::null_mut(),
                )
            };
            if !t.is_null() {
                break t;
            }

            if !showed_prompt {
                Self::prompt_accessibility();
                showed_prompt = true;
            }

            // Granular sleep to respond quickly to shutdown or cancellation
            for _ in 0..10 {
                if STOP_REQUESTED.load(Ordering::SeqCst) {
                    return Ok(());
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        };

        // 3. Attach tap to current thread's CFRunLoop
        let run_loop_source = unsafe { CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0) };

        if run_loop_source.is_null() {
            unsafe { CFRelease(tap as _) };
            return Err(PlatformError::HookInstallationFailed(
                "CFMachPortCreateRunLoopSource failed".to_string(),
            ));
        }

        let current_run_loop = unsafe { CFRunLoopGetCurrent() };

        unsafe {
            CFRunLoopAddSource(current_run_loop, run_loop_source, kCFRunLoopCommonModes);
            CGEventTapEnable(tap, true);
        }

        {
            let mut guard = HOOK_STATE.lock().unwrap();
            *guard = Some(HookState {
                handler: Box::new(handler),
                tap_port: tap,
                run_loop: current_run_loop,
            });
        }

        // Smart trigger buffering runs entirely on this thread: its timer lives on this
        // run loop, so it never races with the tap callback.
        lock_trigger_buffer().reset();
        IN_FLIGHT.store(0, Ordering::SeqCst);
        if let Ok(mut tap_thread) = TAP_THREAD.lock() {
            *tap_thread = Some(std::thread::current().id());
        }
        let trigger_timer = unsafe {
            CFRunLoopTimerCreate(
                kCFAllocatorDefault,
                CFAbsoluteTimeGetCurrent() + TIMER_IDLE_SECS,
                TIMER_IDLE_SECS,
                0,
                0,
                trigger_timer_fired,
                std::ptr::null_mut(),
            )
        };
        if !trigger_timer.is_null() {
            unsafe { CFRunLoopAddTimer(current_run_loop, trigger_timer, kCFRunLoopCommonModes) };
            if let Ok(mut guard) = TRIGGER_TIMER.lock() {
                *guard = Some(TriggerTimer {
                    timer: trigger_timer,
                    run_loop: current_run_loop,
                });
            }
        }

        // Keep App Nap from coalescing the trigger timer, which would make a held key at the
        // end of a word show up late. Idle system sleep stays allowed.
        let process_info = NSProcessInfo::processInfo();
        let activity = process_info.beginActivityWithOptions_reason(
            NSActivityOptions::UserInitiatedAllowingIdleSystemSleep,
            &NSString::from_str("Harfiy klaviatura konvertatsiyasi"),
        );

        println!("macOS klaviatura qabul qiluvchisi (CGEventTap) faollashdi.");

        // 4. Run CoreFoundation run loop (blocks this thread until CFRunLoopStop is called)
        unsafe {
            CFRunLoopRun();
        }

        unsafe { process_info.endActivity(&activity) };

        // Type a key still held when the tap stops, so it is not lost. The tap is disabled
        // first: nothing services it any more, and the typed keys must not wait on it.
        unsafe { CGEventTapEnable(tap, false) };
        autoreleasepool(|_| release_held_key());
        if let Ok(mut guard) = TRIGGER_TIMER.lock() {
            if let Some(trigger) = guard.take() {
                unsafe {
                    CFRunLoopTimerInvalidate(trigger.timer);
                    CFRelease(trigger.timer as _);
                }
            }
        }
        if let Ok(mut tap_thread) = TAP_THREAD.lock() {
            *tap_thread = None;
        }

        // 5. Clean up resources upon exit
        {
            let mut guard = HOOK_STATE.lock().unwrap();
            if let Some(state) = guard.take() {
                unsafe {
                    CGEventTapEnable(state.tap_port, false);
                    CFRunLoopRemoveSource(current_run_loop, run_loop_source, kCFRunLoopCommonModes);
                    CFRelease(run_loop_source as _);
                    CFRelease(state.tap_port as _);
                }
            }
        }

        Ok(())
    }

    fn stop(&mut self) -> Result<(), PlatformError> {
        Self::stop_hook()
    }
}

/// Map macOS virtual keycodes to Harfiy SpecialKey.
fn map_special_key(keycode: u16) -> Option<SpecialKey> {
    match keycode {
        51 => Some(SpecialKey::Backspace),
        36 | 76 => Some(SpecialKey::Enter),
        53 => Some(SpecialKey::Escape),
        48 => Some(SpecialKey::Tab),
        123 => Some(SpecialKey::ArrowLeft),
        124 => Some(SpecialKey::ArrowRight),
        125 => Some(SpecialKey::ArrowDown),
        126 => Some(SpecialKey::ArrowUp),
        117 => Some(SpecialKey::Delete),
        115 => Some(SpecialKey::Home),
        119 => Some(SpecialKey::End),
        116 => Some(SpecialKey::PageUp),
        121 => Some(SpecialKey::PageDown),
        _ => None,
    }
}

// ============================================================================
// Smart Trigger Buffering
// ============================================================================

/// Tag for keystrokes the tap types in place of physical ones. Unlike
/// [`HARFIY_MAGIC_FLAG`], the tap counts these back in, so a physical key that
/// overtakes them in the event stream can be re-queued behind them.
const HARFIY_ORDERED_FLAG: i64 = 0x48415247; // "HARG"

/// Replayed keystrokes that do not come back within this window were dropped
/// and must stop holding up physical keys.
const IN_FLIGHT_STALE_AFTER: Duration = Duration::from_millis(200);

/// Typed before erasing text, so it replaces an inline autocomplete selection.
/// ZERO WIDTH SPACE is invisible and, unlike real whitespace, does not trigger
/// word-boundary features (autocorrect, editor input rules) on the way.
const AUTOCOMPLETE_PLACEHOLDER: &str = "\u{200B}";

/// Flags for typed text and Backspaces: no modifiers, so a held Cmd or Option
/// cannot turn them into a shortcut or a word deletion.
#[allow(non_upper_case_globals)]
const kCGEventFlagMaskNonCoalesced: u64 = 0x00000100;

/// macOS virtual key code of Backspace.
const KEYCODE_BACKSPACE: u16 = 51;

/// Key data needed to replay a held key exactly as the user typed it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MacKey {
    keycode: u16,
    flags: u64,
}

/// The tap's run loop timer that types a held key once the user pauses.
struct TriggerTimer {
    timer: CFRunLoopTimerRef,
    run_loop: CFRunLoopRef,
}

unsafe impl Send for TriggerTimer {}

static TRIGGER_BUFFER: Mutex<TriggerBuffer<MacKey>> = Mutex::new(TriggerBuffer::new());
static TRIGGER_TIMER: Mutex<Option<TriggerTimer>> = Mutex::new(None);
static TAP_THREAD: Mutex<Option<ThreadId>> = Mutex::new(None);
static BUFFER_TIMEOUT_MS: AtomicU64 = AtomicU64::new(250);
static TRIGGER_BUFFERING_ENABLED: AtomicBool = AtomicBool::new(true);
static CONVERSION_ACTIVE: AtomicBool = AtomicBool::new(true);
static AUTOCOMPLETE_FIX: AtomicBool = AtomicBool::new(true);
static ENABLE_NG_FLAG: AtomicBool = AtomicBool::new(false);

/// Keystrokes typed by the tap that it has not seen come back yet (key downs only,
/// since the tap only listens to those).
static IN_FLIGHT: AtomicU32 = AtomicU32::new(0);
static IN_FLIGHT_SINCE: Mutex<Option<Instant>> = Mutex::new(None);

/// Enable or disable smart trigger buffering (default: true).
pub fn set_trigger_buffering(enabled: bool) {
    TRIGGER_BUFFERING_ENABLED.store(enabled, Ordering::SeqCst);
    if !enabled {
        flush_pending_trigger();
    }
}

/// Check if smart trigger buffering is currently enabled.
pub fn is_trigger_buffering_enabled() -> bool {
    TRIGGER_BUFFERING_ENABLED.load(Ordering::Relaxed)
}

/// Set trigger buffering timeout in milliseconds (default: 250).
pub fn set_trigger_buffer_timeout(timeout_ms: u64) {
    BUFFER_TIMEOUT_MS.store(timeout_ms, Ordering::SeqCst);
}

/// Get current trigger buffering timeout in milliseconds.
pub fn get_trigger_buffer_timeout() -> u64 {
    BUFFER_TIMEOUT_MS.load(Ordering::Relaxed)
}

/// Tell the tap whether conversion is active (Harfiy is not paused).
/// While paused, trigger keys are no longer held back.
pub fn set_conversion_active(active: bool) {
    CONVERSION_ACTIVE.store(active, Ordering::SeqCst);
    if !active {
        flush_pending_trigger();
    }
}

/// Check if conversion is active (Harfiy is not paused).
pub fn is_conversion_active() -> bool {
    CONVERSION_ACTIVE.load(Ordering::Relaxed)
}

/// Enable or disable dismissing inline autocomplete before erasing characters (default: true).
///
/// When an application selects an inline suggestion after the typed text, a Backspace
/// erases the suggestion instead of the typed letter. A placeholder typed first replaces
/// the selection, so one extra Backspace then erases exactly the intended text.
pub fn set_autocomplete_fix(enabled: bool) {
    AUTOCOMPLETE_FIX.store(enabled, Ordering::SeqCst);
}

/// Check if inline autocomplete dismissal is enabled.
pub fn is_autocomplete_fix_enabled() -> bool {
    AUTOCOMPLETE_FIX.load(Ordering::Relaxed)
}

/// Configure 'ng' digraph recognition for the event tap.
pub fn set_enable_ng(enabled: bool) {
    ENABLE_NG_FLAG.store(enabled, Ordering::SeqCst);
    // A held key was judged by the previous rule set.
    flush_pending_trigger();
}

fn lock_trigger_buffer() -> MutexGuard<'static, TriggerBuffer<MacKey>> {
    match TRIGGER_BUFFER.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn is_tap_thread() -> bool {
    let current = std::thread::current().id();
    TAP_THREAD
        .lock()
        .map(|tap| *tap == Some(current))
        .unwrap_or(false)
}

/// Type any held trigger key immediately.
///
/// Callable from any thread: the work is handed to the tap's run loop so that it stays
/// ordered with live keystrokes.
pub fn flush_pending_trigger() {
    if is_tap_thread() {
        release_held_key();
        return;
    }
    let guard = match TRIGGER_TIMER.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(trigger) = guard.as_ref() {
        // Fire the trigger timer right away on the tap thread
        unsafe {
            CFRunLoopTimerSetNextFireDate(trigger.timer, CFAbsoluteTimeGetCurrent());
            CFRunLoopWakeUp(trigger.run_loop);
        }
    }
}

extern "C" fn trigger_timer_fired(_timer: CFRunLoopTimerRef, _info: *mut libc::c_void) {
    // The user paused (or a flush was requested): type the held key
    autoreleasepool(|_| release_held_key());
}

/// Type the held trigger key, if any. Runs on the tap thread.
fn release_held_key() {
    kill_trigger_timer();
    let mut out = Injection::default();
    release_held_into(&mut out);
    out.send();
}

/// Queue the held trigger key, if any, to be typed.
fn release_held_into(out: &mut Injection) {
    let held = lock_trigger_buffer().release();
    if let Some(held) = held {
        kill_trigger_timer();
        type_released(out, held);
    }
}

/// Queue a released key: replayed as typed, unless the engine converts it together
/// with the letter before it (e.g. a released 'g' after an 'n' that already reached the app).
///
/// Unlike on Windows, a change of focus is not checked: `NSWorkspace` only notices one
/// when the main run loop runs, so a stale answer would drop keys typed right after
/// switching apps. Cmd+Tab still types the held key first, through the shortcut path.
fn type_released(out: &mut Injection, held: Held<MacKey>) {
    match feed_engine(InputEvent::Char(held.ch)) {
        KeyAction::PassThrough => out.replay(held),
        KeyAction::Suppress => {}
        KeyAction::Transform {
            backspaces_count,
            replacement,
        } => out.transform(backspaces_count, replacement),
    }
}

fn set_trigger_timer(fire_in_secs: f64) {
    let guard = match TRIGGER_TIMER.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(trigger) = guard.as_ref() {
        unsafe {
            CFRunLoopTimerSetNextFireDate(trigger.timer, CFAbsoluteTimeGetCurrent() + fire_in_secs);
        }
    }
}

/// Arm (or re-arm) the timer that types a held key once the user pauses.
fn arm_trigger_timer() {
    set_trigger_timer(BUFFER_TIMEOUT_MS.load(Ordering::Relaxed) as f64 / 1000.0);
}

fn kill_trigger_timer() {
    set_trigger_timer(TIMER_IDLE_SECS);
}

/// Fire date offset that parks the trigger timer while nothing is held.
const TIMER_IDLE_SECS: f64 = 1.0e9;

/// Feed one event to the engine handler installed by `run`.
fn feed_engine(event: InputEvent) -> KeyAction {
    // A panic must not unwind out of the tap callback, so a poisoned lock is reused
    let mut guard = match HOOK_STATE.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    match guard.as_mut() {
        Some(state) => (state.handler)(event),
        None => KeyAction::PassThrough,
    }
}

/// Return the engine to its idle state, e.g. after a shortcut changed the text unseen.
fn reset_engine() {
    // Escape resets the engine and has no other effect on it
    let _ = feed_engine(InputEvent::Special(SpecialKey::Escape));
}

/// Forget everything the buffer and the engine know about the text before the caret.
fn forget_context() {
    lock_trigger_buffer().reset();
    reset_engine();
}

fn note_injected(count: u32) {
    if count == 0 {
        return;
    }
    IN_FLIGHT.fetch_add(count, Ordering::SeqCst);
    if let Ok(mut since) = IN_FLIGHT_SINCE.lock() {
        *since = Some(Instant::now());
    }
}

fn note_injection_arrived() {
    // Only the tap thread touches the counter, so load + store cannot race.
    let n = IN_FLIGHT.load(Ordering::SeqCst);
    if n > 0 {
        IN_FLIGHT.store(n - 1, Ordering::SeqCst);
    }
}

/// Whether the current physical key overtook keystrokes the tap typed earlier,
/// in which case it must be re-queued behind them to keep the typing order.
fn overtakes_injection() -> bool {
    if IN_FLIGHT.load(Ordering::SeqCst) == 0 {
        return false;
    }
    let stale = match IN_FLIGHT_SINCE.lock() {
        Ok(since) => !matches!(*since, Some(t) if t.elapsed() <= IN_FLIGHT_STALE_AFTER),
        Err(_) => true,
    };
    if stale {
        IN_FLIGHT.store(0, Ordering::SeqCst);
        return false;
    }
    true
}

/// One keystroke the tap types.
enum Stroke {
    /// A synthesized key press or release, optionally carrying the exact character typed.
    Key {
        keycode: u16,
        flags: u64,
        ch: Option<char>,
        down: bool,
    },
    /// An owned copy of a physical key down event, posted unchanged.
    Copy(CGEventRef),
}

/// Keystrokes the tap types, in order, in place of physical ones.
#[derive(Default)]
struct Injection {
    strokes: Vec<Stroke>,
}

impl Injection {
    fn is_empty(&self) -> bool {
        self.strokes.is_empty()
    }

    fn press(&mut self, keycode: u16, flags: u64, ch: Option<char>) {
        for down in [true, false] {
            self.strokes.push(Stroke::Key {
                keycode,
                flags,
                ch,
                down,
            });
        }
    }

    fn text(&mut self, text: &str) {
        for ch in text.chars() {
            self.press(0, kCGEventFlagMaskNonCoalesced, Some(ch));
        }
    }

    /// Type a held key the way the user typed it: its own key code and modifiers, so
    /// keydown-driven behaviour keeps working, and its exact character.
    fn replay(&mut self, held: Held<MacKey>) {
        self.press(held.key.keycode, held.key.flags, Some(held.ch));
    }

    /// Re-post the current physical key behind everything queued so far.
    fn forward(&mut self, event: CGEventRef) {
        let copy = unsafe { CGEventCreateCopy(event) };
        if !copy.is_null() {
            self.strokes.push(Stroke::Copy(copy));
        }
    }

    /// Erase `backspaces` characters before the caret and type `replacement`.
    fn transform(&mut self, backspaces: usize, replacement: &str) {
        if backspaces > 0 {
            let mut count = backspaces;
            if AUTOCOMPLETE_FIX.load(Ordering::Relaxed) {
                self.text(AUTOCOMPLETE_PLACEHOLDER);
                count += 1;
            }
            for _ in 0..count {
                self.press(KEYCODE_BACKSPACE, kCGEventFlagMaskNonCoalesced, None);
            }
        }
        self.text(replacement);
    }

    fn send(self) {
        if self.strokes.is_empty() {
            return;
        }
        let mut key_downs = 0;
        unsafe {
            let source = CGEventSourceCreate(kCGEventSourceStateHIDSystem);
            for stroke in self.strokes {
                let (event, down) = match stroke {
                    Stroke::Key {
                        keycode,
                        flags,
                        ch,
                        down,
                    } => {
                        let event = CGEventCreateKeyboardEvent(source, keycode, down);
                        if event.is_null() {
                            continue;
                        }
                        CGEventSetFlags(event, flags);
                        if let Some(ch) = ch {
                            let mut units = [0u16; 2];
                            let units = ch.encode_utf16(&mut units);
                            CGEventKeyboardSetUnicodeString(
                                event,
                                units.len() as _,
                                units.as_ptr(),
                            );
                        }
                        (event, down)
                    }
                    Stroke::Copy(event) => (event, true),
                };
                CGEventSetIntegerValueField(event, kCGEventSourceUserData, HARFIY_ORDERED_FLAG);
                CGEventPost(kCGSessionEventTap, event);
                CFRelease(event as _);
                if down {
                    key_downs += 1;
                }
            }
            if !source.is_null() {
                CFRelease(source as _);
            }
        }
        note_injected(key_downs);
    }
}

/// Native macOS CGEventTap callback.
unsafe extern "C" fn macos_event_tap_callback(
    _proxy: CGEventTapProxy,
    r#type: u32,
    event: CGEventRef,
    _user_info: *mut libc::c_void,
) -> CGEventRef {
    // If the system temporarily disabled the tap due to timeout, re-enable it immediately
    if r#type == kCGEventTapDisabledByTimeout {
        {
            let guard = HOOK_STATE.lock().unwrap();
            if let Some(state) = guard.as_ref() {
                CGEventTapEnable(state.tap_port, true);
            }
        }
        // Keys went past the tap unseen meanwhile: type the held key now and start over
        autoreleasepool(|_| {
            release_held_key();
            forget_context();
        });
        IN_FLIGHT.store(0, Ordering::SeqCst);
        return event;
    }

    // If the user revoked Accessibility permission in macOS System Settings,
    // do NOT re-enable the tap (doing so causes an infinite loop / 100% CPU lockup).
    // Instead, stop the run loop so the interceptor can exit cleanly and wait for re-grant.
    if r#type == kCGEventTapDisabledByUserInput {
        let guard = HOOK_STATE.lock().unwrap();
        if let Some(state) = guard.as_ref() {
            CFRunLoopStop(state.run_loop);
        }
        return event;
    }

    if r#type != kCGEventKeyDown {
        return event;
    }

    let user_data = CGEventGetIntegerValueField(event, kCGEventSourceUserData);

    // Keystrokes typed by the tap coming back through it: count them in
    if user_data == HARFIY_ORDERED_FLAG {
        note_injection_arrived();
        return event;
    }

    // Infinite loop protection: ignore synthetic events injected by Harfiy
    if user_data == HARFIY_MAGIC_FLAG as i64 {
        return event;
    }

    // The tap thread has no autorelease pool of its own for the AppKit calls below
    autoreleasepool(|_| {
        let mut out = Injection::default();
        let forward = handle_key_down(event, &mut out);

        // Once anything is queued, the current key is re-posted behind it instead of passed
        // on, so the application receives every keystroke in the order it was typed.
        if forward && out.is_empty() && !overtakes_injection() {
            return event;
        }
        if forward {
            out.forward(event);
        }
        out.send();
        std::ptr::null_mut()
    })
}

/// Decide what happens to a physical key press. Keystrokes to type go into `out`;
/// returns whether the key itself should still reach the application, after `out`.
unsafe fn handle_key_down(event: CGEventRef, out: &mut Injection) -> bool {
    // Do not interfere with Command or Control shortcuts (e.g. Cmd+C, Cmd+V, Ctrl+C),
    // excluded applications, or fullscreen games while Game Mode is active. What such
    // a key does to the text is unknown, so the engine starts over.
    // Note: Option (Alternate) is intentionally NOT treated as a shortcut because on macOS
    // international and custom layouts (e.g. Uzbek Latin on macOS), Option is used to type
    // characters and diacritics.
    // Option+Backspace deletes a whole word, which neither the buffer nor the engine models.
    let flags = CGEventGetFlags(event);
    let keycode = CGEventGetIntegerValueField(event, kCGKeyboardEventKeycode) as u16;
    let word_delete = keycode == KEYCODE_BACKSPACE && (flags & kCGEventFlagMaskAlternate) != 0;
    if (flags & (kCGEventFlagMaskCommand | kCGEventFlagMaskControl)) != 0
        || word_delete
        || is_current_app_excluded()
        || is_foreground_fullscreen()
    {
        release_held_into(out);
        forget_context();
        return true;
    }

    // Determine InputEvent
    let input_event = if let Some(special) = map_special_key(keycode) {
        InputEvent::Special(special)
    } else {
        let mut actual_len: libc::c_ulong = 0;
        let mut units = [0u16; 20];
        CGEventKeyboardGetUnicodeString(
            event,
            units.len() as _,
            &mut actual_len,
            units.as_mut_ptr(),
        );
        let len = (actual_len as usize).min(units.len());
        let mut chars = char::decode_utf16(units[..len].iter().copied());
        match (chars.next(), chars.next()) {
            (Some(Ok(ch)), None) => InputEvent::Char(ch),
            // A key without a character (e.g. a dead key): type the held key first, but
            // keep the engine state, since a composed character may still complete a pair
            (None, _) => {
                release_held_into(out);
                return true;
            }
            // A key typing several characters at once (custom layouts) is not converted
            _ => {
                release_held_into(out);
                forget_context();
                return true;
            }
        }
    };

    // Smart Trigger Buffering (eliminates autocomplete conflicts and visual flicker)
    if !TRIGGER_BUFFERING_ENABLED.load(Ordering::Relaxed)
        || !CONVERSION_ACTIVE.load(Ordering::Relaxed)
    {
        release_held_into(out);
        return apply_engine(input_event, out);
    }

    let key = MacKey { keycode, flags };

    let (held_before, step) = {
        let mut buffer = lock_trigger_buffer();
        buffer.set_enable_ng(ENABLE_NG_FLAG.load(Ordering::Relaxed));
        let held_before = buffer.held();
        let step = match input_event {
            InputEvent::Char(ch) => buffer.on_char(ch, key),
            InputEvent::Special(special) => buffer.on_special(special),
        };
        (held_before, step)
    };

    if let Some(held) = step.release {
        kill_trigger_timer();
        type_released(out, held);
    }

    match step.action {
        BufferAction::Pass => apply_engine(input_event, out),
        BufferAction::Hold => {
            // Swallow the trigger so the application (and its autocomplete) does not see it yet
            arm_trigger_timer();
            false
        }
        BufferAction::Convert { .. } => {
            kill_trigger_timer();
            match held_before {
                Some(held) => convert_held_pair(held, input_event, out),
                None => apply_engine(input_event, out),
            }
        }
        BufferAction::Cancel => {
            // Backspace erased the held key, which the application never saw. Show the
            // engine both, so it leaves the post-conversion (Instant Undo) state as usual.
            kill_trigger_timer();
            if let Some(held) = held_before {
                let _ = feed_engine(InputEvent::Char(held.ch));
                let _ = feed_engine(InputEvent::Special(SpecialKey::Backspace));
            }
            false
        }
    }
}

/// Type the conversion of a held key and the key completing its pair. The engine has the
/// final say; the buffer only spares the Backspace that would collide with autocomplete.
fn convert_held_pair(held: Held<MacKey>, event: InputEvent, out: &mut Injection) -> bool {
    match feed_engine(InputEvent::Char(held.ch)) {
        KeyAction::PassThrough => {}
        KeyAction::Suppress => return apply_engine(event, out),
        KeyAction::Transform {
            backspaces_count,
            replacement,
        } => {
            // The engine paired the held key with the letter before it instead
            out.transform(backspaces_count, replacement);
            return apply_engine(event, out);
        }
    }

    match feed_engine(event) {
        KeyAction::Transform { replacement, .. } => {
            out.text(replacement);
            false
        }
        // Conversion is off (e.g. Harfiy was paused in the meantime): type both keys as they are
        KeyAction::PassThrough => {
            out.replay(held);
            true
        }
        KeyAction::Suppress => {
            out.replay(held);
            false
        }
    }
}

/// Hand a key to the engine; returns whether the key itself should reach the application.
fn apply_engine(event: InputEvent, out: &mut Injection) -> bool {
    match feed_engine(event) {
        KeyAction::PassThrough => true,
        KeyAction::Suppress => false,
        KeyAction::Transform {
            backspaces_count,
            replacement,
        } => {
            out.transform(backspaces_count, replacement);
            false
        }
    }
}

// ============================================================================
// macOS Platform Helpers (Clipboard, Excluded Apps, Dialogs, etc.)
// ============================================================================

pub fn attach_parent_console() {}

static EXCLUDED_APPS: Mutex<Vec<String>> = Mutex::new(Vec::new());
static CACHED_FG_STATUS: Mutex<Option<(String, bool, Instant)>> = Mutex::new(None);

pub fn set_excluded_apps(apps: &[String]) {
    let mut guard = EXCLUDED_APPS.lock().unwrap();
    *guard = apps.iter().map(|s| s.to_lowercase()).collect();
}

pub fn is_current_app_excluded() -> bool {
    use objc2_app_kit::NSWorkspace;

    let ws = NSWorkspace::sharedWorkspace();
    if let Some(app) = ws.frontmostApplication() {
        if let Some(name) = app.localizedName() {
            let name_lower = name.to_string().to_lowercase();

            {
                let cache = CACHED_FG_STATUS.lock().unwrap();
                if let Some((ref cached_name, excluded, instant)) = *cache {
                    if cached_name == &name_lower && instant.elapsed() < Duration::from_millis(200)
                    {
                        return excluded;
                    }
                }
            }

            let guard = EXCLUDED_APPS.lock().unwrap();
            let is_excluded = guard.iter().any(|ex| {
                let ex_clean = ex.trim_end_matches(".app");
                let name_clean = name_lower.trim_end_matches(".app");
                ex_clean == name_clean
            });

            let mut cache = CACHED_FG_STATUS.lock().unwrap();
            *cache = Some((name_lower, is_excluded, Instant::now()));

            return is_excluded;
        }
    }
    false
}

pub fn scan_running_apps() -> Vec<String> {
    use objc2_app_kit::NSWorkspace;

    let ws = NSWorkspace::sharedWorkspace();
    let running = ws.runningApplications();
    let mut names = Vec::new();

    for app in running.iter() {
        if let Some(name) = app.localizedName() {
            let name_str = name.to_string().to_lowercase();
            if !name_str.is_empty()
                && name_str != "harfiy"
                && name_str != "finder"
                && !names.contains(&name_str)
            {
                names.push(name_str);
            }
        }
    }
    names.sort();
    names
}

pub fn pick_executable_dialog() -> Option<String> {
    let script = r#"
        try
            set chosenFile to choose file of type {"app"} default location (path to applications folder) with prompt "Harfiy - Istisno dasturni tanlang (.app)"
            return POSIX path of chosenFile
        on error
            return ""
        end try
    "#;

    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .ok()?;

    if output.status.success() {
        let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path_str.is_empty() {
            let trimmed = path_str.trim_end_matches('/');
            let path = std::path::Path::new(trimmed);
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                return Some(stem.to_string());
            }
        }
    }
    None
}

/// Open the native macOS file chooser to pick one file.
/// The type filter is not applied here; an unsupported file is reported by the caller.
pub fn pick_file_dialog(
    title: &str,
    _filter_name: &str,
    _extensions: &[&str],
) -> Option<std::path::PathBuf> {
    let script = format!(
        "try\nreturn POSIX path of (choose file with prompt \"{}\")\non error\nreturn \"\"\nend try",
        applescript_quote(title)
    );
    run_osascript_path(&script)
}

/// Escape text for use inside an AppleScript string literal.
fn applescript_quote(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

fn run_osascript_path(script: &str) -> Option<std::path::PathBuf> {
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let chosen = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!chosen.is_empty()).then(|| std::path::PathBuf::from(chosen))
}

/// Open the native macOS Save As panel, pre-filled with `default_name` in `initial_dir`.
/// Returns `None` if the user cancels.
pub fn pick_save_file_dialog(
    title: &str,
    initial_dir: Option<&std::path::Path>,
    default_name: &str,
) -> Option<std::path::PathBuf> {
    let location = initial_dir
        .map(|d| {
            format!(
                " default location (POSIX file \"{}\")",
                applescript_quote(&d.to_string_lossy())
            )
        })
        .unwrap_or_default();
    let script = format!(
        "try\nreturn POSIX path of (choose file name with prompt \"{}\" default name \"{}\"{})\non error\nreturn \"\"\nend try",
        applescript_quote(title),
        applescript_quote(default_name),
        location
    );
    run_osascript_path(&script)
}

static GAME_MODE_ENABLED: AtomicBool = AtomicBool::new(true);
static CACHED_GAME_STATUS: Mutex<Option<(bool, Instant)>> = Mutex::new(None);

pub fn set_game_mode(enabled: bool) {
    GAME_MODE_ENABLED.store(enabled, Ordering::SeqCst);
    let mut cache = CACHED_GAME_STATUS.lock().unwrap();
    *cache = None;
}

pub fn is_game_mode() -> bool {
    GAME_MODE_ENABLED.load(Ordering::SeqCst)
}

pub fn is_foreground_fullscreen() -> bool {
    if !is_game_mode() {
        return false;
    }

    // Check cached result with 200ms TTL for high-frequency keyboard tap callback
    {
        let cache = CACHED_GAME_STATUS.lock().unwrap();
        if let Some((is_fullscreen, instant)) = *cache {
            if instant.elapsed() < Duration::from_millis(200) {
                return is_fullscreen;
            }
        }
    }

    let is_fullscreen = check_is_game_fullscreen();

    let mut cache = CACHED_GAME_STATUS.lock().unwrap();
    *cache = Some((is_fullscreen, Instant::now()));

    is_fullscreen
}

fn check_is_game_fullscreen() -> bool {
    use objc2_app_kit::NSWorkspace;

    let ws = NSWorkspace::sharedWorkspace();
    let front = match ws.frontmostApplication() {
        Some(app) => app,
        None => return false,
    };

    let name = match front.localizedName() {
        Some(n) => n.to_string().to_lowercase(),
        None => return false,
    };

    if name.is_empty() || name == "finder" || name == "harfiy" {
        return false;
    }

    // 1. Check if display is captured (exclusive fullscreen mode used by 3D/OpenGL/Metal games)
    unsafe {
        let main_id = CGMainDisplayID();
        if CGDisplayIsCaptured(main_id) != 0 {
            return true;
        }
    }

    // 2. Check if frontmost window covers the entire display
    unsafe {
        let main_id = CGMainDisplayID();
        let display_bounds = CGDisplayBounds(main_id);
        let pid = front.processIdentifier();

        let window_list = CGWindowListCopyWindowInfo(
            kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
            kCGNullWindowID,
        );

        if !window_list.is_null() {
            let count = core_foundation_sys::array::CFArrayGetCount(window_list);
            let mut is_full = false;

            for i in 0..count {
                let dict = core_foundation_sys::array::CFArrayGetValueAtIndex(window_list, i)
                    as core_foundation_sys::dictionary::CFDictionaryRef;
                if dict.is_null() {
                    continue;
                }

                // Check window owner PID
                let pid_ref = core_foundation_sys::dictionary::CFDictionaryGetValue(
                    dict,
                    kCGWindowOwnerPID as *const _,
                );
                if !pid_ref.is_null() {
                    let mut win_pid: i32 = 0;
                    if core_foundation_sys::number::CFNumberGetValue(
                        pid_ref as core_foundation_sys::number::CFNumberRef,
                        core_foundation_sys::number::kCFNumberIntType,
                        &mut win_pid as *mut _ as *mut _,
                    ) && win_pid != pid
                    {
                        continue;
                    }
                }

                // Check window layer (0 = normal window layer)
                let layer_ref = core_foundation_sys::dictionary::CFDictionaryGetValue(
                    dict,
                    kCGWindowLayer as *const _,
                );
                if !layer_ref.is_null() {
                    let mut layer: i32 = 0;
                    if core_foundation_sys::number::CFNumberGetValue(
                        layer_ref as core_foundation_sys::number::CFNumberRef,
                        core_foundation_sys::number::kCFNumberIntType,
                        &mut layer as *mut _ as *mut _,
                    ) && layer != 0
                    {
                        continue;
                    }
                }

                // Check window bounds
                let bounds_ref = core_foundation_sys::dictionary::CFDictionaryGetValue(
                    dict,
                    kCGWindowBounds as *const _,
                );
                if !bounds_ref.is_null() {
                    let mut rect = core_graphics::geometry::CGRect::new(
                        &core_graphics::geometry::CGPoint::new(0.0, 0.0),
                        &core_graphics::geometry::CGSize::new(0.0, 0.0),
                    );
                    if CGRectMakeWithDictionaryRepresentation(
                        bounds_ref as core_foundation_sys::dictionary::CFDictionaryRef,
                        &mut rect,
                    ) && rect.origin.x <= 0.0
                        && rect.origin.y <= 0.0
                        && rect.size.width >= display_bounds.size.width
                        && rect.size.height >= display_bounds.size.height
                    {
                        is_full = true;
                        break;
                    }
                }
            }

            core_foundation_sys::base::CFRelease(window_list as *const _);
            if is_full {
                return true;
            }
        }
    }

    false
}

pub fn is_autostart_enabled() -> bool {
    if let Some(home) = directories::BaseDirs::new() {
        let plist_path = home
            .config_dir()
            .parent()
            .map(|p| p.join("LaunchAgents/uz.harfiy.app.plist"))
            .unwrap_or_default();
        return plist_path.exists();
    }
    false
}

pub fn set_autostart(enabled: bool) -> Result<(), PlatformError> {
    if let Some(home) = directories::BaseDirs::new() {
        if let Some(parent) = home.config_dir().parent() {
            let launch_agents = parent.join("LaunchAgents");
            let plist_path = launch_agents.join("uz.harfiy.app.plist");
            if enabled {
                let _ = std::fs::create_dir_all(&launch_agents);
                let exe_path = std::env::current_exe().unwrap_or_default();
                let content = format!(
                    r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>uz.harfiy.app</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>start</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
</dict>
</plist>"#,
                    exe_path.display()
                );
                std::fs::write(plist_path, content)
                    .map_err(|e| PlatformError::Other(e.to_string()))?;
            } else if plist_path.exists() {
                let _ = std::fs::remove_file(plist_path);
            }
        }
    }
    Ok(())
}

/// Read text currently on the macOS general pasteboard.
pub fn get_clipboard_text() -> Option<String> {
    use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
    let pb = NSPasteboard::generalPasteboard();
    let ns_str = pb.stringForType(unsafe { NSPasteboardTypeString })?;
    Some(ns_str.to_string())
}

/// Write text to the macOS general pasteboard.
pub fn set_clipboard_text(text: &str) -> bool {
    use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
    use objc2_foundation::NSString;
    let pb = NSPasteboard::generalPasteboard();
    unsafe {
        let _ = pb.clearContents();
        let s = NSString::from_str(text);
        pb.setString_forType(&s, NSPasteboardTypeString)
    }
}

/// Emit synthetic Cmd+C (Copy) on macOS.
pub fn emit_copy() {
    unsafe {
        let source = CGEventSourceCreate(kCGEventSourceStateHIDSystem);
        // Keycode 8 is 'C' on macOS
        let down = CGEventCreateKeyboardEvent(source, 8, true);
        if !down.is_null() {
            CGEventSetFlags(down, kCGEventFlagMaskCommand);
            CGEventSetIntegerValueField(down, kCGEventSourceUserData, HARFIY_MAGIC_FLAG as i64);
            CGEventPost(kCGSessionEventTap, down);
            CFRelease(down as _);
        }

        let up = CGEventCreateKeyboardEvent(source, 8, false);
        if !up.is_null() {
            CGEventSetFlags(up, kCGEventFlagMaskCommand);
            CGEventSetIntegerValueField(up, kCGEventSourceUserData, HARFIY_MAGIC_FLAG as i64);
            CGEventPost(kCGSessionEventTap, up);
            CFRelease(up as _);
        }

        if !source.is_null() {
            CFRelease(source as _);
        }
    }
}

/// Emit synthetic Cmd+V (Paste) on macOS.
pub fn emit_paste() {
    unsafe {
        let source = CGEventSourceCreate(kCGEventSourceStateHIDSystem);
        // Keycode 9 is 'V' on macOS
        let down = CGEventCreateKeyboardEvent(source, 9, true);
        if !down.is_null() {
            CGEventSetFlags(down, kCGEventFlagMaskCommand);
            CGEventSetIntegerValueField(down, kCGEventSourceUserData, HARFIY_MAGIC_FLAG as i64);
            CGEventPost(kCGSessionEventTap, down);
            CFRelease(down as _);
        }

        let up = CGEventCreateKeyboardEvent(source, 9, false);
        if !up.is_null() {
            CGEventSetFlags(up, kCGEventFlagMaskCommand);
            CGEventSetIntegerValueField(up, kCGEventSourceUserData, HARFIY_MAGIC_FLAG as i64);
            CGEventPost(kCGSessionEventTap, up);
            CFRelease(up as _);
        }

        if !source.is_null() {
            CFRelease(source as _);
        }
    }
}

pub fn show_info_dialog(title: &str, message: &str) {
    let script = format!(
        r#"display alert "{}" message "{}" as informational"#,
        title.replace('"', "\\\""),
        message.replace('"', "\\\"")
    );
    let _ = std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .spawn();
}

pub fn show_confirm_dialog(title: &str, message: &str) -> bool {
    let script = format!(
        r#"button returned of (display alert "{}" message "{}" buttons {{"Yo'q", "Ha"}} default button "Ha")"#,
        title.replace('"', "\\\""),
        message.replace('"', "\\\"")
    );
    if let Ok(output) = std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
    {
        let resp = String::from_utf8_lossy(&output.stdout);
        return resp.trim() == "Ha";
    }
    true
}

pub fn show_status_hud(active: bool) {
    let msg = if active {
        "Harfiy: Faol"
    } else {
        "Harfiy: To'xtatildi"
    };
    let script = format!(r#"display notification "{}" with title "Harfiy""#, msg);
    let _ = std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .spawn();
}

pub fn show_custom_hud(message: &str, _is_success: bool) {
    let script = format!(
        r#"display notification "{}" with title "Harfiy""#,
        message.replace('"', "\\\"")
    );
    let _ = std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .spawn();
}

/// Open macOS System Settings directly to Privacy & Security -> Accessibility and prompt for permissions.
pub fn open_accessibility_settings() {
    let _ = MacOsInterceptor::prompt_accessibility();
    let _ = std::process::Command::new("/usr/bin/open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_mode_and_fullscreen_detection() {
        set_game_mode(true);
        assert!(is_game_mode());
        let _ = is_foreground_fullscreen();

        set_game_mode(false);
        assert!(!is_game_mode());
        assert!(!is_foreground_fullscreen());

        set_game_mode(true);
        assert!(is_game_mode());
    }
}
