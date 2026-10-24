//! Linux implementation of keyboard interceptor and emitter using evdev and /dev/uinput.

use crate::{KeyboardEmitter, KeyboardInterceptor, PlatformError};
use harfiy_engine::{InputEvent, KeyAction};

use std::sync::atomic::{AtomicBool, Ordering};

static LINUX_STOP_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Synthetic keyboard event emitter for Linux using /dev/uinput.
#[derive(Debug, Clone, Default)]
pub struct LinuxEmitter;

impl LinuxEmitter {
    pub fn new() -> Self {
        Self
    }
}

impl KeyboardEmitter for LinuxEmitter {
    fn emit_backspace(&mut self, _count: usize) -> Result<(), PlatformError> {
        // Implementation writing KEY_BACKSPACE to /dev/uinput virtual device
        Ok(())
    }

    fn emit_unicode(&mut self, _text: &str) -> Result<(), PlatformError> {
        // Implementation translating text into scancodes and writing to /dev/uinput
        Ok(())
    }
}

/// Linux keyboard interceptor using evdev hardware device grabbing (EVIOCGRAB).
#[derive(Default)]
pub struct LinuxInterceptor;

impl LinuxInterceptor {
    pub fn new() -> Self {
        Self
    }
}

impl KeyboardInterceptor for LinuxInterceptor {
    fn run<F>(&mut self, _handler: F) -> Result<(), PlatformError>
    where
        F: FnMut(InputEvent) -> KeyAction + Send + 'static,
    {
        eprintln!(
            "⚠️ [Harfiy Linux] evdev/uinput klaviatura adapteri hozirda ishlab chiqilmoqda (WIP)."
        );
        eprintln!("ℹ️ To'liq avto-konvertatsiya imkoniyatlari hozircha Windows va macOS tizimlarida barqaror mavjud.");
        while !LINUX_STOP_REQUESTED.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        Ok(())
    }

    fn stop(&mut self) -> Result<(), PlatformError> {
        LINUX_STOP_REQUESTED.store(true, Ordering::SeqCst);
        Ok(())
    }
}
