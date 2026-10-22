//! Harfiy platform abstraction layer.
//!
//! Provides traits and OS-specific implementations for:
//! - Keyboard interception (low-level hooks, event taps, uinput)
//! - Synthetic key emission (SendInput, CGEventPost, uinput virtual device)

pub mod mock;

#[cfg(windows)]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub mod linux;

use harfiy_engine::{InputEvent, KeyAction};

/// Platform errors encountered during interception or emission.
#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("Failed to install hook: {0}")]
    HookInstallationFailed(String),

    #[error("Failed to uninstall hook: {0}")]
    HookUninstallFailed(String),

    #[error("Failed to send synthetic input: {0}")]
    SendInputFailed(String),

    #[error("Accessibility or permissions required: {0}")]
    PermissionDenied(String),

    #[error("Platform not supported: {0}")]
    NotSupported(String),

    #[error("General platform error: {0}")]
    Other(String),
}

/// Abstract keyboard event interceptor.
pub trait KeyboardInterceptor {
    /// Start listening for events and pass them to the handler closure.
    /// This method blocks (e.g. running a message loop) until `stop()` is called.
    fn run<F>(&mut self, handler: F) -> Result<(), PlatformError>
    where
        F: FnMut(InputEvent) -> KeyAction + Send + 'static;

    /// Stop the interceptor and unhook from the OS event stream.
    fn stop(&mut self) -> Result<(), PlatformError>;
}

/// Abstract synthetic keyboard event emitter.
pub trait KeyboardEmitter: Send + Sync {
    /// Emit one or more Backspace key events.
    fn emit_backspace(&mut self, count: usize) -> Result<(), PlatformError>;

    /// Emit Unicode text as synthetic keyboard input.
    fn emit_unicode(&mut self, text: &str) -> Result<(), PlatformError>;
}

pub use mock::{MockEmittedEvent, MockEmitter, MockInterceptor, MockPlatform};

#[cfg(windows)]
pub use self::windows::{
    acquire_single_instance_lock, pump_messages, SingleInstanceGuard, WindowsEmitter,
    WindowsInterceptor, HARFIY_MAGIC_FLAG,
};

#[cfg(target_os = "macos")]
pub use self::macos::{MacOsEmitter, MacOsInterceptor};

#[cfg(target_os = "linux")]
pub use self::linux::{LinuxEmitter, LinuxInterceptor};

#[cfg(unix)]
pub struct SingleInstanceGuard {
    _file: std::fs::File,
}

#[cfg(unix)]
pub fn acquire_single_instance_lock() -> Option<SingleInstanceGuard> {
    use std::os::unix::io::AsRawFd;
    let lock_path = std::env::temp_dir().join("harfiy.lock");
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)
        .ok()?;

    let fd = file.as_raw_fd();
    let res = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
    if res == 0 {
        Some(SingleInstanceGuard { _file: file })
    } else {
        None
    }
}

#[cfg(not(any(windows, unix)))]
pub struct SingleInstanceGuard;

#[cfg(not(any(windows, unix)))]
pub fn acquire_single_instance_lock() -> Option<SingleInstanceGuard> {
    Some(SingleInstanceGuard)
}
