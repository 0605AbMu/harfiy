//! Windows implementation of keyboard interceptor and emitter using Win32 API.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use harfiy_engine::{InputEvent, KeyAction, SpecialKey};
use windows::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::{CreateMutexW, GetCurrentThreadId};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, GetKeyboardLayout, GetKeyboardState, SendInput, ToUnicodeEx, INPUT, INPUT_0,
    INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE,
    VIRTUAL_KEY, VK_BACK, VK_CAPITAL, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE,
    VK_HOME, VK_LEFT, VK_LWIN, VK_MENU, VK_NEXT, VK_PRIOR, VK_RETURN, VK_RIGHT, VK_RWIN,
    VK_SHIFT, VK_TAB, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, PeekMessageW, PostThreadMessageW,
    SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, MSG,
    PM_NOREMOVE, PM_REMOVE, WH_KEYBOARD_LL, WM_KEYDOWN, WM_QUIT, WM_SYSKEYDOWN,
};

use crate::{KeyboardEmitter, KeyboardInterceptor, PlatformError};

/// Magic tag attached to `dwExtraInfo` in synthetic keystrokes to prevent infinite loops.
/// ASCII value for "HARF".
pub const HARFIY_MAGIC_FLAG: usize = 0x48415246;

/// Named Mutex guard for ensuring only one instance of Harfiy runs at a time.
pub struct SingleInstanceGuard {
    handle: HANDLE,
}

// Safety: The Win32 mutex handle is thread-safe and can be sent across threads.
unsafe impl Send for SingleInstanceGuard {}
unsafe impl Sync for SingleInstanceGuard {}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

/// Try to acquire the single instance lock for Harfiy.
/// Returns Some(guard) if this is the only instance, or None if another instance is already running.
pub fn acquire_single_instance_lock() -> Option<SingleInstanceGuard> {
    use windows::core::w;

    unsafe {
        let handle = CreateMutexW(None, true, w!("HarfiySingleInstanceMutex")).ok()?;
        if GetLastError().0 == 183 {
            // ERROR_ALREADY_EXISTS (183)
            let _ = CloseHandle(handle);
            return None;
        }
        Some(SingleInstanceGuard { handle })
    }
}

/// Global state held during the active keyboard hook.
struct HookState {
    handler: Box<dyn FnMut(InputEvent) -> KeyAction + Send>,
    hook_handle_raw: isize,
}

static HOOK_STATE: Mutex<Option<HookState>> = Mutex::new(None);
static HOOK_THREAD_ID: AtomicU32 = AtomicU32::new(0);

/// Synthetic keyboard event emitter using `SendInput`.
#[derive(Debug, Clone, Default)]
pub struct WindowsEmitter;

impl WindowsEmitter {
    pub fn new() -> Self {
        Self
    }
}

impl KeyboardEmitter for WindowsEmitter {
    fn emit_backspace(&mut self, count: usize) -> Result<(), PlatformError> {
        if count == 0 {
            return Ok(());
        }

        let mut inputs = Vec::with_capacity(count * 2);
        for _ in 0..count {
            // Key Down
            inputs.push(INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_BACK,
                        wScan: 0,
                        dwFlags: KEYBD_EVENT_FLAGS(0),
                        time: 0,
                        dwExtraInfo: HARFIY_MAGIC_FLAG,
                    },
                },
            });
            // Key Up
            inputs.push(INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_BACK,
                        wScan: 0,
                        dwFlags: KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: HARFIY_MAGIC_FLAG,
                    },
                },
            });
        }

        let sent = unsafe {
            SendInput(
                &inputs,
                std::mem::size_of::<INPUT>() as i32,
            )
        };

        if sent as usize != inputs.len() {
            return Err(PlatformError::SendInputFailed(format!(
                "Expected to send {} inputs, sent {}",
                inputs.len(),
                sent
            )));
        }

        Ok(())
    }

    fn emit_unicode(&mut self, text: &str) -> Result<(), PlatformError> {
        let code_units: Vec<u16> = text.encode_utf16().collect();
        if code_units.is_empty() {
            return Ok(());
        }

        let mut inputs = Vec::with_capacity(code_units.len() * 2);
        for cu in code_units {
            // Key Down
            inputs.push(INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(0),
                        wScan: cu,
                        dwFlags: KEYEVENTF_UNICODE,
                        time: 0,
                        dwExtraInfo: HARFIY_MAGIC_FLAG,
                    },
                },
            });
            // Key Up
            inputs.push(INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(0),
                        wScan: cu,
                        dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: HARFIY_MAGIC_FLAG,
                    },
                },
            });
        }

        let sent = unsafe {
            SendInput(
                &inputs,
                std::mem::size_of::<INPUT>() as i32,
            )
        };

        if sent as usize != inputs.len() {
            return Err(PlatformError::SendInputFailed(format!(
                "Expected to send {} inputs, sent {}",
                inputs.len(),
                sent
            )));
        }

        Ok(())
    }
}

/// Windows keyboard hook interceptor using `WH_KEYBOARD_LL`.
#[derive(Default)]
pub struct WindowsInterceptor {
    is_running: bool,
}

impl WindowsInterceptor {
    pub fn new() -> Self {
        Self { is_running: false }
    }

    /// Globally stop and unhook the Windows low-level keyboard hook.
    /// Completely thread-safe and callable from any thread without deadlocking.
    pub fn stop_hook() -> Result<(), PlatformError> {
        let mut state_guard = HOOK_STATE.lock().unwrap();
        if let Some(state) = state_guard.take() {
            unsafe {
                let _ = UnhookWindowsHookEx(HHOOK(state.hook_handle_raw as *mut _));
            }
        }

        let thread_id = HOOK_THREAD_ID.swap(0, Ordering::SeqCst);
        if thread_id != 0 {
            for _ in 0..5 {
                let res = unsafe { PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
                if res.is_ok() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }

        Ok(())
    }
}

impl KeyboardInterceptor for WindowsInterceptor {
    fn run<F>(&mut self, handler: F) -> Result<(), PlatformError>
    where
        F: FnMut(InputEvent) -> KeyAction + Send + 'static,
    {
        // Force Windows to instantiate the message queue for this worker thread
        let mut init_msg = MSG::default();
        unsafe {
            let _ = PeekMessageW(&mut init_msg, None, 0, 0, PM_NOREMOVE);
        }

        let thread_id = unsafe { GetCurrentThreadId() };
        HOOK_THREAD_ID.store(thread_id, Ordering::SeqCst);

        let hook_handle = unsafe {
            SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(low_level_keyboard_proc),
                None,
                0,
            )
        }
        .map_err(|e| PlatformError::HookInstallationFailed(e.to_string()))?;

        {
            let mut state_guard = HOOK_STATE.lock().unwrap();
            *state_guard = Some(HookState {
                handler: Box::new(handler),
                hook_handle_raw: hook_handle.0 as isize,
            });
        }

        self.is_running = true;

        // Run Windows message pump
        let mut msg = MSG::default();
        loop {
            let ret = unsafe { GetMessageW(&mut msg, None, 0, 0) };
            if ret.0 <= 0 {
                // WM_QUIT or error
                break;
            }
            unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        self.is_running = false;
        let _ = Self::stop_hook();
        Ok(())
    }

    fn stop(&mut self) -> Result<(), PlatformError> {
        self.is_running = false;
        Self::stop_hook()
    }
}

/// Non-blocking message pump for UI events (such as tray icon clicks and global hotkeys).
pub fn pump_messages() {
    unsafe {
        let mut msg = MSG::default();
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// Check if a modifier key (Ctrl or Alt) is currently pressed down.
fn is_control_or_alt_down() -> bool {
    unsafe {
        let ctrl_state = GetKeyState(VK_CONTROL.0 as i32);
        let menu_state = GetKeyState(VK_MENU.0 as i32);
        (ctrl_state & 0x8000u16 as i16) != 0 || (menu_state & 0x8000u16 as i16) != 0
    }
}

/// Translate virtual key to SpecialKey if applicable.
fn map_special_key(vk: VIRTUAL_KEY) -> Option<SpecialKey> {
    match vk {
        VK_BACK => Some(SpecialKey::Backspace),
        VK_RETURN => Some(SpecialKey::Enter),
        VK_ESCAPE => Some(SpecialKey::Escape),
        VK_TAB => Some(SpecialKey::Tab),
        VK_LEFT => Some(SpecialKey::ArrowLeft),
        VK_RIGHT => Some(SpecialKey::ArrowRight),
        VK_UP => Some(SpecialKey::ArrowUp),
        VK_DOWN => Some(SpecialKey::ArrowDown),
        VK_HOME => Some(SpecialKey::Home),
        VK_END => Some(SpecialKey::End),
        VK_PRIOR => Some(SpecialKey::PageUp),
        VK_NEXT => Some(SpecialKey::PageDown),
        VK_DELETE => Some(SpecialKey::Delete),
        _ => None,
    }
}

/// Translate virtual key and scan code to a Unicode character.
fn vk_to_unicode(vk: VIRTUAL_KEY, scan_code: u32) -> Option<char> {
    let mut key_state = [0u8; 256];
    let ok = unsafe { GetKeyboardState(&mut key_state) };
    if ok.is_err() {
        return None;
    }

    let mut buffer = [0u16; 8];
    let hkl = unsafe { GetKeyboardLayout(0) };

    // wFlags = 4 prevents altering dead key status on modern Windows
    let count = unsafe {
        ToUnicodeEx(
            vk.0 as u32,
            scan_code,
            &key_state,
            &mut buffer,
            4,
            hkl,
        )
    };

    if count == 1 {
        char::from_u32(buffer[0] as u32)
    } else {
        None
    }
}

/// Low-level keyboard hook callback procedure.
unsafe extern "system" fn low_level_keyboard_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code < 0 {
        return CallNextHookEx(None, n_code, w_param, l_param);
    }

    let kbd = *(l_param.0 as *const KBDLLHOOKSTRUCT);

    // Infinite loop protection: ignore synthetic events injected by Harfiy
    if kbd.dwExtraInfo == HARFIY_MAGIC_FLAG {
        return CallNextHookEx(None, n_code, w_param, l_param);
    }

    let msg = w_param.0 as u32;
    if msg != WM_KEYDOWN && msg != WM_SYSKEYDOWN {
        return CallNextHookEx(None, n_code, w_param, l_param);
    }

    let vk = VIRTUAL_KEY(kbd.vkCode as u16);

    // Modifier keys (Shift, Ctrl, Alt, Win, CapsLock) are not processed directly
    if matches!(
        vk,
        VK_SHIFT | VK_CONTROL | VK_MENU | VK_LWIN | VK_RWIN | VK_CAPITAL
    ) {
        return CallNextHookEx(None, n_code, w_param, l_param);
    }

    // Do not interfere with Ctrl or Alt combinations (e.g. Ctrl+S, Ctrl+C)
    if is_control_or_alt_down() {
        return CallNextHookEx(None, n_code, w_param, l_param);
    }

    // Determine InputEvent
    let input_event = if let Some(special) = map_special_key(vk) {
        Some(InputEvent::Special(special))
    } else if let Some(ch) = vk_to_unicode(vk, kbd.scanCode) {
        Some(InputEvent::Char(ch))
    } else {
        None
    };

    if let Some(event) = input_event {
        let mut state_guard = HOOK_STATE.lock().unwrap();
        if let Some(state) = state_guard.as_mut() {
            let action = (state.handler)(event);
            match action {
                KeyAction::PassThrough => {
                    return CallNextHookEx(None, n_code, w_param, l_param);
                }
                KeyAction::Suppress => {
                    // Swallow the keystroke
                    return LRESULT(1);
                }
                KeyAction::Transform {
                    backspaces_count,
                    replacement,
                } => {
                    // 1. Emit backspace(s) to erase previous character(s)
                    let mut emitter = WindowsEmitter::new();
                    let _ = emitter.emit_backspace(backspaces_count);
                    // 2. Emit replacement text
                    let _ = emitter.emit_unicode(replacement);
                    // 3. Swallow the current key event so it is not typed
                    return LRESULT(1);
                }
            }
        }
    }

    CallNextHookEx(None, n_code, w_param, l_param)
}
