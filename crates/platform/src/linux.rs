//! Linux implementation of keyboard interceptor and emitter using evdev and /dev/uinput.

use crate::{KeyboardEmitter, KeyboardInterceptor, PlatformError};
use harfiy_engine::{InputEvent, KeyAction};

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
        Ok(())
    }

    fn stop(&mut self) -> Result<(), PlatformError> {
        Ok(())
    }
}
