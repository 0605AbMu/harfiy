//! Windows implementation of keyboard interceptor and emitter using Win32 API.

use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use harfiy_engine::{InputEvent, KeyAction, SpecialKey};
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, BOOL, COLORREF, HANDLE, HWND, LPARAM, LRESULT, RECT, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreateRoundRectRgn, CreateSolidBrush, DeleteObject, DrawTextW,
    EndPaint, FillRect, InvalidateRect, SelectObject, SetBkMode, SetTextColor, SetWindowRgn,
    DT_CENTER, DT_SINGLELINE, DT_VCENTER, HBRUSH, HGDIOBJ, PAINTSTRUCT, TRANSPARENT,
};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SZ,
};
use windows::Win32::System::Threading::{
    CreateMutexW, GetCurrentThreadId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, OFN_EXPLORER, OFN_FILEMUSTEXIST, OFN_PATHMUSTEXIST, OPENFILENAMEW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, GetKeyboardLayout, GetKeyboardState, SendInput, ToUnicodeEx, INPUT, INPUT_0,
    INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, VIRTUAL_KEY,
    VK_BACK, VK_CAPITAL, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_HOME, VK_LEFT,
    VK_LWIN, VK_MENU, VK_NEXT, VK_PRIOR, VK_RETURN, VK_RIGHT, VK_RWIN, VK_SHIFT, VK_TAB, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, EnumWindows,
    GetForegroundWindow, GetMessageW, GetSystemMetrics, GetWindowRect, GetWindowTextLengthW,
    GetWindowThreadProcessId, IsWindowVisible, KillTimer, MessageBoxW, PeekMessageW, PostMessageW,
    PostQuitMessage, PostThreadMessageW, RegisterClassW, SetLayeredWindowAttributes, SetTimer,
    SetWindowsHookExW, ShowWindow, TranslateMessage, UnhookWindowsHookEx, HHOOK, IDYES,
    KBDLLHOOKSTRUCT, LWA_ALPHA, MB_ICONINFORMATION, MB_ICONQUESTION, MB_OK, MB_YESNO, MSG,
    PM_NOREMOVE, PM_REMOVE, SM_CXSCREEN, SM_CYSCREEN, SW_SHOWNOACTIVATE, WH_KEYBOARD_LL, WM_APP,
    WM_DESTROY, WM_KEYDOWN, WM_PAINT, WM_QUIT, WM_SYSKEYDOWN, WM_TIMER, WNDCLASSW, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use crate::{KeyboardEmitter, KeyboardInterceptor, PlatformError};

/// Magic tag attached to `dwExtraInfo` in synthetic keystrokes to prevent infinite loops.
/// ASCII value for "HARF".
pub const HARFIY_MAGIC_FLAG: usize = 0x48415246;

extern "system" {
    fn AttachConsole(dw_process_id: u32) -> i32;
    fn GetModuleHandleA(lpModuleName: *const u8) -> usize;
    fn GetProcAddress(hModule: usize, lpProcName: *const u8) -> usize;
    fn SetProcessDPIAware() -> BOOL;
}

/// Initialize high-DPI awareness on Windows (Per-Monitor V2 if available, or System DPI).
pub fn init_dpi_awareness() {
    unsafe {
        let user32 = GetModuleHandleA(b"user32.dll\0".as_ptr());
        if user32 != 0 {
            let proc = GetProcAddress(user32, b"SetProcessDpiAwarenessContext\0".as_ptr());
            if proc != 0 {
                let func: unsafe extern "system" fn(isize) -> BOOL = std::mem::transmute(proc);
                // DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2 is -4
                if func(-4).as_bool() {
                    return;
                }
            }
        }
        let _ = SetProcessDPIAware();
    }
}

/// Attach to the parent console if invoked from a terminal (e.g. for CLI commands).
pub fn attach_parent_console() {
    unsafe {
        AttachConsole(0xFFFFFFFF);
    }
}

/// Named Mutex guard for ensuring only one instance of Harfiy runs at a time.
pub struct SingleInstanceGuard {
    handle: HANDLE,
}

// Safety: The Win32 mutex handle is thread-safe and can be sent across threads.
unsafe impl Send for SingleInstanceGuard {}
unsafe impl Sync for SingleInstanceGuard {}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(std::env::temp_dir().join("harfiy.pid"));
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
        let _ = std::fs::write(
            std::env::temp_dir().join("harfiy.pid"),
            std::process::id().to_string(),
        );
        Some(SingleInstanceGuard { handle })
    }
}

/// Retrieve the PID of the active running daemon instance if present.
pub fn get_running_daemon_pid() -> Option<u32> {
    let pid_path = std::env::temp_dir().join("harfiy.pid");
    let content = std::fs::read_to_string(&pid_path).ok()?;
    let pid: u32 = content.trim().parse().ok()?;

    unsafe {
        if let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            let mut exit_code = 0u32;
            let res = windows::Win32::System::Threading::GetExitCodeProcess(handle, &mut exit_code);
            let _ = CloseHandle(handle);
            if res.is_ok() && exit_code == 259 {
                // STILL_ACTIVE (259)
                return Some(pid);
            }
        }
    }
    let _ = std::fs::remove_file(&pid_path);
    None
}

/// Terminate the active running daemon instance on Windows.
pub fn stop_running_daemon() -> Result<bool, PlatformError> {
    if let Some(pid) = get_running_daemon_pid() {
        unsafe {
            if let Ok(handle) = OpenProcess(
                windows::Win32::System::Threading::PROCESS_TERMINATE,
                false,
                pid,
            ) {
                let _ = windows::Win32::System::Threading::TerminateProcess(handle, 0);
                let _ = CloseHandle(handle);
            }
        }
        let _ = std::fs::remove_file(std::env::temp_dir().join("harfiy.pid"));
        return Ok(true);
    }
    Ok(false)
}

/// Show a native informational message dialog.
pub fn show_info_dialog(title: &str, message: &str) {
    use windows::core::HSTRING;
    let t = HSTRING::from(title);
    let m = HSTRING::from(message);
    unsafe {
        let _ = MessageBoxW(
            None,
            windows::core::PCWSTR(m.as_ptr()),
            windows::core::PCWSTR(t.as_ptr()),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

/// Show a native confirmation dialog (Yes / No).
pub fn show_confirm_dialog(title: &str, message: &str) -> bool {
    use windows::core::HSTRING;
    let t = HSTRING::from(title);
    let m = HSTRING::from(message);
    unsafe {
        let res = MessageBoxW(
            None,
            windows::core::PCWSTR(m.as_ptr()),
            windows::core::PCWSTR(t.as_ptr()),
            MB_YESNO | MB_ICONQUESTION,
        );
        res == IDYES
    }
}

static HUD_HWND: AtomicIsize = AtomicIsize::new(0);
static HUD_ACTIVE: AtomicBool = AtomicBool::new(true);
static HUD_CUSTOM_MESSAGE: Mutex<Option<(String, bool)>> = Mutex::new(None);

const WM_HUD_UPDATE: u32 = WM_APP + 1;

/// Show or update a modern, native floating status HUD indicator on Windows.
pub fn show_status_hud(active: bool) {
    *HUD_CUSTOM_MESSAGE.lock().unwrap() = None;
    HUD_ACTIVE.store(active, Ordering::SeqCst);
    let hwnd_raw = HUD_HWND.load(Ordering::SeqCst);
    if hwnd_raw != 0 {
        unsafe {
            let _ = PostMessageW(
                HWND(hwnd_raw as *mut _),
                WM_HUD_UPDATE,
                WPARAM(if active { 1 } else { 0 }),
                LPARAM(0),
            );
        }
    } else {
        std::thread::spawn(move || {
            run_hud_window();
        });
    }
}

/// Show a custom floating notification HUD with green/gray indicator dot.
pub fn show_custom_hud(message: &str, is_success: bool) {
    *HUD_CUSTOM_MESSAGE.lock().unwrap() = Some((message.to_string(), is_success));
    let hwnd_raw = HUD_HWND.load(Ordering::SeqCst);
    if hwnd_raw != 0 {
        unsafe {
            let _ = PostMessageW(
                HWND(hwnd_raw as *mut _),
                WM_HUD_UPDATE,
                WPARAM(if is_success { 1 } else { 0 }),
                LPARAM(0),
            );
        }
    } else {
        std::thread::spawn(move || {
            run_hud_window();
        });
    }
}

fn run_hud_window() {
    use windows::core::w;

    unsafe {
        let class_name = w!("HarfiyStatusHUD");
        let wnd_class = WNDCLASSW {
            lpfnWndProc: Some(hud_wnd_proc),
            lpszClassName: class_name,
            hbrBackground: HBRUSH(std::ptr::null_mut()),
            ..Default::default()
        };

        let _ = RegisterClassW(&wnd_class);

        let width = 280;
        let height = 44;
        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let x = (screen_w - width) / 2;
        let y = 60; // Top-center floating position

        let hwnd = match CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED,
            class_name,
            w!("Harfiy HUD"),
            WS_POPUP,
            x,
            y,
            width,
            height,
            None,
            None,
            None,
            None,
        ) {
            Ok(h) => h,
            Err(_) => return,
        };

        HUD_HWND.store(hwnd.0 as isize, Ordering::SeqCst);

        // Apply smooth rounded pill region (radius 22px)
        let rgn = CreateRoundRectRgn(0, 0, width + 1, height + 1, 22, 22);
        let _ = SetWindowRgn(hwnd, rgn, true);

        // Modern semi-transparent glass style (~94% opacity)
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 240, LWA_ALPHA);

        // Display without stealing focus from active application
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);

        // Auto-close timer after 1100ms
        let _ = SetTimer(hwnd, 1, 1100, None);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        HUD_HWND.store(0, Ordering::SeqCst);
    }
}

unsafe extern "system" fn hud_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_HUD_UPDATE => {
            let _ = SetTimer(hwnd, 1, 1100, None);
            let _ = InvalidateRect(hwnd, None, true);
            LRESULT(0)
        }
        WM_TIMER => {
            let _ = KillTimer(hwnd, 1);
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);

            // Dark sleek slate background: RGB(28, 30, 36) -> 0x00241E1C
            let bg_brush = CreateSolidBrush(COLORREF(0x00241E1C));
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: 280,
                bottom: 44,
            };
            FillRect(hdc, &rect, bg_brush);
            let _ = DeleteObject(HGDIOBJ(bg_brush.0 as *mut _));

            // Setup transparent text drawing
            SetBkMode(hdc, TRANSPARENT);

            // Create modern Segoe UI font
            use windows::core::w;
            let font = CreateFontW(
                -16,
                0,
                0,
                0,
                600, // FW_SEMIBOLD
                0,
                0,
                0,
                1, // DEFAULT_CHARSET
                0,
                0,
                0,
                0,
                w!("Segoe UI"),
            );
            let old_font = SelectObject(hdc, HGDIOBJ(font.0 as *mut _));

            let custom_opt = HUD_CUSTOM_MESSAGE.lock().unwrap().clone();
            let (indicator, status_text, color) = if let Some((msg, ok)) = custom_opt {
                if ok {
                    ("●", msg, COLORREF(0x005EC522))
                } else {
                    ("●", msg, COLORREF(0x00AFA39C))
                }
            } else {
                let active = HUD_ACTIVE.load(Ordering::SeqCst);
                if active {
                    ("●", "Harfiy: Faol".to_string(), COLORREF(0x005EC522))
                } else {
                    ("●", "Harfiy: To'xtatildi".to_string(), COLORREF(0x00AFA39C))
                }
            };

            let full_text = format!("{}  {}", indicator, status_text);
            let mut text_wide: Vec<u16> =
                full_text.encode_utf16().chain(std::iter::once(0)).collect();

            SetTextColor(hdc, color);
            DrawTextW(
                hdc,
                &mut text_wide,
                &mut rect,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );

            SelectObject(hdc, old_font);
            let _ = DeleteObject(HGDIOBJ(font.0 as *mut _));

            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_DESTROY => {
            HUD_HWND.store(0, Ordering::SeqCst);
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

static EXCLUDED_APPS: Mutex<Vec<String>> = Mutex::new(Vec::new());
static CACHED_FG_STATUS: Mutex<Option<(isize, bool, Instant)>> = Mutex::new(None);

/// Update the active list of excluded applications.
pub fn set_excluded_apps(apps: &[String]) {
    let mut list = EXCLUDED_APPS.lock().unwrap();
    *list = apps.iter().map(|s| s.to_lowercase()).collect();
    let mut cache = CACHED_FG_STATUS.lock().unwrap();
    *cache = None;
}

/// Check if the currently focused foreground window belongs to an excluded application.
pub fn is_current_app_excluded() -> bool {
    let fg = unsafe { GetForegroundWindow() };
    if fg.0.is_null() {
        return false;
    }
    let fg_raw = fg.0 as isize;

    {
        let cache = CACHED_FG_STATUS.lock().unwrap();
        if let Some((cached_hwnd, excluded, instant)) = *cache {
            if cached_hwnd == fg_raw && instant.elapsed() < std::time::Duration::from_millis(200) {
                return excluded;
            }
        }
    }

    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(fg, Some(&mut pid)) };
    let mut is_excluded = false;
    if pid != 0 && pid != std::process::id() {
        if let Some(exe) = get_process_name_by_pid(pid) {
            let exe_lower = exe.to_lowercase();
            let guard = EXCLUDED_APPS.lock().unwrap();
            is_excluded = guard.iter().any(|app| app == &exe_lower);
        }
    }

    let mut cache = CACHED_FG_STATUS.lock().unwrap();
    *cache = Some((fg_raw, is_excluded, Instant::now()));

    is_excluded
}

/// Retrieve the executable file name for a process ID.
pub fn get_process_name_by_pid(pid: u32) -> Option<String> {
    use windows::core::PWSTR;

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = [0u16; 1024];
        let mut size = buffer.len() as u32;
        let res = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(handle);

        if res.is_ok() && size > 0 {
            let path_str = String::from_utf16_lossy(&buffer[..size as usize]);
            let path = std::path::Path::new(&path_str);
            return path
                .file_name()
                .and_then(|f| f.to_str())
                .map(|s| s.to_string());
        }
    }
    None
}

/// Scan for active running GUI desktop applications with open windows.
pub fn scan_running_apps() -> Vec<String> {
    let mut apps: Vec<String> = Vec::new();
    let apps_ptr = &mut apps as *mut Vec<String>;

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        unsafe {
            if !IsWindowVisible(hwnd).as_bool() {
                return BOOL(1);
            }
            let len = GetWindowTextLengthW(hwnd);
            if len == 0 {
                return BOOL(1);
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == 0 || pid == std::process::id() {
                return BOOL(1);
            }
            if let Some(exe_name) = get_process_name_by_pid(pid) {
                let exe_lower = exe_name.to_lowercase();
                if !matches!(
                    exe_lower.as_str(),
                    "harfiy.exe"
                        | "shellexperiencehost.exe"
                        | "searchhost.exe"
                        | "startmenuexperiencehost.exe"
                        | "textinputhost.exe"
                        | "applicationframehost.exe"
                        | "systemsettings.exe"
                        | "explorer.exe"
                ) {
                    let apps = &mut *(lparam.0 as *mut Vec<String>);
                    if !apps.contains(&exe_lower) {
                        apps.push(exe_lower);
                    }
                }
            }
            BOOL(1)
        }
    }

    unsafe {
        let _ = EnumWindows(Some(enum_proc), LPARAM(apps_ptr as isize));
    }

    apps.sort();
    apps
}

/// Open native Windows file selection dialog (Explorer) to pick an executable file (.exe).
pub fn pick_executable_dialog() -> Option<String> {
    use windows::core::{w, PWSTR};

    let mut file_buf = [0u16; 1024];
    let filter = w!("Dasturlar (*.exe)\0*.exe\0Barcha fayllar (*.*)\0*.*\0\0");

    let mut ofn = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        lpstrFilter: filter,
        lpstrFile: PWSTR(file_buf.as_mut_ptr()),
        nMaxFile: file_buf.len() as u32,
        Flags: OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST,
        lpstrTitle: w!("Harfiy - Istisno dasturni tanlang (.exe)"),
        ..Default::default()
    };

    unsafe {
        if GetOpenFileNameW(&mut ofn).as_bool() {
            let len = file_buf
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(file_buf.len());
            let path_str = String::from_utf16_lossy(&file_buf[..len]);
            let path = std::path::Path::new(&path_str);
            if let Some(file_name) = path.file_name().and_then(|f| f.to_str()) {
                return Some(file_name.to_lowercase());
            }
        }
    }
    None
}

const AUTOSTART_SUBKEY: windows::core::PCWSTR =
    windows::core::w!(r"Software\Microsoft\Windows\CurrentVersion\Run");
const AUTOSTART_NAME: windows::core::PCWSTR = windows::core::w!("Harfiy");

/// Check if Harfiy is configured to run on Windows startup in Registry.
pub fn is_autostart_enabled() -> bool {
    unsafe {
        let mut hkey = HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, AUTOSTART_SUBKEY, 0, KEY_READ, &mut hkey).is_ok() {
            let mut buf_len = 0u32;
            let res = RegQueryValueExW(hkey, AUTOSTART_NAME, None, None, None, Some(&mut buf_len));
            let _ = RegCloseKey(hkey);
            res.is_ok() && buf_len > 0
        } else {
            false
        }
    }
}

/// Enable or disable running Harfiy on Windows startup via Registry.
pub fn set_autostart(enabled: bool) -> Result<(), PlatformError> {
    unsafe {
        let mut hkey = HKEY::default();
        let open_res = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            AUTOSTART_SUBKEY,
            0,
            KEY_WRITE | KEY_READ,
            &mut hkey,
        );
        if !open_res.is_ok() {
            return Err(PlatformError::HookInstallationFailed(format!(
                "Failed to open registry key: {:?}",
                open_res
            )));
        }

        if enabled {
            let exe_path = std::env::current_exe()
                .map_err(|e| PlatformError::HookInstallationFailed(e.to_string()))?;
            let cmd = format!("\"{}\" start", exe_path.display());
            let wide: Vec<u16> = cmd.encode_utf16().chain(std::iter::once(0)).collect();
            let slice = std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2);
            let res = RegSetValueExW(hkey, AUTOSTART_NAME, 0, REG_SZ, Some(slice));
            let _ = RegCloseKey(hkey);
            if !res.is_ok() {
                return Err(PlatformError::HookInstallationFailed(format!(
                    "Failed to set registry value: {:?}",
                    res
                )));
            }
        } else {
            let _ = RegDeleteValueW(hkey, AUTOSTART_NAME);
            let _ = RegCloseKey(hkey);
        }
        Ok(())
    }
}

const CF_UNICODETEXT: u32 = 13;

/// Read unicode text currently on Windows clipboard.
pub fn get_clipboard_text() -> Option<String> {
    unsafe {
        if OpenClipboard(None).is_err() {
            return None;
        }
        let handle = match GetClipboardData(CF_UNICODETEXT) {
            Ok(h) => h,
            Err(_) => {
                let _ = CloseClipboard();
                return None;
            }
        };
        let ptr = GlobalLock(windows::Win32::Foundation::HGLOBAL(handle.0));
        if ptr.is_null() {
            let _ = CloseClipboard();
            return None;
        }

        let slice = std::slice::from_raw_parts(ptr as *const u16, 1_000_000);
        let len = slice.iter().position(|&c| c == 0).unwrap_or(0);
        let text = String::from_utf16_lossy(&slice[..len]);

        let _ = GlobalUnlock(windows::Win32::Foundation::HGLOBAL(handle.0));
        let _ = CloseClipboard();
        Some(text)
    }
}

/// Write unicode text to Windows clipboard.
pub fn set_clipboard_text(text: &str) -> bool {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes_count = wide.len() * 2;

    unsafe {
        if OpenClipboard(None).is_err() {
            return false;
        }
        let _ = EmptyClipboard();

        let hmem = match GlobalAlloc(GMEM_MOVEABLE, bytes_count) {
            Ok(h) => h,
            Err(_) => {
                let _ = CloseClipboard();
                return false;
            }
        };

        let ptr = GlobalLock(hmem);
        if ptr.is_null() {
            let _ = CloseClipboard();
            return false;
        }

        std::ptr::copy_nonoverlapping(wide.as_ptr() as *const u8, ptr as *mut u8, bytes_count);
        let _ = GlobalUnlock(hmem);

        let res = SetClipboardData(CF_UNICODETEXT, HANDLE(hmem.0));
        let _ = CloseClipboard();
        res.is_ok()
    }
}

/// Emit synthetic Ctrl+C key combination to copy selected text.
pub fn emit_copy() {
    let inputs = [
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_CONTROL,
                    wScan: 0,
                    dwFlags: KEYBD_EVENT_FLAGS(0),
                    time: 0,
                    dwExtraInfo: HARFIY_MAGIC_FLAG,
                },
            },
        },
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0x43), // 'C'
                    wScan: 0,
                    dwFlags: KEYBD_EVENT_FLAGS(0),
                    time: 0,
                    dwExtraInfo: HARFIY_MAGIC_FLAG,
                },
            },
        },
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0x43),
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: HARFIY_MAGIC_FLAG,
                },
            },
        },
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_CONTROL,
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: HARFIY_MAGIC_FLAG,
                },
            },
        },
    ];
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

/// Emit synthetic Ctrl+V key combination to paste text.
pub fn emit_paste() {
    let inputs = [
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_CONTROL,
                    wScan: 0,
                    dwFlags: KEYBD_EVENT_FLAGS(0),
                    time: 0,
                    dwExtraInfo: HARFIY_MAGIC_FLAG,
                },
            },
        },
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0x56), // 'V'
                    wScan: 0,
                    dwFlags: KEYBD_EVENT_FLAGS(0),
                    time: 0,
                    dwExtraInfo: HARFIY_MAGIC_FLAG,
                },
            },
        },
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0x56),
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: HARFIY_MAGIC_FLAG,
                },
            },
        },
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_CONTROL,
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: HARFIY_MAGIC_FLAG,
                },
            },
        },
    ];
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

static GAME_MODE_ENABLED: AtomicBool = AtomicBool::new(true);

/// Enable or disable automatic game mode (fullscreen game detection).
pub fn set_game_mode(enabled: bool) {
    GAME_MODE_ENABLED.store(enabled, Ordering::SeqCst);
}

/// Query if game mode is currently enabled.
pub fn is_game_mode() -> bool {
    GAME_MODE_ENABLED.load(Ordering::SeqCst)
}

/// Check if the currently focused window is a full-screen application / game.
pub fn is_foreground_fullscreen() -> bool {
    if !is_game_mode() {
        return false;
    }
    unsafe {
        let fg = GetForegroundWindow();
        if fg.0.is_null() {
            return false;
        }

        let mut rect = RECT::default();
        if GetWindowRect(fg, &mut rect).is_err() {
            return false;
        }

        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let screen_h = GetSystemMetrics(SM_CYSCREEN);

        if rect.left <= 0 && rect.top <= 0 && rect.right >= screen_w && rect.bottom >= screen_h {
            let mut pid = 0u32;
            GetWindowThreadProcessId(fg, Some(&mut pid));
            if pid != 0 && pid != std::process::id() {
                if let Some(name) = get_process_name_by_pid(pid) {
                    let name_lower = name.to_lowercase();
                    if name_lower != "explorer.exe" && name_lower != "harfiy.exe" {
                        return true;
                    }
                }
            }
        }
        false
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

        let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };

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

        let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };

        if sent as usize != inputs.len() {
            return Err(PlatformError::SendInputFailed(format!(
                "Expected to send {} inputs, sent {}",
                inputs.len(),
                sent
            )));
        }

        Ok(())
    }

    fn emit_transform(
        &mut self,
        backspaces_count: usize,
        replacement: &str,
    ) -> Result<(), PlatformError> {
        let code_units: Vec<u16> = replacement.encode_utf16().collect();
        let total_inputs = backspaces_count * 2 + code_units.len() * 2;
        if total_inputs == 0 {
            return Ok(());
        }

        let mut inputs = Vec::with_capacity(total_inputs);

        // 1. Backspaces (key down + key up)
        for _ in 0..backspaces_count {
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

        // 2. Unicode characters (key down + key up)
        for cu in code_units {
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

        // Send all synthetic inputs in one single atomic batch
        let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
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

        let hook_handle =
            unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(low_level_keyboard_proc), None, 0) }
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

    // Synchronize critical modifier keys directly via GetKeyState,
    // because WH_KEYBOARD_LL runs on Harfiy's background worker thread
    // where GetKeyboardState may not reflect the active window thread's state.
    unsafe {
        if (GetKeyState(VK_SHIFT.0 as i32) & 0x8000u16 as i16) != 0 {
            key_state[VK_SHIFT.0 as usize] = 0x80;
        }
        if (GetKeyState(VK_CAPITAL.0 as i32) & 0x0001) != 0 {
            key_state[VK_CAPITAL.0 as usize] = 0x01;
        }
        if (GetKeyState(VK_CONTROL.0 as i32) & 0x8000u16 as i16) != 0 {
            key_state[VK_CONTROL.0 as usize] = 0x80;
        }
        if (GetKeyState(VK_MENU.0 as i32) & 0x8000u16 as i16) != 0 {
            key_state[VK_MENU.0 as usize] = 0x80;
        }
    }

    let mut buffer = [0u16; 8];

    // Query keyboard layout of the currently focused active window thread.
    // Harfiy's background thread layout does not update when the user switches input language.
    let hkl = unsafe {
        let fg = GetForegroundWindow();
        if !fg.0.is_null() {
            let thread_id = GetWindowThreadProcessId(fg, None);
            GetKeyboardLayout(thread_id)
        } else {
            GetKeyboardLayout(0)
        }
    };

    // wFlags = 4 prevents altering dead key status on Windows 10 v1607 and newer.
    // On older Windows (e.g. Windows 10 LTSB 2015 / Build 10240, Windows 7/8), passing 4
    // causes ToUnicodeEx to return 0. We gracefully fallback to wFlags = 0.
    let mut count = unsafe { ToUnicodeEx(vk.0 as u32, scan_code, &key_state, &mut buffer, 4, hkl) };
    if count == 0 {
        count = unsafe { ToUnicodeEx(vk.0 as u32, scan_code, &key_state, &mut buffer, 0, hkl) };
    }

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

    // Do not interfere if the active foreground window belongs to an excluded application
    if is_current_app_excluded() {
        return CallNextHookEx(None, n_code, w_param, l_param);
    }

    // Do not interfere if Game Mode is active and foreground window is a fullscreen application / game
    if is_foreground_fullscreen() {
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
                    // Emit backspace(s) and replacement text atomically in one batch
                    let mut emitter = WindowsEmitter::new();
                    let _ = emitter.emit_transform(backspaces_count, replacement);
                    // Swallow the current key event so it is not typed
                    return LRESULT(1);
                }
            }
        }
    }

    CallNextHookEx(None, n_code, w_param, l_param)
}
