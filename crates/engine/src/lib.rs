//! Harfiy Engine - High-performance, zero-allocation FSM and rule engine
//! for real-time Uzbek Latin orthography conversion.

pub mod fsm;
pub mod rules;

pub use fsm::{EngineConfig, FsmState, HarfiyEngine, InputEvent, KeyAction, SpecialKey};
pub use rules::{is_apostrophe, is_first_trigger, is_word_delimiter, match_rule, ConversionMatch};
