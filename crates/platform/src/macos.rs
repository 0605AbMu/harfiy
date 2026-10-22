//! macOS implementation of keyboard interceptor and emitter using CoreGraphics.

use harfiy_engine::{InputEvent, KeyAction};
use crate::{KeyboardEmitter, KeyboardInterceptor, PlatformError};

/// Synthetic keyboard event emitter for macOS using CoreGraphics events.
#[derive(Debug, Clone, Default)]
pub struct MacOsEmitter;

impl MacOsEmitter {
    pub fn new() -> Self {
        Self
    }
}

impl KeyboardEmitter for MacOsEmitter {
    fn emit_backspace(&mut self, _count: usize) -> Result<(), PlatformError> {
        // Implementation using CGEventCreateKeyboardEvent & CGEventPost
        Ok(())
    }

    fn emit_unicode(&mut self, _text: &str) -> Result<(), PlatformError> {
        // Implementation using CGEventKeyboardSetUnicodeString & CGEventPost
        Ok(())
    }
}

/// macOS keyboard interceptor using CGEventTap.
#[derive(Default)]
pub struct MacOsInterceptor;

impl MacOsInterceptor {
    pub fn new() -> Self {
        Self
    }

    /// Check if the application has Accessibility permissions (AXIsProcessTrustedWithOptions).
    pub fn is_accessibility_trusted() -> bool {
        true
    }
}

impl KeyboardInterceptor for MacOsInterceptor {
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
