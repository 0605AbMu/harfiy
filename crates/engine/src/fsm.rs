//! Finite State Machine for Harfiy.

use crate::rules::{is_first_trigger, is_word_delimiter, match_rule};

/// Action instructed by the engine to the platform emitter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    /// Pass the key event through to the target application unmodified.
    PassThrough,
    /// Suppress the key event (do not forward to the target application).
    Suppress,
    /// Erase previously entered characters with Backspace and emit a replacement string.
    Transform {
        backspaces_count: usize,
        replacement: &'static str,
    },
}

/// Navigation and control keys that trigger FSM resets or undo actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialKey {
    Backspace,
    Escape,
    Enter,
    Tab,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Home,
    End,
    PageUp,
    PageDown,
    Delete,
    Other,
}

/// Abstract input event received from the platform interceptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    Char(char),
    Special(SpecialKey),
}

/// Internal states of the Finite State Machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsmState {
    /// Waiting for any trigger key.
    Idle,
    /// First convertible key received ('s', 'c', 'o', 'g', etc.), awaiting second character.
    WaitingForSecondChar { prev_char: char },
    /// Just executed a Transform action. Immediate Backspace triggers Instant Undo.
    JustTransformed { undo_replacement: &'static str },
}

/// Configuration options for the engine.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Whether to enable 'ng' / 'NG' conversion to 'ñ' / 'Ñ' (default: false).
    pub enable_ng: bool,
    /// Whether the conversion engine is globally active (default: true).
    pub enabled: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            enable_ng: false,
            enabled: true,
        }
    }
}

/// Map a convertible pair to its exact original static string representation for Instant Undo.
fn get_original_pair_str(prev: char, curr: char) -> Option<&'static str> {
    match (prev, curr) {
        ('s', 'h') => Some("sh"),
        ('s', 'H') => Some("sH"),
        ('S', 'h') => Some("Sh"),
        ('S', 'H') => Some("SH"),

        ('c', 'h') => Some("ch"),
        ('c', 'H') => Some("cH"),
        ('C', 'h') => Some("Ch"),
        ('C', 'H') => Some("CH"),

        ('o', '\'') => Some("o'"),
        ('o', '`') => Some("o`"),
        ('o', '‘') => Some("o‘"),
        ('o', '’') => Some("o’"),
        ('o', 'ʻ') => Some("oʻ"),
        ('o', 'ʼ') => Some("oʼ"),
        ('o', '´') => Some("o´"),

        ('O', '\'') => Some("O'"),
        ('O', '`') => Some("O`"),
        ('O', '‘') => Some("O‘"),
        ('O', '’') => Some("O’"),
        ('O', 'ʻ') => Some("Oʻ"),
        ('O', 'ʼ') => Some("Oʼ"),
        ('O', '´') => Some("O´"),

        ('g', '\'') => Some("g'"),
        ('g', '`') => Some("g`"),
        ('g', '‘') => Some("g‘"),
        ('g', '’') => Some("g’"),
        ('g', 'ʻ') => Some("gʻ"),
        ('g', 'ʼ') => Some("gʼ"),
        ('g', '´') => Some("g´"),

        ('G', '\'') => Some("G'"),
        ('G', '`') => Some("G`"),
        ('G', '‘') => Some("G‘"),
        ('G', '’') => Some("G’"),
        ('G', 'ʻ') => Some("Gʻ"),
        ('G', 'ʼ') => Some("Gʼ"),
        ('G', '´') => Some("G´"),

        ('n', 'g') => Some("ng"),
        ('n', 'G') => Some("nG"),
        ('N', 'g') => Some("Ng"),
        ('N', 'G') => Some("NG"),

        _ => None,
    }
}

/// The core conversion engine containing the FSM and smart backspace history buffer.
#[derive(Debug, Clone)]
pub struct HarfiyEngine {
    state: FsmState,
    config: EngineConfig,
    history: Vec<char>,
}

impl Default for HarfiyEngine {
    fn default() -> Self {
        Self::new(EngineConfig::default())
    }
}

impl HarfiyEngine {
    /// Create a new engine instance with the given configuration.
    pub fn new(config: EngineConfig) -> Self {
        Self {
            state: FsmState::Idle,
            config,
            history: Vec::with_capacity(32),
        }
    }

    /// Enable or disable the engine.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.config.enabled = enabled;
        if !enabled {
            self.reset();
        }
    }

    /// Check if the engine is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Reset internal state and character history back to IDLE.
    pub fn reset(&mut self) {
        self.state = FsmState::Idle;
        self.history.clear();
    }

    /// Push a character to the bounded history buffer (keeps at most 32 recent chars).
    fn push_history(&mut self, c: char) {
        if self.history.len() >= 32 {
            self.history.remove(0);
        }
        self.history.push(c);
    }

    /// Get current FSM state (useful for diagnostics and testing).
    pub fn current_state(&self) -> FsmState {
        self.state
    }

    /// Process an input event and determine what action to take.
    pub fn process(&mut self, event: InputEvent) -> KeyAction {
        if !self.config.enabled {
            return KeyAction::PassThrough;
        }

        // Handle Instant Undo when in JustTransformed state
        if let FsmState::JustTransformed { undo_replacement } = self.state {
            if event == InputEvent::Special(SpecialKey::Backspace) {
                // Instant Undo: erase the converted char (1 backspace) and emit original pair
                self.state = FsmState::Idle;
                self.history.pop();
                for ch in undo_replacement.chars() {
                    self.push_history(ch);
                }
                return KeyAction::Transform {
                    backspaces_count: 1,
                    replacement: undo_replacement,
                };
            }
            // For any other event, return to Idle and process from Idle
            self.state = FsmState::Idle;
        }

        match event {
            InputEvent::Special(key) => self.handle_special(key),
            InputEvent::Char(c) => self.handle_char(c),
        }
    }

    fn handle_special(&mut self, key: SpecialKey) -> KeyAction {
        match key {
            SpecialKey::Backspace => {
                // Smart Backspace Buffer:
                // Pop the deleted character from history
                if !self.history.is_empty() {
                    self.history.pop();
                }

                // Check the new tail of history. If it is a first trigger,
                // restore WaitingForSecondChar state!
                if let Some(&prev) = self.history.last() {
                    if is_first_trigger(prev, self.config.enable_ng) {
                        self.state = FsmState::WaitingForSecondChar { prev_char: prev };
                    } else {
                        self.state = FsmState::Idle;
                    }
                } else {
                    self.state = FsmState::Idle;
                }
                KeyAction::PassThrough
            }
            SpecialKey::Escape
            | SpecialKey::Enter
            | SpecialKey::Tab
            | SpecialKey::ArrowLeft
            | SpecialKey::ArrowRight
            | SpecialKey::ArrowUp
            | SpecialKey::ArrowDown
            | SpecialKey::Home
            | SpecialKey::End
            | SpecialKey::PageUp
            | SpecialKey::PageDown
            | SpecialKey::Delete => {
                self.reset();
                KeyAction::PassThrough
            }
            SpecialKey::Other => KeyAction::PassThrough,
        }
    }

    fn handle_char(&mut self, c: char) -> KeyAction {
        // Check if character is a word boundary delimiter
        if is_word_delimiter(c) {
            self.reset();
            return KeyAction::PassThrough;
        }

        match self.state {
            FsmState::Idle => {
                self.push_history(c);
                if is_first_trigger(c, self.config.enable_ng) {
                    self.state = FsmState::WaitingForSecondChar { prev_char: c };
                }
                KeyAction::PassThrough
            }

            FsmState::WaitingForSecondChar { prev_char } => {
                // Check if (prev_char, c) forms a valid conversion rule
                if let Some(rule) = match_rule(prev_char, c, self.config.enable_ng) {
                    // Valid conversion:
                    let undo_str = get_original_pair_str(prev_char, c).unwrap_or(rule.replacement);
                    self.state = FsmState::JustTransformed {
                        undo_replacement: undo_str,
                    };
                    self.history.pop();
                    for ch in rule.replacement.chars() {
                        self.push_history(ch);
                    }
                    KeyAction::Transform {
                        backspaces_count: 1,
                        replacement: rule.replacement,
                    }
                } else {
                    // Does not form a pair with prev_char.
                    self.push_history(c);
                    // If 'c' is itself a first trigger, transition to WaitingForSecondChar(c)
                    if is_first_trigger(c, self.config.enable_ng) {
                        self.state = FsmState::WaitingForSecondChar { prev_char: c };
                    } else {
                        self.state = FsmState::Idle;
                    }
                    KeyAction::PassThrough
                }
            }

            FsmState::JustTransformed { .. } => {
                // Unreachable because JustTransformed was handled above, but for exhaustiveness:
                self.state = FsmState::Idle;
                self.handle_char(c)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sh_conversion() {
        let mut engine = HarfiyEngine::default();

        // Type 's'
        assert_eq!(
            engine.process(InputEvent::Char('s')),
            KeyAction::PassThrough
        );
        // Type 'h' -> should transform
        assert_eq!(
            engine.process(InputEvent::Char('h')),
            KeyAction::Transform {
                backspaces_count: 1,
                replacement: "ş",
            }
        );
    }

    #[test]
    fn test_sh_case_variations() {
        let mut engine = HarfiyEngine::default();

        // 'S' + 'h' -> 'Ş'
        assert_eq!(
            engine.process(InputEvent::Char('S')),
            KeyAction::PassThrough
        );
        assert_eq!(
            engine.process(InputEvent::Char('h')),
            KeyAction::Transform {
                backspaces_count: 1,
                replacement: "Ş",
            }
        );

        engine.reset();

        // 'S' + 'H' -> 'Ş'
        assert_eq!(
            engine.process(InputEvent::Char('S')),
            KeyAction::PassThrough
        );
        assert_eq!(
            engine.process(InputEvent::Char('H')),
            KeyAction::Transform {
                backspaces_count: 1,
                replacement: "Ş",
            }
        );
    }

    #[test]
    fn test_ch_conversion() {
        let mut engine = HarfiyEngine::default();

        assert_eq!(
            engine.process(InputEvent::Char('c')),
            KeyAction::PassThrough
        );
        assert_eq!(
            engine.process(InputEvent::Char('h')),
            KeyAction::Transform {
                backspaces_count: 1,
                replacement: "ç",
            }
        );

        engine.reset();

        assert_eq!(
            engine.process(InputEvent::Char('C')),
            KeyAction::PassThrough
        );
        assert_eq!(
            engine.process(InputEvent::Char('h')),
            KeyAction::Transform {
                backspaces_count: 1,
                replacement: "Ç",
            }
        );
    }

    #[test]
    fn test_apostrophe_conversion() {
        let mut engine = HarfiyEngine::default();

        for apo in ['\'', '`', '‘', '’', 'ʻ', 'ʼ', '´'] {
            // o' -> ö
            assert_eq!(
                engine.process(InputEvent::Char('o')),
                KeyAction::PassThrough
            );
            assert_eq!(
                engine.process(InputEvent::Char(apo)),
                KeyAction::Transform {
                    backspaces_count: 1,
                    replacement: "ö",
                }
            );

            // O' -> Ö
            assert_eq!(
                engine.process(InputEvent::Char('O')),
                KeyAction::PassThrough
            );
            assert_eq!(
                engine.process(InputEvent::Char(apo)),
                KeyAction::Transform {
                    backspaces_count: 1,
                    replacement: "Ö",
                }
            );

            // g' -> ğ
            assert_eq!(
                engine.process(InputEvent::Char('g')),
                KeyAction::PassThrough
            );
            assert_eq!(
                engine.process(InputEvent::Char(apo)),
                KeyAction::Transform {
                    backspaces_count: 1,
                    replacement: "ğ",
                }
            );

            // G' -> Ğ
            assert_eq!(
                engine.process(InputEvent::Char('G')),
                KeyAction::PassThrough
            );
            assert_eq!(
                engine.process(InputEvent::Char(apo)),
                KeyAction::Transform {
                    backspaces_count: 1,
                    replacement: "Ğ",
                }
            );
        }
    }

    #[test]
    fn test_instant_undo() {
        let mut engine = HarfiyEngine::default();

        // Type 's' then 'h' -> transforms to 'ş'
        engine.process(InputEvent::Char('s'));
        assert_eq!(
            engine.process(InputEvent::Char('h')),
            KeyAction::Transform {
                backspaces_count: 1,
                replacement: "ş",
            }
        );

        // Immediate Backspace -> Instant Undo to "sh"
        assert_eq!(
            engine.process(InputEvent::Special(SpecialKey::Backspace)),
            KeyAction::Transform {
                backspaces_count: 1,
                replacement: "sh",
            }
        );

        // Second Backspace -> normal PassThrough
        assert_eq!(
            engine.process(InputEvent::Special(SpecialKey::Backspace)),
            KeyAction::PassThrough
        );
    }

    #[test]
    fn test_always_converts_even_ishoq() {
        let mut engine = HarfiyEngine::default();

        // Type "Is"
        assert_eq!(
            engine.process(InputEvent::Char('I')),
            KeyAction::PassThrough
        );
        assert_eq!(
            engine.process(InputEvent::Char('s')),
            KeyAction::PassThrough
        );
        // Type 'h' -> Always converts ('s' + 'h' -> 'ş')!
        assert_eq!(
            engine.process(InputEvent::Char('h')),
            KeyAction::Transform {
                backspaces_count: 1,
                replacement: "ş",
            }
        );
    }

    #[test]
    fn test_always_converts_even_mushaf() {
        let mut engine = HarfiyEngine::default();

        // Type "mus"
        for c in ['m', 'u', 's'] {
            assert_eq!(engine.process(InputEvent::Char(c)), KeyAction::PassThrough);
        }
        // Type 'h' -> Always converts to 'ş'!
        assert_eq!(
            engine.process(InputEvent::Char('h')),
            KeyAction::Transform {
                backspaces_count: 1,
                replacement: "ş",
            }
        );
    }

    #[test]
    fn test_normal_word_shahar() {
        let mut engine = HarfiyEngine::default();

        // Type 's' then 'h' at word start -> transforms to 'ş'
        assert_eq!(
            engine.process(InputEvent::Char('s')),
            KeyAction::PassThrough
        );
        assert_eq!(
            engine.process(InputEvent::Char('h')),
            KeyAction::Transform {
                backspaces_count: 1,
                replacement: "ş",
            }
        );
    }

    #[test]
    fn test_navigation_resets() {
        let mut engine = HarfiyEngine::default();

        engine.process(InputEvent::Char('s'));
        // ArrowLeft resets FSM
        assert_eq!(
            engine.process(InputEvent::Special(SpecialKey::ArrowLeft)),
            KeyAction::PassThrough
        );
        // 'h' now is just 'h', no transform!
        assert_eq!(
            engine.process(InputEvent::Char('h')),
            KeyAction::PassThrough
        );
    }

    #[test]
    fn test_smart_backspace_restores_trigger_osa() {
        let mut engine = HarfiyEngine::default();

        // User types 'o', 's', 'a'
        assert_eq!(engine.process(InputEvent::Char('o')), KeyAction::PassThrough);
        assert_eq!(engine.process(InputEvent::Char('s')), KeyAction::PassThrough);
        assert_eq!(engine.process(InputEvent::Char('a')), KeyAction::PassThrough);

        // User deletes 'a' with Backspace
        assert_eq!(
            engine.process(InputEvent::Special(SpecialKey::Backspace)),
            KeyAction::PassThrough
        );

        // Now user types 'h' -> should transform 's' + 'h' -> 'ş'!
        assert_eq!(
            engine.process(InputEvent::Char('h')),
            KeyAction::Transform {
                backspaces_count: 1,
                replacement: "ş",
            }
        );
    }

    #[test]
    fn test_smart_backspace_apostrophe_oga() {
        let mut engine = HarfiyEngine::default();

        // User types 'o', 'g', 'x'
        assert_eq!(engine.process(InputEvent::Char('o')), KeyAction::PassThrough);
        assert_eq!(engine.process(InputEvent::Char('g')), KeyAction::PassThrough);
        assert_eq!(engine.process(InputEvent::Char('x')), KeyAction::PassThrough);

        // Delete 'x'
        assert_eq!(
            engine.process(InputEvent::Special(SpecialKey::Backspace)),
            KeyAction::PassThrough
        );

        // Type apostrophe -> should transform 'g' + '\'' -> 'ğ'!
        assert_eq!(
            engine.process(InputEvent::Char('\'')),
            KeyAction::Transform {
                backspaces_count: 1,
                replacement: "ğ",
            }
        );
    }

    #[test]
    fn test_smart_backspace_multiple_pops() {
        let mut engine = HarfiyEngine::default();

        // Type 's', 'x', 'y'
        assert_eq!(engine.process(InputEvent::Char('s')), KeyAction::PassThrough);
        assert_eq!(engine.process(InputEvent::Char('x')), KeyAction::PassThrough);
        assert_eq!(engine.process(InputEvent::Char('y')), KeyAction::PassThrough);

        // Backspace twice
        assert_eq!(
            engine.process(InputEvent::Special(SpecialKey::Backspace)),
            KeyAction::PassThrough
        );
        assert_eq!(
            engine.process(InputEvent::Special(SpecialKey::Backspace)),
            KeyAction::PassThrough
        );

        // Now 's' is active again -> type 'h'
        assert_eq!(
            engine.process(InputEvent::Char('h')),
            KeyAction::Transform {
                backspaces_count: 1,
                replacement: "ş",
            }
        );
    }
}
