//! Mock platform implementations for testing without OS dependencies.

use crate::{KeyboardEmitter, KeyboardInterceptor, PlatformError};
use harfiy_engine::{InputEvent, KeyAction};
use std::sync::{Arc, Mutex};

/// Recorded event in the mock emitter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MockEmittedEvent {
    Backspace(usize),
    Unicode(String),
}

/// In-memory keyboard emitter that logs all synthetic inputs.
#[derive(Debug, Clone, Default)]
pub struct MockEmitter {
    events: Arc<Mutex<Vec<MockEmittedEvent>>>,
}

impl MockEmitter {
    pub fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn events(&self) -> Vec<MockEmittedEvent> {
        self.events.lock().unwrap().clone()
    }

    pub fn clear(&self) {
        self.events.lock().unwrap().clear();
    }
}

impl KeyboardEmitter for MockEmitter {
    fn emit_backspace(&mut self, count: usize) -> Result<(), PlatformError> {
        self.events
            .lock()
            .unwrap()
            .push(MockEmittedEvent::Backspace(count));
        Ok(())
    }

    fn emit_unicode(&mut self, text: &str) -> Result<(), PlatformError> {
        self.events
            .lock()
            .unwrap()
            .push(MockEmittedEvent::Unicode(text.to_string()));
        Ok(())
    }
}

/// In-memory interceptor that dispatches predefined events through the handler.
pub struct MockInterceptor {
    pending_events: Vec<InputEvent>,
    is_running: bool,
}

impl MockInterceptor {
    pub fn new(events: Vec<InputEvent>) -> Self {
        Self {
            pending_events: events,
            is_running: false,
        }
    }
}

impl KeyboardInterceptor for MockInterceptor {
    fn run<F>(&mut self, mut handler: F) -> Result<(), PlatformError>
    where
        F: FnMut(InputEvent) -> KeyAction + Send + 'static,
    {
        self.is_running = true;
        for event in self.pending_events.drain(..) {
            if !self.is_running {
                break;
            }
            let _ = handler(event);
        }
        self.is_running = false;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), PlatformError> {
        self.is_running = false;
        Ok(())
    }
}

/// Combined mock platform.
#[derive(Default)]
pub struct MockPlatform {
    pub emitter: MockEmitter,
}

impl MockPlatform {
    pub fn new() -> Self {
        Self {
            emitter: MockEmitter::new(),
        }
    }
}
