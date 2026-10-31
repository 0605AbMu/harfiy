//! WebAssembly bindings for Harfiy Engine (Web & SPA).
//!
//! Provides zero-overhead access to the unified Rust FSM and conversion rules
//! for browser environments, Single Page Applications (SPAs), and Node.js.

use harfiy_engine::{
    EngineConfig, FsmState, HarfiyEngine, InputEvent, KeyAction, SpecialKey,
};
use wasm_bindgen::prelude::*;

/// Result of processing a key event in WebAssembly.
#[wasm_bindgen]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmKeyAction {
    action_type: u8, // 0 = PassThrough, 1 = Suppress, 2 = Transform
    backspaces_count: usize,
    replacement: String,
}

#[wasm_bindgen]
impl WasmKeyAction {
    /// Action type: "passThrough", "suppress", or "transform"
    #[wasm_bindgen(getter)]
    pub fn action_type(&self) -> String {
        match self.action_type {
            0 => "passThrough".to_string(),
            1 => "suppress".to_string(),
            2 => "transform".to_string(),
            _ => "unknown".to_string(),
        }
    }

    /// Number of characters to delete backwards before inserting `replacement`.
    #[wasm_bindgen(getter)]
    pub fn backspaces_count(&self) -> usize {
        self.backspaces_count
    }

    /// Replacement string to insert into the input field.
    #[wasm_bindgen(getter)]
    pub fn replacement(&self) -> String {
        self.replacement.clone()
    }

    /// Whether this action requires transforming the text field.
    pub fn is_transform(&self) -> bool {
        self.action_type == 2
    }

    /// Whether this action allows passing the key through unmodified.
    pub fn is_pass_through(&self) -> bool {
        self.action_type == 0
    }

    /// Whether this action suppresses the key event.
    pub fn is_suppress(&self) -> bool {
        self.action_type == 1
    }
}

fn convert_action(action: KeyAction) -> WasmKeyAction {
    match action {
        KeyAction::PassThrough => WasmKeyAction {
            action_type: 0,
            backspaces_count: 0,
            replacement: String::new(),
        },
        KeyAction::Suppress => WasmKeyAction {
            action_type: 1,
            backspaces_count: 0,
            replacement: String::new(),
        },
        KeyAction::Transform {
            backspaces_count,
            replacement,
        } => WasmKeyAction {
            action_type: 2,
            backspaces_count,
            replacement: replacement.to_string(),
        },
    }
}

/// WebAssembly wrapper around [`HarfiyEngine`].
#[wasm_bindgen]
pub struct WasmHarfiyEngine {
    engine: HarfiyEngine,
}

#[wasm_bindgen]
impl WasmHarfiyEngine {
    /// Create a new Harfiy Engine instance.
    #[wasm_bindgen(constructor)]
    pub fn new(enable_ng: Option<bool>, enabled: Option<bool>) -> Self {
        let config = EngineConfig {
            enable_ng: enable_ng.unwrap_or(false),
            enabled: enabled.unwrap_or(true),
        };
        Self {
            engine: HarfiyEngine::new(config),
        }
    }

    /// Process a single character keystroke.
    pub fn process_char(&mut self, c: char) -> WasmKeyAction {
        let action = self.engine.process(InputEvent::Char(c));
        convert_action(action)
    }

    /// Process Backspace key (handles Smart Backspace and Instant Undo).
    pub fn process_backspace(&mut self) -> WasmKeyAction {
        let action = self.engine.process(InputEvent::Special(SpecialKey::Backspace));
        convert_action(action)
    }

    /// Process a key event by name (e.g. from KeyboardEvent.key: "Backspace", "Enter", "s", "h").
    pub fn process_key(&mut self, key: &str) -> WasmKeyAction {
        let event = match key {
            "Backspace" => InputEvent::Special(SpecialKey::Backspace),
            "Enter" => InputEvent::Special(SpecialKey::Enter),
            "Escape" => InputEvent::Special(SpecialKey::Escape),
            "Tab" => InputEvent::Special(SpecialKey::Tab),
            "ArrowLeft" => InputEvent::Special(SpecialKey::ArrowLeft),
            "ArrowRight" => InputEvent::Special(SpecialKey::ArrowRight),
            "ArrowUp" => InputEvent::Special(SpecialKey::ArrowUp),
            "ArrowDown" => InputEvent::Special(SpecialKey::ArrowDown),
            "Home" => InputEvent::Special(SpecialKey::Home),
            "End" => InputEvent::Special(SpecialKey::End),
            "PageUp" => InputEvent::Special(SpecialKey::PageUp),
            "PageDown" => InputEvent::Special(SpecialKey::PageDown),
            "Delete" => InputEvent::Special(SpecialKey::Delete),
            // Modifier keys should not disturb engine state (Shift, Alt, Ctrl, Meta, etc.)
            "Shift" | "Control" | "Alt" | "Meta" | "CapsLock" => {
                return WasmKeyAction {
                    action_type: 0,
                    backspaces_count: 0,
                    replacement: String::new(),
                };
            }
            s if s.chars().count() == 1 => {
                let c = s.chars().next().unwrap();
                InputEvent::Char(c)
            }
            _ => InputEvent::Special(SpecialKey::Other),
        };

        let action = self.engine.process(event);
        convert_action(action)
    }

    /// Reset internal state and character history back to IDLE.
    pub fn reset(&mut self) {
        self.engine.reset();
    }

    /// Enable or disable the engine.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.engine.set_enabled(enabled);
    }

    /// Check if the engine is enabled.
    pub fn is_enabled(&self) -> bool {
        self.engine.is_enabled()
    }

    /// Update 'ng' -> 'ñ' conversion setting dynamically.
    pub fn set_enable_ng(&mut self, enable_ng: bool) {
        self.engine.set_enable_ng(enable_ng);
    }

    /// Check if 'ng' conversion is enabled.
    pub fn is_enable_ng(&self) -> bool {
        self.engine.is_enable_ng()
    }

    /// Current FSM state name ("idle", "waitingForSecondChar", or "justTransformed").
    pub fn state_name(&self) -> String {
        match self.engine.current_state() {
            FsmState::Idle => "idle".to_string(),
            FsmState::WaitingForSecondChar { .. } => "waitingForSecondChar".to_string(),
            FsmState::JustTransformed { .. } => "justTransformed".to_string(),
        }
    }
}

/// A planned text edit with UTF-16 code unit offsets matching JavaScript string indexing.
#[wasm_bindgen]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmEdit {
    pub start: usize,
    pub end: usize,
    pub kind: i32,
    replacement: String,
}

#[wasm_bindgen]
impl WasmEdit {
    #[wasm_bindgen(getter)]
    pub fn replacement(&self) -> String {
        self.replacement.clone()
    }
}

/// Result of full text conversion with conversion statistics.
#[wasm_bindgen]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmConversionResult {
    text: String,
    pub conversions_count: usize,
}

#[wasm_bindgen]
impl WasmConversionResult {
    #[wasm_bindgen(getter)]
    pub fn text(&self) -> String {
        self.text.clone()
    }
}

/// Convert an entire string to the new Uzbek Latin orthography.
#[wasm_bindgen]
pub fn convert_text(
    text: &str,
    enable_ng: Option<bool>,
    smart_apostrophe: Option<bool>,
) -> String {
    let (res, _) = harfiy_engine::convert_text(
        text,
        enable_ng.unwrap_or(false),
        smart_apostrophe.unwrap_or(true),
    );
    res
}

/// Convert an entire string and also return statistics (number of conversions performed).
#[wasm_bindgen]
pub fn convert_text_with_stats(
    text: &str,
    enable_ng: Option<bool>,
    smart_apostrophe: Option<bool>,
) -> WasmConversionResult {
    let (text, conversions_count) = harfiy_engine::convert_text(
        text,
        enable_ng.unwrap_or(false),
        smart_apostrophe.unwrap_or(true),
    );
    WasmConversionResult {
        text,
        conversions_count,
    }
}

/// Plan text edits for a given text, with indices mapped to UTF-16 code units (for JS).
#[wasm_bindgen]
pub fn plan_edits_utf16(
    text: &str,
    enable_ng: Option<bool>,
    smart_apostrophe: Option<bool>,
) -> Vec<WasmEdit> {
    let chars: Vec<char> = text.chars().collect();
    let edits = harfiy_engine::plan_edits(
        &chars,
        enable_ng.unwrap_or(false),
        smart_apostrophe.unwrap_or(false),
    );
    if edits.is_empty() {
        return Vec::new();
    }

    let mut utf16_at = Vec::with_capacity(chars.len() + 1);
    let mut offset = 0usize;
    for c in &chars {
        utf16_at.push(offset);
        offset += c.len_utf16();
    }
    utf16_at.push(offset);

    edits
        .into_iter()
        .map(|e| WasmEdit {
            start: utf16_at[e.start],
            end: utf16_at[e.end],
            kind: e.kind.map_or(-1, |k| k as i32),
            replacement: e.replacement.to_string(),
        })
        .collect()
}

/// Check if a character is any recognized variation of an apostrophe.
#[wasm_bindgen]
pub fn is_apostrophe(c: char) -> bool {
    harfiy_engine::is_apostrophe(c)
}

/// Check if a character is a delimiter (word boundary) that resets the word buffer.
#[wasm_bindgen]
pub fn is_word_delimiter(c: char) -> bool {
    harfiy_engine::is_word_delimiter(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_convert() {
        let res = convert_text("shahar, choy, o'rmon, g'ildirak", None, None);
        assert_eq!(res, "şahar, çoy, örmon, ğildirak");

        let stats = convert_text_with_stats("shahar, choy, o'rmon, g'ildirak", None, None);
        assert_eq!(stats.text(), "şahar, çoy, örmon, ğildirak");
        assert_eq!(stats.conversions_count, 4);
    }

    #[test]
    fn test_fsm_realtime() {
        let mut engine = WasmHarfiyEngine::new(None, None);

        let a1 = engine.process_char('s');
        assert!(a1.is_pass_through());
        assert_eq!(engine.state_name(), "waitingForSecondChar");

        let a2 = engine.process_char('h');
        assert!(a2.is_transform());
        assert_eq!(a2.backspaces_count(), 1);
        assert_eq!(a2.replacement(), "ş");
        assert_eq!(engine.state_name(), "justTransformed");

        // Instant Undo: Backspace restores "sh"
        let undo = engine.process_backspace();
        assert!(undo.is_transform());
        assert_eq!(undo.backspaces_count(), 1);
        assert_eq!(undo.replacement(), "sh");
        assert_eq!(engine.state_name(), "idle");
    }

    #[test]
    fn test_process_key_string() {
        let mut engine = WasmHarfiyEngine::new(None, None);
        let _ = engine.process_key("c");
        let a = engine.process_key("h");
        assert!(a.is_transform());
        assert_eq!(a.replacement(), "ç");
    }

    #[test]
    fn test_plan_edits_utf16_surrogate_pairs() {
        // '😀' takes 2 UTF-16 code units in JS
        let edits = plan_edits_utf16("😀sh o'", None, None);
        assert_eq!(edits.len(), 2);
        assert_eq!(edits[0].start, 2);
        assert_eq!(edits[0].end, 4);
        assert_eq!(edits[0].replacement(), "ş");

        assert_eq!(edits[1].start, 5);
        assert_eq!(edits[1].end, 7);
        assert_eq!(edits[1].replacement(), "ö");
    }
}
