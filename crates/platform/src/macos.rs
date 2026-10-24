//! macOS implementation of keyboard interceptor and emitter using CoreGraphics and AppKit.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;
use core_foundation_sys::base::{kCFAllocatorDefault, CFIndex, CFRelease};
use core_foundation_sys::runloop::{
    kCFRunLoopCommonModes, CFRunLoopAddSource, CFRunLoopGetCurrent, CFRunLoopRef,
    CFRunLoopRun, CFRunLoopSourceRef, CFRunLoopStop,
};

use harfiy_engine::{InputEvent, KeyAction, SpecialKey};
use crate::{KeyboardEmitter, KeyboardInterceptor, PlatformError};

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
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(options: core_foundation_sys::dictionary::CFDictionaryRef) -> bool;
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
                    CGEventSetIntegerValueField(down, kCGEventSourceUserData, HARFIY_MAGIC_FLAG as i64);
                    CGEventPost(kCGSessionEventTap, down);
                    CFRelease(down as _);
                }

                // Key Up
                let up = CGEventCreateKeyboardEvent(source, 51, false);
                if !up.is_null() {
                    CGEventSetIntegerValueField(up, kCGEventSourceUserData, HARFIY_MAGIC_FLAG as i64);
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
                    CGEventSetIntegerValueField(down, kCGEventSourceUserData, HARFIY_MAGIC_FLAG as i64);
                    CGEventPost(kCGSessionEventTap, down);
                    CFRelease(down as _);
                }

                // Key Up
                let up = CGEventCreateKeyboardEvent(source, 0, false);
                if !up.is_null() {
                    CGEventKeyboardSetUnicodeString(up, chunk.len() as _, chunk.as_ptr());
                    CGEventSetIntegerValueField(up, kCGEventSourceUserData, HARFIY_MAGIC_FLAG as i64);
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

/// macOS keyboard interceptor using CGEventTap.
#[derive(Default)]
pub struct MacOsInterceptor;

impl MacOsInterceptor {
    pub fn new() -> Self {
        Self
    }

    /// Check if the application has Accessibility permissions (AXIsProcessTrusted).
    pub fn is_accessibility_trusted() -> bool {
        unsafe { AXIsProcessTrusted() }
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

    /// Stop the interceptor hook and stop CFRunLoop from any thread.
    pub fn stop_hook() -> Result<(), PlatformError> {
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
        let mask = 1u64 << kCGEventKeyDown;

        // 2. Create the event tap on current login session (wait if accessibility permission pending)
        let mut showed_prompt = false;
        let tap = loop {
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
                show_info_dialog(
                    "Harfiy - Ruxsat talab qilinadi",
                    "Harfiy klaviaturadagi harflarni avtomatik o'girishi uchun Accessibility (Maxsus imkoniyatlar) ruxsati zarur.\n\nIltimos, ochilgan tizim sozlamalarida Harfiy-ni yoqing.",
                );
                showed_prompt = true;
            }

            std::thread::sleep(std::time::Duration::from_millis(1500));
        };

        // 3. Attach tap to current thread's CFRunLoop
        let run_loop_source = unsafe {
            CFMachPortCreateRunLoopSource(kCFAllocatorDefault, tap, 0)
        };

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

        println!("macOS klaviatura qabul qiluvchisi (CGEventTap) faollashdi.");

        // 4. Run CoreFoundation run loop (blocks this thread until stop() is called)
        unsafe {
            CFRunLoopRun();
        }

        // 5. Clean up resources upon exit
        {
            let mut guard = HOOK_STATE.lock().unwrap();
            if let Some(state) = guard.take() {
                unsafe {
                    CGEventTapEnable(state.tap_port, false);
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

/// Native macOS CGEventTap callback.
unsafe extern "C" fn macos_event_tap_callback(
    _proxy: CGEventTapProxy,
    r#type: u32,
    event: CGEventRef,
    _user_info: *mut libc::c_void,
) -> CGEventRef {
    // If the system temporarily disabled the tap due to timeout, re-enable it immediately
    if r#type == kCGEventTapDisabledByTimeout || r#type == kCGEventTapDisabledByUserInput {
        let guard = HOOK_STATE.lock().unwrap();
        if let Some(state) = guard.as_ref() {
            CGEventTapEnable(state.tap_port, true);
        }
        return event;
    }

    if r#type != kCGEventKeyDown {
        return event;
    }

    // Infinite loop protection: ignore synthetic events injected by Harfiy
    let user_data = CGEventGetIntegerValueField(event, kCGEventSourceUserData);
    if user_data == HARFIY_MAGIC_FLAG as i64 {
        return event;
    }

    // Do not interfere if Command, Control, or Option (Alt) modifier is down (e.g. Cmd+C, Cmd+V, Ctrl+C)
    let flags = CGEventGetFlags(event);
    if (flags & (kCGEventFlagMaskCommand | kCGEventFlagMaskControl | kCGEventFlagMaskAlternate)) != 0 {
        return event;
    }

    // Do not interfere if active application is excluded
    if is_current_app_excluded() {
        return event;
    }

    // Do not interfere if Game Mode is active
    if is_foreground_fullscreen() {
        return event;
    }

    let keycode = CGEventGetIntegerValueField(event, kCGKeyboardEventKeycode) as u16;

    // Determine InputEvent
    let input_event = if let Some(special) = map_special_key(keycode) {
        Some(InputEvent::Special(special))
    } else {
        let mut actual_len: libc::c_ulong = 0;
        let mut chars = [0u16; 4];
        CGEventKeyboardGetUnicodeString(event, 4, &mut actual_len, chars.as_mut_ptr());
        if actual_len > 0 {
            char::decode_utf16(chars[..actual_len as usize].iter().copied())
                .next()
                .and_then(|r| r.ok())
                .map(InputEvent::Char)
        } else {
            None
        }
    };

    if let Some(ev) = input_event {
        let mut guard = HOOK_STATE.lock().unwrap();
        if let Some(state) = guard.as_mut() {
            let action = (state.handler)(ev);
            match action {
                KeyAction::PassThrough => return event,
                KeyAction::Suppress => {
                    // Swallow the keystroke
                    return std::ptr::null_mut();
                }
                KeyAction::Transform {
                    backspaces_count,
                    replacement,
                } => {
                    // 1. Emit backspace(s) to erase previous character(s)
                    let mut emitter = MacOsEmitter::new();
                    let _ = emitter.emit_backspace(backspaces_count);
                    // 2. Emit replacement unicode string
                    let _ = emitter.emit_unicode(replacement);
                    // 3. Swallow the current key event so it is not typed
                    return std::ptr::null_mut();
                }
            }
        }
    }

    event
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
                    if cached_name == &name_lower && instant.elapsed() < Duration::from_millis(200) {
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
                    ) {
                        if win_pid != pid {
                            continue;
                        }
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
                    ) {
                        if layer != 0 {
                            continue;
                        }
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
                    ) {
                        if rect.origin.x <= 0.0
                            && rect.origin.y <= 0.0
                            && rect.size.width >= display_bounds.size.width
                            && rect.size.height >= display_bounds.size.height
                        {
                            is_full = true;
                            break;
                        }
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
                std::fs::write(plist_path, content).map_err(|e| PlatformError::Other(e.to_string()))?;
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
    if let Ok(output) = std::process::Command::new("osascript").arg("-e").arg(script).output() {
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
