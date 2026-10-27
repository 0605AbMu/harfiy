//! OS-level logging module for Harfiy.
//!
//! Provides direct integration with OS event logging facilities:
//! - Windows: Windows Event Viewer (`Application` event log via Win32 `ReportEventW`)
//! - Linux / macOS: System logger (`syslog` / Unified Logging via `libc::syslog`)
//! - Fallback: In-memory ring buffer and standard error stream
//!
//! Also implements the `log::Log` trait to capture all standard `log` crate records
//! (`log::error!`, `log::warn!`, `log::info!`) and automatically routes unhandled
//! process panics to the OS log.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::SystemTime;

/// Severity level for OS-level event records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
}

impl From<log::Level> for LogLevel {
    fn from(level: log::Level) -> Self {
        match level {
            log::Level::Error => LogLevel::Error,
            log::Level::Warn => LogLevel::Warn,
            log::Level::Info => LogLevel::Info,
            log::Level::Debug | log::Level::Trace => LogLevel::Debug,
        }
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Error => write!(f, "ERROR"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Debug => write!(f, "DEBUG"),
        }
    }
}

/// A stored in-memory log entry for inspection and debugging.
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: SystemTime,
    pub level: LogLevel,
    pub target: String,
    pub message: String,
}

static LOGGER_INITIALIZED: AtomicBool = AtomicBool::new(false);
static RECENT_LOGS: Mutex<Vec<LogEntry>> = Mutex::new(Vec::new());
const MAX_RECENT_LOGS: usize = 100;

fn record_recent_log(level: LogLevel, target: &str, message: &str) {
    if let Ok(mut logs) = RECENT_LOGS.lock() {
        if logs.len() >= MAX_RECENT_LOGS {
            logs.remove(0);
        }
        logs.push(LogEntry {
            timestamp: SystemTime::now(),
            level,
            target: target.to_string(),
            message: message.to_string(),
        });
    }
}

/// Retrieve a copy of recent logs recorded in memory.
pub fn get_recent_logs() -> Vec<LogEntry> {
    if let Ok(logs) = RECENT_LOGS.lock() {
        logs.clone()
    } else {
        Vec::new()
    }
}

/// Clear in-memory log history.
pub fn clear_recent_logs() {
    if let Ok(mut logs) = RECENT_LOGS.lock() {
        logs.clear();
    }
}

/// Send a record directly to the operating system's native logging facility.
pub fn log_to_os(level: LogLevel, tag: &str, message: &str) {
    record_recent_log(level, tag, message);
    log_to_os_impl(level, tag, message);
}

#[cfg(windows)]
fn log_to_os_impl(level: LogLevel, tag: &str, message: &str) {
    use windows::core::{w, PCWSTR};
    use windows::Win32::System::EventLog::*;

    let (event_type, event_id) = match level {
        LogLevel::Error => (EVENTLOG_ERROR_TYPE, 1001),
        LogLevel::Warn => (EVENTLOG_WARNING_TYPE, 1002),
        LogLevel::Info => (EVENTLOG_INFORMATION_TYPE, 1003),
        LogLevel::Debug => (EVENTLOG_INFORMATION_TYPE, 1004),
    };

    unsafe {
        if let Ok(handle) = RegisterEventSourceW(None, w!("Harfiy")) {
            let formatted = if tag.is_empty() || tag == "Harfiy" {
                message.to_string()
            } else {
                format!("[{}] {}", tag, message)
            };
            let wide: Vec<u16> = formatted.encode_utf16().chain(std::iter::once(0)).collect();
            let strings = [PCWSTR(wide.as_ptr())];
            let _ = ReportEventW(
                handle,
                event_type,
                0,
                event_id,
                None,
                0,
                Some(&strings),
                None,
            );
            let _ = DeregisterEventSource(handle);
        }
    }
}

#[cfg(windows)]
/// Best-effort registration of Harfiy in Windows Event Log registry.
///
/// Configures `HKLM\SYSTEM\CurrentControlSet\Services\EventLog\Application\Harfiy`
/// so Event Viewer natively resolves messages without warnings.
/// Fails gracefully if process does not have administrator privileges.
pub fn register_windows_event_source() -> Result<(), String> {
    use windows::core::w;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegSetValueExW, HKEY, HKEY_LOCAL_MACHINE, KEY_WRITE,
        REG_DWORD, REG_EXPAND_SZ, REG_OPTION_NON_VOLATILE,
    };

    unsafe {
        let subkey = w!("SYSTEM\\CurrentControlSet\\Services\\EventLog\\Application\\Harfiy");
        let mut hkey = HKEY::default();
        let status = RegCreateKeyExW(
            HKEY_LOCAL_MACHINE,
            subkey,
            0,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut hkey,
            None,
        );

        if status.is_err() {
            return Err(format!("RegCreateKeyExW error: {:?}", status));
        }

        let types: u32 = 7; // Error (1) | Warning (2) | Information (4)
        let _ = RegSetValueExW(
            hkey,
            w!("TypesSupported"),
            0,
            REG_DWORD,
            Some(std::slice::from_raw_parts(
                &types as *const u32 as *const u8,
                std::mem::size_of::<u32>(),
            )),
        );

        let msg_file = w!("%SystemRoot%\\System32\\netmsg.dll");
        let msg_bytes = std::slice::from_raw_parts(
            msg_file.as_ptr() as *const u8,
            (msg_file.as_wide().len() + 1) * 2,
        );
        let _ = RegSetValueExW(
            hkey,
            w!("EventMessageFile"),
            0,
            REG_EXPAND_SZ,
            Some(msg_bytes),
        );

        let _ = RegCloseKey(hkey);
        Ok(())
    }
}

#[cfg(not(windows))]
pub fn register_windows_event_source() -> Result<(), String> {
    Ok(())
}

#[cfg(unix)]
fn log_to_os_impl(level: LogLevel, tag: &str, message: &str) {
    use std::ffi::CString;

    let priority = match level {
        LogLevel::Error => libc::LOG_ERR,
        LogLevel::Warn => libc::LOG_WARNING,
        LogLevel::Info => libc::LOG_INFO,
        LogLevel::Debug => libc::LOG_DEBUG,
    };

    let formatted = if tag.is_empty() || tag == "Harfiy" {
        format!("Harfiy: {}\0", message)
    } else {
        format!("Harfiy [{}]: {}\0", tag, message)
    };

    if let Ok(c_str) = CString::new(formatted.trim_end_matches('\0')) {
        unsafe {
            libc::syslog(priority, b"%s\0".as_ptr() as *const libc::c_char, c_str.as_ptr());
        }
    }
}

#[cfg(not(any(windows, unix)))]
fn log_to_os_impl(level: LogLevel, tag: &str, message: &str) {
    eprintln!("[OS-LOG] [{:?}] [{}] {}", level, tag, message);
}

/// An implementation of `log::Log` that dispatches records to OS facilities.
pub struct OsLogger {
    max_level: log::LevelFilter,
}

impl log::Log for OsLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= self.max_level
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let level: LogLevel = record.level().into();
        let target = record.target();
        let message = format!("{}", record.args());

        // Echo to stderr for terminal/console output
        eprintln!("[{}] [{}] {}", record.level(), target, message);

        // Forward to OS logging facility (Windows Event Viewer / syslog)
        log_to_os(level, target, &message);
    }

    fn flush(&self) {}
}

/// Initialize the global OS-level logger for Harfiy.
///
/// Automatically hooks into standard `log` macros (`log::error!`, etc.)
/// and configures a panic hook to capture crash details in the OS event viewer.
pub fn init_os_logger(max_level: log::LevelFilter) -> Result<(), log::SetLoggerError> {
    if LOGGER_INITIALIZED.swap(true, Ordering::SeqCst) {
        return Ok(());
    }

    let logger = OsLogger { max_level };
    log::set_boxed_logger(Box::new(logger))?;
    log::set_max_level(max_level);

    #[cfg(windows)]
    {
        let _ = register_windows_event_source();
    }

    // Capture unhandled panics and forward to OS event log
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let payload = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Noma'lum xatolik (Unknown panic payload)".to_string()
        };

        let location = panic_info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "noma'lum joylashuv".to_string());

        let err_msg = format!(
            "Harfiy xizmati to'satdan to'xtadi (Panic): {} [{}]",
            payload, location
        );

        eprintln!("\n🚨 {}", err_msg);
        log_to_os(LogLevel::Error, "Harfiy::Panic", &err_msg);

        default_hook(panic_info);
    }));

    Ok(())
}

/// Send an error message to the OS event log.
pub fn log_error(message: &str) {
    if LOGGER_INITIALIZED.load(Ordering::SeqCst) {
        log::error!("{}", message);
    } else {
        eprintln!("[ERROR] {}", message);
        log_to_os(LogLevel::Error, "Harfiy", message);
    }
}

/// Send a warning message to the OS event log.
pub fn log_warn(message: &str) {
    if LOGGER_INITIALIZED.load(Ordering::SeqCst) {
        log::warn!("{}", message);
    } else {
        eprintln!("[WARN] {}", message);
        log_to_os(LogLevel::Warn, "Harfiy", message);
    }
}

/// Send an informational message to the OS event log.
pub fn log_info(message: &str) {
    if LOGGER_INITIALIZED.load(Ordering::SeqCst) {
        log::info!("{}", message);
    } else {
        eprintln!("[INFO] {}", message);
        log_to_os(LogLevel::Info, "Harfiy", message);
    }
}

/// A parsed OS log record representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OsLogRecord {
    pub time: String,
    pub level: String,
    pub event_id: u32,
    pub message: String,
}

/// Query recent event logs emitted by Harfiy to the operating system.
pub fn query_os_logs(limit: usize) -> Vec<OsLogRecord> {
    query_os_logs_impl(limit)
}

#[cfg(windows)]
fn query_os_logs_impl(limit: usize) -> Vec<OsLogRecord> {
    use std::process::Command;

    let xpath = "*[System[Provider[@Name='Harfiy']]]";
    let output = Command::new("wevtutil")
        .args([
            "qe",
            "Application",
            &format!("/q:{}", xpath),
            &format!("/c:{}", limit),
            "/rd:true",
            "/f:xml",
        ])
        .output();

    let mut records = Vec::new();
    if let Ok(out) = output {
        if out.status.success() {
            let xml_text = String::from_utf8_lossy(&out.stdout);
            for event_str in xml_text.split("</Event>") {
                if !event_str.contains("<Event") {
                    continue;
                }
                let event_id = event_str
                    .split("<EventID")
                    .nth(1)
                    .and_then(|s| s.split('>').nth(1))
                    .and_then(|s| s.split('<').next())
                    .and_then(|s| s.trim().parse::<u32>().ok())
                    .unwrap_or(0);

                let level_num = event_str
                    .split("<Level>")
                    .nth(1)
                    .and_then(|s| s.split("</Level>").next())
                    .and_then(|s| s.trim().parse::<u32>().ok())
                    .unwrap_or(4);

                let level = match level_num {
                    1 | 2 => "ERROR",
                    3 => "WARN",
                    _ => "INFO",
                };

                let time = event_str
                    .split("SystemTime='")
                    .nth(1)
                    .and_then(|s| s.split('\'').next())
                    .unwrap_or("")
                    .to_string();

                let message = event_str
                    .split("<Data>")
                    .nth(1)
                    .and_then(|s| s.split("</Data>").next())
                    .unwrap_or("")
                    .to_string();

                if !message.is_empty() || event_id != 0 {
                    records.push(OsLogRecord {
                        time,
                        level: level.to_string(),
                        event_id,
                        message,
                    });
                }
            }
        }
    }

    if records.is_empty() {
        for entry in get_recent_logs().into_iter().rev().take(limit) {
            records.push(OsLogRecord {
                time: "recent".to_string(),
                level: entry.level.to_string(),
                event_id: match entry.level {
                    LogLevel::Error => 1001,
                    LogLevel::Warn => 1002,
                    LogLevel::Info => 1003,
                    LogLevel::Debug => 1004,
                },
                message: if entry.target.is_empty() || entry.target == "Harfiy" {
                    entry.message
                } else {
                    format!("[{}] {}", entry.target, entry.message)
                },
            });
        }
    }

    records
}

#[cfg(not(windows))]
fn query_os_logs_impl(limit: usize) -> Vec<OsLogRecord> {
    get_recent_logs()
        .into_iter()
        .rev()
        .take(limit)
        .map(|entry| OsLogRecord {
            time: "recent".to_string(),
            level: entry.level.to_string(),
            event_id: match entry.level {
                LogLevel::Error => 1001,
                LogLevel::Warn => 1002,
                LogLevel::Info => 1003,
                LogLevel::Debug => 1004,
            },
            message: format!("[{}] {}", entry.target, entry.message),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_ordering_and_display() {
        assert!(LogLevel::Error < LogLevel::Warn);
        assert!(LogLevel::Warn < LogLevel::Info);
        assert_eq!(LogLevel::Error.to_string(), "ERROR");
        assert_eq!(LogLevel::Warn.to_string(), "WARN");
        assert_eq!(LogLevel::Info.to_string(), "INFO");
        assert_eq!(LogLevel::Debug.to_string(), "DEBUG");
    }

    #[test]
    fn test_record_and_get_recent_logs() {
        let unique_tag = "UniqueTestTag_12345";
        log_to_os(LogLevel::Error, unique_tag, "Xatolik ro'y berdi");
        log_to_os(LogLevel::Info, unique_tag, "Xizmat boshlandi");

        let entries = get_recent_logs();
        let filtered: Vec<_> = entries.into_iter().filter(|e| e.target == unique_tag).collect();
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].level, LogLevel::Error);
        assert_eq!(filtered[0].message, "Xatolik ro'y berdi");
        assert_eq!(filtered[1].level, LogLevel::Info);
        assert_eq!(filtered[1].message, "Xizmat boshlandi");
    }

    #[cfg(windows)]
    #[test]
    fn test_windows_event_log_emission() {
        // Test that emitting an error directly to Windows Event Viewer does not panic
        log_to_os(
            LogLevel::Error,
            "HarfiyTest",
            "Harfiy error log testi (Windows Event Log)",
        );
    }

    #[test]
    fn test_query_os_logs_retrieval() {
        let unique_msg = "Harfiy query log unit test verification";
        log_to_os(LogLevel::Error, "UnitTestQuery", unique_msg);
        let records = query_os_logs(10);
        assert!(!records.is_empty());
        assert!(records.iter().any(|r| r.message.contains(unique_msg)));
    }
}
