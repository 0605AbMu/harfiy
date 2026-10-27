//! Windows dialogs for saving converted files, and a progress window for long operations.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use windows::core::{w, HSTRING, PCWSTR, PWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW, CreateRoundRectRgn,
    CreateSolidBrush, DeleteDC, DeleteObject, DrawTextW, EndPaint, FillRect, GetDC, GetDeviceCaps,
    GetStockObject, InvalidateRect, ReleaseDC, RoundRect, ScreenToClient, SelectObject, SetBkMode,
    SetTextColor, SetWindowRgn, DRAW_TEXT_FORMAT, DT_CENTER, DT_END_ELLIPSIS, DT_LEFT,
    DT_SINGLELINE, DT_VCENTER, HBRUSH, HDC, HFONT, HGDIOBJ, LOGPIXELSX, NULL_PEN, PAINTSTRUCT,
    SRCCOPY, TRANSPARENT,
};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::Controls::Dialogs::{
    GetSaveFileNameW, OFN_EXPLORER, OFN_HIDEREADONLY, OFN_NOCHANGEDIR, OFN_OVERWRITEPROMPT,
    OFN_PATHMUSTEXIST, OPENFILENAMEW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetCursorPos,
    GetForegroundWindow, GetMessageW, GetSystemMetrics, GetWindowLongPtrW, GetWindowThreadProcessId,
    KillTimer, LoadCursorW, PostMessageW, PostQuitMessage, RegisterClassW,
    SetForegroundWindow, SetLayeredWindowAttributes, SetTimer, SetWindowLongPtrW, ShowWindow,
    TranslateMessage, GWLP_USERDATA, IDC_ARROW, LWA_ALPHA, MSG, SM_CXSCREEN, SW_SHOWNOACTIVATE,
    WM_CLOSE, WM_DESTROY, WM_ERASEBKGND, WM_LBUTTONDOWN, WM_PAINT, WM_TIMER, WNDCLASSW,
    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, WS_VISIBLE,
};

// ---------------------------------------------------------------------------------------------
// Dialogs
// ---------------------------------------------------------------------------------------------

/// A tiny off-screen, always-on-top window that owns a dialog so the dialog opens in front of
/// every other window, even when the process has not had the focus for a while (for example
/// after a long conversion).
struct DialogOwner(Option<HWND>);

impl DialogOwner {
    fn new() -> Self {
        unsafe {
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                w!("STATIC"),
                w!(""),
                WS_POPUP | WS_VISIBLE,
                -32000,
                -32000,
                1,
                1,
                None,
                None,
                None,
                None,
            )
            .ok();
            if let Some(hwnd) = hwnd {
                bring_to_front(hwnd);
            }
            DialogOwner(hwnd)
        }
    }

    fn hwnd(&self) -> HWND {
        self.0.unwrap_or_default()
    }
}

impl Drop for DialogOwner {
    fn drop(&mut self) {
        if let Some(hwnd) = self.0 {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
        }
    }
}

/// Windows refuses to hand the foreground to a process that has had no recent input. Briefly
/// sharing the input queue of the current foreground window is the standard way around that.
unsafe fn bring_to_front(hwnd: HWND) {
    let foreground = GetForegroundWindow();
    let foreground_thread = GetWindowThreadProcessId(foreground, None);
    let this_thread = GetCurrentThreadId();
    let attached = foreground_thread != 0
        && foreground_thread != this_thread
        && AttachThreadInput(this_thread, foreground_thread, true).as_bool();
    let _ = SetForegroundWindow(hwnd);
    if attached {
        let _ = AttachThreadInput(this_thread, foreground_thread, false);
    }
}

fn wide_buffer(initial: &str, len: usize) -> Vec<u16> {
    let mut buffer = vec![0u16; len];
    for (slot, unit) in buffer.iter_mut().zip(initial.encode_utf16()).take(len - 1) {
        *slot = unit;
    }
    buffer
}

fn path_from_wide(buffer: &[u16]) -> String {
    let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..len])
}

/// Open the native Save As dialog, pre-filled with `default_name` in `initial_dir`.
/// Asks before overwriting. Returns `None` if the user cancels.
pub fn pick_save_file_dialog(
    title: &str,
    initial_dir: Option<&Path>,
    default_name: &str,
) -> Option<PathBuf> {
    let extension = Path::new(default_name)
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default();

    // Filter list: "<name>\0*.ext\0<name>\0*.*\0\0"
    let mut filter: Vec<u16> = Vec::new();
    let parts: [String; 4] = [
        format!("{} fayllari (*.{})", extension.to_uppercase(), extension),
        format!("*.{}", extension),
        "Barcha fayllar (*.*)".to_string(),
        "*.*".to_string(),
    ];
    for part in &parts {
        filter.extend(part.encode_utf16());
        filter.push(0);
    }
    filter.push(0);

    let title_w = HSTRING::from(title);
    let extension_w = HSTRING::from(extension.as_str());
    let initial_dir_w = initial_dir.map(|d| HSTRING::from(d.as_os_str()));
    let mut file_buf = wide_buffer(default_name, 32 * 1024);

    let owner = DialogOwner::new();
    let mut ofn = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: owner.hwnd(),
        lpstrFilter: PCWSTR(filter.as_ptr()),
        lpstrFile: PWSTR(file_buf.as_mut_ptr()),
        nMaxFile: file_buf.len() as u32,
        lpstrInitialDir: initial_dir_w
            .as_ref()
            .map_or(PCWSTR::null(), |d| PCWSTR(d.as_ptr())),
        lpstrTitle: PCWSTR(title_w.as_ptr()),
        lpstrDefExt: if extension.is_empty() {
            PCWSTR::null()
        } else {
            PCWSTR(extension_w.as_ptr())
        },
        Flags: OFN_EXPLORER
            | OFN_OVERWRITEPROMPT
            | OFN_PATHMUSTEXIST
            | OFN_NOCHANGEDIR
            | OFN_HIDEREADONLY,
        ..Default::default()
    };

    if !unsafe { GetSaveFileNameW(&mut ofn) }.as_bool() {
        return None;
    }
    let chosen = path_from_wide(&file_buf);
    (!chosen.is_empty()).then(|| PathBuf::from(chosen))
}

// ---------------------------------------------------------------------------------------------
// Progress window
// ---------------------------------------------------------------------------------------------

const TIMER_ID: usize = 2;
const SHOW_TIMER_ID: usize = 3;
const FRAME_MS: u32 = 40;

// Logical size at 96 DPI.
const WIDTH: i32 = 360;
const HEIGHT: i32 = 122;

// Colors are 0x00BBGGRR.
const COLOR_BG: u32 = 0x0024_1E1C; // (28, 30, 36)
const COLOR_TRACK: u32 = 0x0042_3A37; // (55, 58, 66)
const COLOR_GREEN: u32 = 0x005E_C522; // (34, 197, 94)
const COLOR_GREEN_LIGHT: u32 = 0x00AC_EF86; // (134, 239, 172)
const COLOR_TEXT: u32 = 0x00F6_F4F3; // (243, 244, 246)
const COLOR_MUTED: u32 = 0x00AF_A39C; // (156, 163, 175)
const COLOR_BUTTON: u32 = 0x0042_3A37;
const COLOR_BUTTON_HOVER: u32 = 0x005A_4F4B; // (75, 79, 90)

struct ProgressState {
    title: String,
    status: String,
    detail: String,
    /// `None` means the amount of work is unknown (an animated bar is shown).
    fraction: Option<f32>,
}

struct ProgressShared {
    state: Mutex<ProgressState>,
    hwnd: AtomicIsize,
    cancelled: AtomicBool,
    frame: AtomicU32,
    /// DPI scale in thousandths (1000 = 100%).
    scale_milli: AtomicU32,
    /// The window stays hidden for this long, so quick operations never flash it.
    show_delay_ms: u32,
}

/// A small always-on-top progress window with an animated bar and a cancel button.
///
/// It runs on its own thread and never takes the keyboard focus. Dropping it closes it.
pub struct ProgressWindow {
    shared: Arc<ProgressShared>,
    thread: Option<JoinHandle<()>>,
}

impl ProgressWindow {
    /// Show the window right away.
    pub fn open(title: &str) -> Self {
        Self::open_after(title, Duration::ZERO)
    }

    /// Create the window but only show it once `delay` has passed. If it is dropped before
    /// that (the operation was quick), it never appears.
    pub fn open_after(title: &str, delay: Duration) -> Self {
        let shared = Arc::new(ProgressShared {
            state: Mutex::new(ProgressState {
                title: title.to_string(),
                status: String::new(),
                detail: String::new(),
                fraction: None,
            }),
            hwnd: AtomicIsize::new(0),
            cancelled: AtomicBool::new(false),
            frame: AtomicU32::new(0),
            scale_milli: AtomicU32::new(1000),
            show_delay_ms: delay.as_millis().min(u32::MAX as u128) as u32,
        });

        let (ready_tx, ready_rx) = mpsc::channel();
        let thread_shared = Arc::clone(&shared);
        let thread = std::thread::spawn(move || run_progress_window(thread_shared, ready_tx));
        let _ = ready_rx.recv_timeout(Duration::from_secs(3));

        Self {
            shared,
            thread: Some(thread),
        }
    }

    /// Set what the window shows. `fraction` is 0..=1, or `None` while the amount of work is
    /// unknown.
    pub fn update(&self, fraction: Option<f32>, status: &str, detail: &str) {
        if let Ok(mut state) = self.shared.state.lock() {
            state.fraction = fraction.map(|f| f.clamp(0.0, 1.0));
            state.status = status.to_string();
            state.detail = detail.to_string();
        }
    }

    /// Whether the user pressed "Bekor qilish".
    pub fn is_cancelled(&self) -> bool {
        self.shared.cancelled.load(Ordering::SeqCst)
    }
}

impl Drop for ProgressWindow {
    fn drop(&mut self) {
        let raw = self.shared.hwnd.load(Ordering::SeqCst);
        if raw != 0 {
            unsafe {
                let _ = PostMessageW(HWND(raw as *mut _), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn scaled(value: i32, scale: f32) -> i32 {
    (value as f32 * scale).round() as i32
}

fn run_progress_window(shared: Arc<ProgressShared>, ready: mpsc::Sender<()>) {
    unsafe {
        let class_name = w!("HarfiyProgress");
        let wnd_class = WNDCLASSW {
            lpfnWndProc: Some(progress_wnd_proc),
            lpszClassName: class_name,
            hbrBackground: HBRUSH(std::ptr::null_mut()),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        let _ = RegisterClassW(&wnd_class);

        let screen_dc = GetDC(None);
        let dpi = GetDeviceCaps(screen_dc, LOGPIXELSX).max(96);
        ReleaseDC(None, screen_dc);
        let scale = dpi as f32 / 96.0;
        shared
            .scale_milli
            .store((scale * 1000.0) as u32, Ordering::SeqCst);

        let width = scaled(WIDTH, scale);
        let height = scaled(HEIGHT, scale);
        let x = (GetSystemMetrics(SM_CXSCREEN) - width) / 2;
        let y = scaled(110, scale); // below the status HUD

        let hwnd = match CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED,
            class_name,
            w!("Harfiy"),
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
            Ok(hwnd) => hwnd,
            Err(_) => {
                let _ = ready.send(());
                return;
            }
        };

        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Arc::as_ptr(&shared) as isize);
        shared.hwnd.store(hwnd.0 as isize, Ordering::SeqCst);

        let region = CreateRoundRectRgn(0, 0, width + 1, height + 1, scaled(20, scale), scaled(20, scale));
        let _ = SetWindowRgn(hwnd, region, true);
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 245, LWA_ALPHA);
        if shared.show_delay_ms == 0 {
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            let _ = SetTimer(hwnd, TIMER_ID, FRAME_MS, None);
        } else {
            let _ = SetTimer(hwnd, SHOW_TIMER_ID, shared.show_delay_ms, None);
        }
        let _ = ready.send(());

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        shared.hwnd.store(0, Ordering::SeqCst);
    }
}

/// The cancel button, in client coordinates.
fn cancel_button_rect(scale: f32) -> RECT {
    RECT {
        left: scaled(228, scale),
        top: scaled(86, scale),
        right: scaled(340, scale),
        bottom: scaled(110, scale),
    }
}

fn point_in_rect(x: i32, y: i32, rect: &RECT) -> bool {
    x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom
}

unsafe extern "system" fn progress_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let shared_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const ProgressShared;
    // The window thread owns an `Arc` for as long as the window exists, so the pointer is valid.
    let shared = shared_ptr.as_ref();

    match msg {
        WM_TIMER if wparam.0 == SHOW_TIMER_ID => {
            let _ = KillTimer(hwnd, SHOW_TIMER_ID);
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            let _ = SetTimer(hwnd, TIMER_ID, FRAME_MS, None);
            LRESULT(0)
        }
        WM_TIMER => {
            if let Some(shared) = shared {
                shared.frame.fetch_add(1, Ordering::Relaxed);
            }
            let _ = InvalidateRect(hwnd, None, false);
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            if let Some(shared) = shared {
                let scale = shared.scale_milli.load(Ordering::SeqCst) as f32 / 1000.0;
                let x = (lparam.0 & 0xFFFF) as u16 as i16 as i32;
                let y = ((lparam.0 >> 16) & 0xFFFF) as u16 as i16 as i32;
                if point_in_rect(x, y, &cancel_button_rect(scale)) {
                    shared.cancelled.store(true, Ordering::SeqCst);
                }
            }
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            if let Some(shared) = shared {
                paint(hwnd, hdc, shared);
            }
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_DESTROY => {
            let _ = KillTimer(hwnd, TIMER_ID);
            let _ = KillTimer(hwnd, SHOW_TIMER_ID);
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn make_font(height: i32, weight: i32) -> HFONT {
    CreateFontW(
        -height,
        0,
        0,
        0,
        weight,
        0,
        0,
        0,
        1, // DEFAULT_CHARSET
        0,
        0,
        0,
        0,
        w!("Segoe UI"),
    )
}

unsafe fn draw_text(hdc: HDC, text: &str, rect: RECT, color: u32, font: HFONT, flags: DRAW_TEXT_FORMAT) {
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    if wide.is_empty() {
        return;
    }
    let mut rect = rect;
    let old_font = SelectObject(hdc, HGDIOBJ(font.0));
    SetTextColor(hdc, COLORREF(color));
    DrawTextW(hdc, &mut wide, &mut rect, flags);
    SelectObject(hdc, old_font);
}

unsafe fn fill_round(hdc: HDC, rect: RECT, color: u32, radius: i32) {
    let brush = CreateSolidBrush(COLORREF(color));
    let old_brush = SelectObject(hdc, HGDIOBJ(brush.0));
    let old_pen = SelectObject(hdc, GetStockObject(NULL_PEN));
    let _ = RoundRect(
        hdc,
        rect.left,
        rect.top,
        rect.right + 1,
        rect.bottom + 1,
        radius,
        radius,
    );
    SelectObject(hdc, old_pen);
    SelectObject(hdc, old_brush);
    let _ = DeleteObject(HGDIOBJ(brush.0));
}

/// Draw the whole window into an off-screen bitmap, then copy it over in one go (no flicker).
unsafe fn paint(hwnd: HWND, window_dc: HDC, shared: &ProgressShared) {
    let scale = shared.scale_milli.load(Ordering::SeqCst) as f32 / 1000.0;
    let s = |v: i32| scaled(v, scale);
    let (width, height) = (s(WIDTH), s(HEIGHT));

    let hdc = CreateCompatibleDC(window_dc);
    let bitmap = CreateCompatibleBitmap(window_dc, width, height);
    let old_bitmap = SelectObject(hdc, HGDIOBJ(bitmap.0));

    let background = CreateSolidBrush(COLORREF(COLOR_BG));
    let full = RECT {
        left: 0,
        top: 0,
        right: width,
        bottom: height,
    };
    FillRect(hdc, &full, background);
    let _ = DeleteObject(HGDIOBJ(background.0));
    SetBkMode(hdc, TRANSPARENT);

    let (title, status, detail, fraction) = match shared.state.lock() {
        Ok(state) => (
            state.title.clone(),
            state.status.clone(),
            state.detail.clone(),
            state.fraction,
        ),
        Err(_) => (String::new(), String::new(), String::new(), None),
    };
    let frame = shared.frame.load(Ordering::Relaxed);
    let cancelled = shared.cancelled.load(Ordering::SeqCst);

    let small = make_font(s(13), 400);
    let medium = make_font(s(15), 600);
    let single = DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS;

    draw_text(
        hdc,
        &title,
        RECT { left: s(20), top: s(12), right: s(340), bottom: s(32) },
        COLOR_MUTED,
        small,
        single,
    );
    draw_text(
        hdc,
        &status,
        RECT { left: s(20), top: s(34), right: s(340), bottom: s(60) },
        COLOR_TEXT,
        medium,
        single,
    );

    // Progress bar
    let track = RECT { left: s(20), top: s(68), right: s(340), bottom: s(76) };
    let radius = s(8);
    fill_round(hdc, track, COLOR_TRACK, radius);
    let track_width = track.right - track.left;
    let phase = (frame % 50) as f32 / 50.0;
    match fraction {
        Some(f) => {
            let fill_width = ((track_width as f32) * f).round() as i32;
            if fill_width > 0 {
                let fill = RECT { right: track.left + fill_width.max(s(8)), ..track };
                fill_round(hdc, fill, COLOR_GREEN, radius);
                // A light band sweeps across the filled part so the bar is visibly alive.
                let band_width = s(48);
                let band_left = fill.left + (((fill.right - fill.left + band_width) as f32) * phase) as i32 - band_width;
                let band = RECT {
                    left: band_left.max(fill.left + s(4)),
                    right: (band_left + band_width).min(fill.right - s(4)),
                    ..fill
                };
                if band.right > band.left {
                    fill_round(hdc, band, COLOR_GREEN_LIGHT, radius);
                }
            }
        }
        None => {
            // Unknown amount of work: a segment travels back and forth.
            let segment = track_width / 3;
            let ping_pong = if phase < 0.5 { phase * 2.0 } else { (1.0 - phase) * 2.0 };
            let left = track.left + (((track_width - segment) as f32) * ping_pong) as i32;
            fill_round(hdc, RECT { left, right: left + segment, ..track }, COLOR_GREEN, radius);
        }
    }

    // Percentage / counters on the left
    let percent = fraction.map(|f| format!("{}%", (f * 100.0).round() as i32));
    let detail_line = match (detail.is_empty(), percent) {
        (true, Some(p)) => p,
        (false, Some(p)) => format!("{}  ·  {}", detail, p),
        (_, None) => detail,
    };
    draw_text(
        hdc,
        &detail_line,
        RECT { left: s(20), top: s(86), right: s(220), bottom: s(110) },
        COLOR_MUTED,
        small,
        single,
    );

    // Cancel button (hover is computed from the cursor each frame)
    let button = cancel_button_rect(scale);
    let mut cursor = POINT::default();
    let _ = GetCursorPos(&mut cursor);
    let _ = ScreenToClient(hwnd, &mut cursor);
    let hovered = !cancelled && point_in_rect(cursor.x, cursor.y, &button);
    fill_round(
        hdc,
        button,
        if hovered { COLOR_BUTTON_HOVER } else { COLOR_BUTTON },
        s(10),
    );
    draw_text(
        hdc,
        if cancelled { "Bekor qilinmoqda..." } else { "Bekor qilish" },
        button,
        if cancelled { COLOR_MUTED } else { COLOR_TEXT },
        small,
        DT_CENTER | DT_SINGLELINE | DT_VCENTER,
    );

    let _ = DeleteObject(HGDIOBJ(small.0));
    let _ = DeleteObject(HGDIOBJ(medium.0));

    let _ = BitBlt(window_dc, 0, 0, width, height, hdc, 0, 0, SRCCOPY);
    SelectObject(hdc, old_bitmap);
    let _ = DeleteObject(HGDIOBJ(bitmap.0));
    let _ = DeleteDC(hdc);
}
