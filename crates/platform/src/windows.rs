//! Windows implementation of keyboard interceptor and emitter using Win32 API.

use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

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
    GetOpenFileNameW, OFN_EXPLORER, OFN_FILEMUSTEXIST, OFN_PATHMUSTEXIST,
    OPENFILENAMEW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyState, GetKeyboardLayout, GetKeyboardState, SendInput, ToUnicodeEx,
    HKL, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_EXTENDEDKEY,
    KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, VIRTUAL_KEY, VK_BACK, VK_CAPITAL, VK_CONTROL, VK_DELETE,
    VK_DOWN, VK_END, VK_ESCAPE, VK_HOME, VK_LCONTROL, VK_LEFT, VK_LMENU, VK_LSHIFT, VK_LWIN,
    VK_MENU, VK_NEXT, VK_OEM_3, VK_OEM_7, VK_PACKET, VK_PRIOR, VK_RCONTROL, VK_RETURN, VK_RIGHT,
    VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT, VK_TAB, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, EnumWindows,
    GetForegroundWindow, GetMessageW, GetSystemMetrics, GetWindowRect, GetWindowTextLengthW,
    GetWindowThreadProcessId, IsWindowVisible, KillTimer, MessageBoxW, PeekMessageW, PostMessageW,
    PostQuitMessage, PostThreadMessageW, RegisterClassW, SetLayeredWindowAttributes, SetTimer,
    SetWindowsHookExW, ShowWindow, TranslateMessage, UnhookWindowsHookEx, HHOOK, IDYES,
    KBDLLHOOKSTRUCT, LLKHF_EXTENDED, LWA_ALPHA, MB_ICONINFORMATION, MB_ICONQUESTION, MB_OK,
    MB_YESNO, MSG, PM_NOREMOVE, PM_REMOVE, SM_CXSCREEN, SM_CYSCREEN, SW_SHOWNOACTIVATE,
    WH_KEYBOARD_LL, WM_APP, WM_DESTROY, WM_KEYDOWN, WM_PAINT, WM_QUIT, WM_SYSKEYDOWN, WM_TIMER,
    WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

use crate::trigger_buffer::{BufferAction, Held, TriggerBuffer};
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

/// Open the native Windows file dialog to pick one file.
/// `extensions` are lowercase file extensions without the dot, shown as the first filter.
pub fn pick_file_dialog(
    title: &str,
    filter_name: &str,
    extensions: &[&str],
) -> Option<std::path::PathBuf> {
    use windows::core::{HSTRING, PCWSTR, PWSTR};

    // Filter list: "<name>\0*.a;*.b\0<name>\0*.*\0\0"
    let patterns = extensions
        .iter()
        .map(|e| format!("*.{}", e))
        .collect::<Vec<_>>()
        .join(";");
    let mut filter: Vec<u16> = Vec::new();
    for part in [filter_name, patterns.as_str(), "Barcha fayllar (*.*)", "*.*"] {
        filter.extend(part.encode_utf16());
        filter.push(0);
    }
    filter.push(0);

    let title_w = HSTRING::from(title);
    let mut file_buf = vec![0u16; 32 * 1024];

    let mut ofn = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        lpstrFilter: PCWSTR(filter.as_ptr()),
        lpstrFile: PWSTR(file_buf.as_mut_ptr()),
        nMaxFile: file_buf.len() as u32,
        Flags: OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST,
        lpstrTitle: PCWSTR(title_w.as_ptr()),
        ..Default::default()
    };

    if !unsafe { GetOpenFileNameW(&mut ofn) }.as_bool() {
        return None;
    }

    let len = file_buf.iter().position(|&c| c == 0).unwrap_or(file_buf.len());
    let chosen = String::from_utf16_lossy(&file_buf[..len]);
    (!chosen.is_empty()).then(|| std::path::PathBuf::from(chosen))
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

fn open_clipboard_with_retry() -> bool {
    for _ in 0..10 {
        if unsafe { OpenClipboard(None) }.is_ok() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    false
}

/// Read unicode text currently on Windows clipboard.
pub fn get_clipboard_text() -> Option<String> {
    if !open_clipboard_with_retry() {
        return None;
    }
    unsafe {
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

    if !open_clipboard_with_retry() {
        return false;
    }
    unsafe {
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

fn make_key_input(vk: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: HARFIY_MAGIC_FLAG,
            },
        },
    }
}

/// Emit synthetic Ctrl+C key combination to copy selected text.
pub fn emit_copy() {
    // 1. Synthetically release modifier keys that may be held down by the user (e.g. from Ctrl+Alt+U shortcut).
    // If Alt remains held down, Windows treats the combination as Alt+C or Ctrl+Alt+C rather than Ctrl+C,
    // failing to copy text to clipboard.
    let release_inputs = [
        make_key_input(VK_MENU, KEYEVENTF_KEYUP),
        make_key_input(VK_LMENU, KEYEVENTF_KEYUP),
        make_key_input(VK_RMENU, KEYEVENTF_KEYUP),
        make_key_input(VK_SHIFT, KEYEVENTF_KEYUP),
        make_key_input(VK_LWIN, KEYEVENTF_KEYUP),
        make_key_input(VK_RWIN, KEYEVENTF_KEYUP),
    ];
    unsafe {
        SendInput(&release_inputs, std::mem::size_of::<INPUT>() as i32);
    }
    std::thread::sleep(std::time::Duration::from_millis(25));

    // 2. Send clean Ctrl+C
    let copy_inputs = [
        make_key_input(VK_CONTROL, KEYBD_EVENT_FLAGS(0)),
        make_key_input(VIRTUAL_KEY(0x43), KEYBD_EVENT_FLAGS(0)), // 'C'
        make_key_input(VIRTUAL_KEY(0x43), KEYEVENTF_KEYUP),
        make_key_input(VK_CONTROL, KEYEVENTF_KEYUP),
    ];
    unsafe {
        SendInput(&copy_inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

/// Emit synthetic Ctrl+V key combination to paste text.
pub fn emit_paste() {
    // 1. Release Alt / Shift / Win keys before pasting so the target application receives pure Ctrl+V
    let release_inputs = [
        make_key_input(VK_MENU, KEYEVENTF_KEYUP),
        make_key_input(VK_LMENU, KEYEVENTF_KEYUP),
        make_key_input(VK_RMENU, KEYEVENTF_KEYUP),
        make_key_input(VK_SHIFT, KEYEVENTF_KEYUP),
        make_key_input(VK_LWIN, KEYEVENTF_KEYUP),
        make_key_input(VK_RWIN, KEYEVENTF_KEYUP),
    ];
    unsafe {
        SendInput(&release_inputs, std::mem::size_of::<INPUT>() as i32);
    }
    std::thread::sleep(std::time::Duration::from_millis(25));

    // 2. Send clean Ctrl+V
    let paste_inputs = [
        make_key_input(VK_CONTROL, KEYBD_EVENT_FLAGS(0)),
        make_key_input(VIRTUAL_KEY(0x56), KEYBD_EVENT_FLAGS(0)), // 'V'
        make_key_input(VIRTUAL_KEY(0x56), KEYEVENTF_KEYUP),
        make_key_input(VK_CONTROL, KEYEVENTF_KEYUP),
    ];
    unsafe {
        SendInput(&paste_inputs, std::mem::size_of::<INPUT>() as i32);
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

/// Tag for keystrokes the hook types in place of physical ones. Unlike
/// [`HARFIY_MAGIC_FLAG`], the hook counts these back in, so a physical key that
/// overtakes them in the input queue can be re-queued behind them.
const HARFIY_ORDERED_FLAG: usize = 0x48415247; // "HARG"

/// Thread message asking the hook thread to type the held trigger key.
const WM_HARFIY_FLUSH: u32 = WM_APP + 2;

/// Replayed keystrokes that do not come back within this window were dropped
/// (e.g. by another hook) and must stop holding up physical keys.
const IN_FLIGHT_STALE_AFTER: Duration = Duration::from_millis(200);

/// Typed before erasing text, so it replaces an inline autocomplete selection.
/// ZERO WIDTH SPACE is invisible and, unlike real whitespace, does not trigger
/// word-boundary features (AutoCorrect, editor input rules) on the way.
const AUTOCOMPLETE_PLACEHOLDER: &str = "\u{200B}";

/// Physical key data needed to replay a held key exactly as the user typed it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RawKey {
    vk: u16,
    scan: u16,
    extended: bool,
    /// Whether Shift, Ctrl, Alt, Win or CapsLock was in effect when the key was pressed.
    modified: bool,
    /// Foreground window the key was typed into.
    window: isize,
}

static TRIGGER_BUFFER: Mutex<TriggerBuffer<RawKey>> = Mutex::new(TriggerBuffer::new());
/// Thread timer of the hook thread that types a held key once the user pauses.
static TRIGGER_TIMER_ID: AtomicUsize = AtomicUsize::new(0);
static BUFFER_TIMEOUT_MS: AtomicU64 = AtomicU64::new(250);
static TRIGGER_BUFFERING_ENABLED: AtomicBool = AtomicBool::new(true);
static CONVERSION_ACTIVE: AtomicBool = AtomicBool::new(true);
static AUTOCOMPLETE_FIX: AtomicBool = AtomicBool::new(true);
static ENABLE_NG_FLAG: AtomicBool = AtomicBool::new(false);

/// Keystrokes typed by the hook that it has not seen come back yet.
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

/// Tell the hook whether conversion is active (Harfiy is not paused).
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

/// Configure 'ng' digraph recognition for the platform hook.
pub fn set_enable_ng(enabled: bool) {
    ENABLE_NG_FLAG.store(enabled, Ordering::SeqCst);
    // A held key was judged by the previous rule set.
    flush_pending_trigger();
}

fn lock_trigger_buffer() -> MutexGuard<'static, TriggerBuffer<RawKey>> {
    match TRIGGER_BUFFER.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Type any held trigger key immediately.
///
/// Callable from any thread: the work is handed to the hook thread so that it stays
/// ordered with live keystrokes.
pub fn flush_pending_trigger() {
    let thread_id = HOOK_THREAD_ID.load(Ordering::SeqCst);
    if thread_id == 0 {
        // No hook is running, so nothing can be held.
        return;
    }
    if thread_id == unsafe { GetCurrentThreadId() } {
        release_held_key();
    } else {
        unsafe {
            let _ = PostThreadMessageW(thread_id, WM_HARFIY_FLUSH, WPARAM(0), LPARAM(0));
        }
    }
}

/// Type the held trigger key, if any. Runs on the hook thread.
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
fn type_released(out: &mut Injection, held: Held<RawKey>) {
    if !held_in_focus(held) {
        // Typing it now would put it into another window, possibly as a shortcut there
        reset_engine();
        return;
    }
    match feed_engine(InputEvent::Char(held.ch)) {
        KeyAction::PassThrough => out.replay(held),
        KeyAction::Suppress => {}
        KeyAction::Transform {
            backspaces_count,
            replacement,
        } => out.transform(backspaces_count, replacement),
    }
}

/// Arm (or re-arm) the hook thread's timer that types a held key once the user pauses.
fn arm_trigger_timer() {
    let timeout = BUFFER_TIMEOUT_MS
        .load(Ordering::Relaxed)
        .clamp(1, u32::MAX as u64) as u32;
    // Passing the current id re-arms the existing thread timer instead of adding another.
    let current = TRIGGER_TIMER_ID.load(Ordering::Relaxed);
    let id = unsafe { SetTimer(None, current, timeout, None) };
    TRIGGER_TIMER_ID.store(id, Ordering::Relaxed);
}

fn kill_trigger_timer() {
    let id = TRIGGER_TIMER_ID.swap(0, Ordering::Relaxed);
    if id != 0 {
        unsafe {
            let _ = KillTimer(None, id);
        }
    }
}

/// Whether the window a held key was typed into still has the focus.
fn held_in_focus(held: Held<RawKey>) -> bool {
    held.key.window == foreground_window()
}

fn foreground_window() -> isize {
    unsafe { GetForegroundWindow().0 as isize }
}

/// Feed one event to the engine handler installed by `run`.
fn feed_engine(event: InputEvent) -> KeyAction {
    // A panic must not unwind out of the hook callback, so a poisoned lock is reused
    let mut state_guard = match HOOK_STATE.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    match state_guard.as_mut() {
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
    // Only the hook thread touches the counter, so load + store cannot race.
    let n = IN_FLIGHT.load(Ordering::SeqCst);
    if n > 0 {
        IN_FLIGHT.store(n - 1, Ordering::SeqCst);
    }
}

/// Whether the current physical key overtook keystrokes the hook typed earlier,
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

/// Keystrokes the hook types, in order, in place of physical ones.
#[derive(Default)]
struct Injection {
    inputs: Vec<INPUT>,
    /// Index of the first input that must wait until the target application has
    /// processed the Backspaces before it.
    settle_at: Option<usize>,
}

impl Injection {
    fn is_empty(&self) -> bool {
        self.inputs.is_empty()
    }

    fn push(&mut self, vk: u16, scan: u16, flags: KEYBD_EVENT_FLAGS) {
        self.inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(vk),
                    wScan: scan,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: HARFIY_ORDERED_FLAG,
                },
            },
        });
    }

    /// Press a physical key, and release it too when `release` is set.
    fn key(&mut self, key: RawKey, release: bool) {
        if key.vk == VK_PACKET.0 {
            // Unicode input injected by another program carries the character in the scan code.
            self.push(0, key.scan, KEYEVENTF_UNICODE);
            if release {
                self.push(0, key.scan, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP);
            }
            return;
        }
        let flags = if key.extended {
            KEYEVENTF_EXTENDEDKEY
        } else {
            KEYBD_EVENT_FLAGS(0)
        };
        self.push(key.vk, key.scan, flags);
        if release {
            self.push(key.vk, key.scan, flags | KEYEVENTF_KEYUP);
        }
    }

    fn text(&mut self, text: &str) {
        for cu in text.encode_utf16() {
            self.push(0, cu, KEYEVENTF_UNICODE);
            self.push(0, cu, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP);
        }
    }

    /// Type a held key the way the user typed it.
    fn replay(&mut self, held: Held<RawKey>) {
        if held.key.modified || modifiers_in_effect() {
            // Replaying the raw key under other modifiers could produce another
            // character or a shortcut; typing the character itself cannot.
            let mut buf = [0u8; 4];
            self.text(held.ch.encode_utf8(&mut buf));
        } else {
            // A real key press keeps keydown-driven behaviour (shortcuts, autocomplete
            // widgets) working, unlike a Unicode packet.
            self.key(held.key, true);
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
            let backspace = RawKey {
                vk: VK_BACK.0,
                scan: 0x000E,
                extended: false,
                modified: false,
                window: 0,
            };
            for _ in 0..count {
                self.key(backspace, true);
            }
            self.settle_at = Some(self.inputs.len());
        }
        self.text(replacement);
    }

    fn send(self) {
        let split = self.settle_at.unwrap_or(self.inputs.len());
        let (erase, rest) = self.inputs.split_at(split);
        send_ordered(erase);
        if !erase.is_empty() && !rest.is_empty() {
            // Yield / micro-sleep to allow target application (Electron, Chromium, WPF)
            // to process Backspace before receiving replacement unicode characters
            std::thread::sleep(Duration::from_millis(8));
        }
        send_ordered(rest);
    }
}

fn send_ordered(inputs: &[INPUT]) {
    if inputs.is_empty() {
        return;
    }
    let sent = unsafe { SendInput(inputs, std::mem::size_of::<INPUT>() as i32) };
    note_injected(sent);
}

fn is_key_down(vk: VIRTUAL_KEY) -> bool {
    unsafe { (GetAsyncKeyState(vk.0 as i32) as u16 & 0x8000) != 0 }
}

/// Whether Shift, Ctrl, Alt or Win is held, or CapsLock is on.
fn modifiers_in_effect() -> bool {
    is_key_down(VK_SHIFT)
        || is_key_down(VK_CONTROL)
        || is_key_down(VK_MENU)
        || is_key_down(VK_LWIN)
        || is_key_down(VK_RWIN)
        || unsafe { (GetKeyState(VK_CAPITAL.0 as i32) & 0x0001) != 0 }
}

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
                        wScan: 0x000E,
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
                        wScan: 0x000E,
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
        if backspaces_count == 0 && code_units.is_empty() {
            return Ok(());
        }

        // 1. Backspaces (key down + key up) sent first with hardware scan code
        if backspaces_count > 0 {
            let mut bs_inputs = Vec::with_capacity(backspaces_count * 2);
            for _ in 0..backspaces_count {
                bs_inputs.push(INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VK_BACK,
                            wScan: 0x000E,
                            dwFlags: KEYBD_EVENT_FLAGS(0),
                            time: 0,
                            dwExtraInfo: HARFIY_MAGIC_FLAG,
                        },
                    },
                });
                bs_inputs.push(INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VK_BACK,
                            wScan: 0x000E,
                            dwFlags: KEYEVENTF_KEYUP,
                            time: 0,
                            dwExtraInfo: HARFIY_MAGIC_FLAG,
                        },
                    },
                });
            }

            let sent = unsafe { SendInput(&bs_inputs, std::mem::size_of::<INPUT>() as i32) };
            if sent as usize != bs_inputs.len() {
                return Err(PlatformError::SendInputFailed(format!(
                    "Expected to send {} backspace inputs, sent {}",
                    bs_inputs.len(),
                    sent
                )));
            }

            // Yield / micro-sleep to allow target application (Electron, Chromium, WPF)
            // to process Backspace before receiving replacement unicode characters
            std::thread::sleep(std::time::Duration::from_millis(8));
        }

        // 2. Unicode characters (key down + key up)
        if !code_units.is_empty() {
            let mut unicode_inputs = Vec::with_capacity(code_units.len() * 2);
            for cu in code_units {
                unicode_inputs.push(INPUT {
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
                unicode_inputs.push(INPUT {
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

            let sent = unsafe { SendInput(&unicode_inputs, std::mem::size_of::<INPUT>() as i32) };
            if sent as usize != unicode_inputs.len() {
                return Err(PlatformError::SendInputFailed(format!(
                    "Expected to send {} unicode inputs, sent {}",
                    unicode_inputs.len(),
                    sent
                )));
            }
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
        // Handled by the hook thread before the WM_QUIT posted below
        flush_pending_trigger();
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

        lock_trigger_buffer().reset();
        IN_FLIGHT.store(0, Ordering::SeqCst);

        // Run Windows message pump. The trigger timer and flush requests are handled here,
        // on the hook thread, so they never race with the hook callback.
        let mut msg = MSG::default();
        loop {
            let ret = unsafe { GetMessageW(&mut msg, None, 0, 0) };
            if ret.0 <= 0 {
                // WM_QUIT or error
                break;
            }
            if msg.hwnd.0.is_null() {
                let timer_id = TRIGGER_TIMER_ID.load(Ordering::Relaxed);
                let is_trigger_timer =
                    msg.message == WM_TIMER && timer_id != 0 && msg.wParam.0 == timer_id;
                if is_trigger_timer || msg.message == WM_HARFIY_FLUSH {
                    // The user paused (or a flush was requested): type the held key
                    release_held_key();
                    continue;
                }
            }
            unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        // Type a key still held when the hook stops, so it is not lost
        release_held_key();
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

/// Check if a modifier key (Ctrl or Alt) is currently pressed down for a shortcut.
/// AltGr (Right Alt) is excluded because international and custom layouts (including Uzbek)
/// use AltGr to type characters, not shortcuts.
fn is_control_or_alt_down() -> bool {
    unsafe {
        let rmenu_down = (GetKeyState(VK_RMENU.0 as i32) & 0x8000u16 as i16) != 0;
        if rmenu_down {
            // AltGr (Right Alt) is actively used to produce valid characters, do not treat as shortcut
            return false;
        }

        let lctrl = (GetKeyState(VK_LCONTROL.0 as i32) & 0x8000u16 as i16) != 0;
        let rctrl = (GetKeyState(VK_RCONTROL.0 as i32) & 0x8000u16 as i16) != 0;
        let lmenu = (GetKeyState(VK_LMENU.0 as i32) & 0x8000u16 as i16) != 0;

        lctrl || rctrl || lmenu
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

/// Translate virtual key and scan code to a Unicode character across all keyboard layouts.
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

    // Fallback 1: Extract device layout from HIWORD of hkl (e.g. 0x04090443 for Uzbek Latin in Windows).
    // In Windows, when input language is Uzbek Latin (0x0443), the composite layout handle
    // is 0x04090443, which is not registered in user32's internal HKL table of background processes,
    // causing ToUnicodeEx to return 0. Extracting the layout HIWORD (0x0409) resolves this completely!
    if count == 0 {
        let hi = (hkl.0 as usize >> 16) & 0xFFFF;
        if hi != 0 {
            let sub_hkl = HKL(((hi << 16) | hi) as *mut _);
            count = unsafe { ToUnicodeEx(vk.0 as u32, scan_code, &key_state, &mut buffer, 4, sub_hkl) };
            if count == 0 {
                count = unsafe { ToUnicodeEx(vk.0 as u32, scan_code, &key_state, &mut buffer, 0, sub_hkl) };
            }
        }
    }

    // Fallback 2: Thread's own keyboard layout
    if count == 0 {
        let thread_hkl = unsafe { GetKeyboardLayout(0) };
        if thread_hkl != hkl {
            count = unsafe { ToUnicodeEx(vk.0 as u32, scan_code, &key_state, &mut buffer, 4, thread_hkl) };
            if count == 0 {
                count = unsafe { ToUnicodeEx(vk.0 as u32, scan_code, &key_state, &mut buffer, 0, thread_hkl) };
            }
        }
    }

    // Fallback 3: Standard US QWERTY layout (0x04090409)
    if count == 0 {
        let us_hkl = HKL(0x04090409 as *mut _);
        count = unsafe { ToUnicodeEx(vk.0 as u32, scan_code, &key_state, &mut buffer, 4, us_hkl) };
        if count == 0 {
            count = unsafe { ToUnicodeEx(vk.0 as u32, scan_code, &key_state, &mut buffer, 0, us_hkl) };
        }
    }

    // Handle normal characters (count == 1) and dead keys (count == -1, e.g. apostrophe / accents on international layouts)
    if count == 1 || count == -1 {
        char::from_u32(buffer[0] as u32)
    } else if count > 1 {
        // Multi-character sequence (e.g. combined dead key + base letter)
        char::from_u32(buffer[(count - 1) as usize] as u32)
    } else {
        // Ultimate fallback for physical apostrophe / backtick keys if unmapped in active layout
        match vk {
            VK_OEM_7 => {
                if (key_state[VK_SHIFT.0 as usize] & 0x80) != 0 {
                    Some('"')
                } else {
                    Some('\'')
                }
            }
            VK_OEM_3 => {
                if (key_state[VK_SHIFT.0 as usize] & 0x80) != 0 {
                    Some('~')
                } else {
                    Some('`')
                }
            }
            _ => None,
        }
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

    // Keystrokes typed by the hook coming back through it: count them in
    if kbd.dwExtraInfo == HARFIY_ORDERED_FLAG {
        note_injection_arrived();
        return CallNextHookEx(None, n_code, w_param, l_param);
    }

    // Infinite loop protection: ignore synthetic events injected by Harfiy
    if kbd.dwExtraInfo == HARFIY_MAGIC_FLAG {
        return CallNextHookEx(None, n_code, w_param, l_param);
    }

    let msg = w_param.0 as u32;
    if msg != WM_KEYDOWN && msg != WM_SYSKEYDOWN {
        return CallNextHookEx(None, n_code, w_param, l_param);
    }

    let vk = VIRTUAL_KEY(kbd.vkCode as u16);
    let key = RawKey {
        vk: vk.0,
        scan: kbd.scanCode as u16,
        extended: (kbd.flags.0 & LLKHF_EXTENDED.0) != 0,
        modified: modifiers_in_effect(),
        window: foreground_window(),
    };

    let mut out = Injection::default();
    let forward = handle_key_down(vk, kbd.scanCode, key, &mut out);

    // Keystrokes injected from inside the hook arrive after the current key, so once
    // anything is queued the current key is replayed behind it instead of passed on.
    if forward && out.is_empty() && !overtakes_injection() {
        return CallNextHookEx(None, n_code, w_param, l_param);
    }
    if forward {
        out.key(key, false);
    }
    out.send();
    LRESULT(1)
}

/// Decide what happens to a physical key press. Keystrokes to type go into `out`;
/// returns whether the key itself should still reach the application, after `out`.
fn handle_key_down(vk: VIRTUAL_KEY, scan_code: u32, key: RawKey, out: &mut Injection) -> bool {
    match vk {
        // Shift and CapsLock only change the next character, which may still complete a pair
        VK_SHIFT | VK_LSHIFT | VK_RSHIFT | VK_CAPITAL => return true,
        // AltGr types characters on many layouts; it also sends a fake LCtrl with scan code 0x21D
        VK_RMENU => return true,
        VK_LCONTROL if scan_code & 0x200 != 0 => return true,
        // Ctrl, Alt and Win start a shortcut: type the held key before the shortcut applies
        VK_CONTROL | VK_LCONTROL | VK_RCONTROL | VK_MENU | VK_LMENU | VK_LWIN | VK_RWIN => {
            release_held_into(out);
            return true;
        }
        _ => {}
    }

    // Do not interfere with shortcuts (e.g. Ctrl+S, Win+S), excluded applications, or
    // fullscreen games while Game Mode is active. What such a key does to the text is
    // unknown, so the engine starts over.
    if is_control_or_alt_down()
        || is_key_down(VK_LWIN)
        || is_key_down(VK_RWIN)
        || is_current_app_excluded()
        || is_foreground_fullscreen()
    {
        release_held_into(out);
        forget_context();
        return true;
    }

    let event = match map_special_key(vk) {
        Some(special) => InputEvent::Special(special),
        None => match vk_to_unicode(vk, scan_code) {
            Some(ch) => InputEvent::Char(ch),
            None => {
                release_held_into(out);
                forget_context();
                return true;
            }
        },
    };

    // Smart Trigger Buffering (eliminates autocomplete conflicts and visual flicker)
    if !TRIGGER_BUFFERING_ENABLED.load(Ordering::Relaxed)
        || !CONVERSION_ACTIVE.load(Ordering::Relaxed)
    {
        release_held_into(out);
        return apply_engine(event, out);
    }

    let (held_before, step) = {
        let mut buffer = lock_trigger_buffer();
        buffer.set_enable_ng(ENABLE_NG_FLAG.load(Ordering::Relaxed));
        let held_before = buffer.held();
        let step = match event {
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
        BufferAction::Pass => apply_engine(event, out),
        BufferAction::Hold => {
            // Swallow the trigger so the application (and its autocomplete) does not see it yet
            arm_trigger_timer();
            false
        }
        BufferAction::Convert { .. } => {
            kill_trigger_timer();
            match held_before {
                Some(held) => convert_held_pair(held, event, out),
                None => apply_engine(event, out),
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
fn convert_held_pair(held: Held<RawKey>, event: InputEvent, out: &mut Injection) -> bool {
    if !held_in_focus(held) {
        reset_engine();
        return apply_engine(event, out);
    }

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
