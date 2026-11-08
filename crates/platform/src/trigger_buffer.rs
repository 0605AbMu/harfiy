//! Platform-independent smart trigger buffer.
//!
//! Some applications run inline autocomplete on every keystroke, so a lone first-trigger
//! letter (`s`, `c`, `o`, `g`, and `n` when `ng` is enabled) can make them insert a
//! suggestion before Harfiy gets the chance to convert the pair. To avoid that, the
//! keyboard hook holds such a letter back until the next key shows whether it forms a
//! conversion pair.
//!
//! [`TriggerBuffer`] holds the decision logic only. It never calls an OS API, never
//! allocates and knows nothing about time: the hook arms its own timeout and calls
//! [`TriggerBuffer::release`] when it fires.

use harfiy_engine::{is_first_trigger, is_word_delimiter, match_rule, SpecialKey};

/// A trigger key held back from the target application, with the
/// platform-specific data (`K`) needed to replay it exactly as typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Held<K> {
    /// The character the key produces.
    pub ch: char,
    /// Platform data needed to replay the key (scan code, virtual key, flags...).
    pub key: K,
}

/// What the hook must do with the current key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferAction {
    /// Not buffered: the key goes to the engine and on to the application as usual.
    Pass,
    /// The key is now held: swallow it and arm the timeout.
    Hold,
    /// The held key and this key form a rule: type `replacement` in place of both and swallow
    /// this key. The engine must be fed `prev` and then this key.
    Convert {
        /// The held character that starts the pair.
        prev: char,
        /// The converted text to type in place of both keys.
        replacement: &'static str,
    },
    /// Backspace erased the held key, which the application never saw: swallow it.
    Cancel,
}

/// The outcome of feeding one key to a [`TriggerBuffer`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferStep<K> {
    /// A held key that must be typed before the current key is handled.
    pub release: Option<Held<K>>,
    /// What to do with the current key.
    pub action: BufferAction,
}

impl<K> BufferStep<K> {
    const fn new(release: Option<Held<K>>, action: BufferAction) -> Self {
        Self { release, action }
    }
}

/// State machine deciding which trigger keys to hold back from the application.
#[derive(Debug, Clone)]
pub struct TriggerBuffer<K> {
    /// The key currently held back, if any.
    held: Option<Held<K>>,
    /// The last character known to have reached the application, or `None` after a
    /// word boundary, special key or conversion.
    last: Option<char>,
    /// Whether the optional `ng` -> `ñ` rule is active.
    enable_ng: bool,
}

impl<K: Copy> TriggerBuffer<K> {
    /// Create an idle buffer with the `ng` rule disabled.
    pub const fn new() -> Self {
        Self {
            held: None,
            last: None,
            enable_ng: false,
        }
    }

    /// Enable or disable the optional `ng` -> `ñ` rule.
    pub fn set_enable_ng(&mut self, enabled: bool) {
        self.enable_ng = enabled;
    }

    /// Whether a trigger key is currently held back.
    pub fn is_holding(&self) -> bool {
        self.held.is_some()
    }

    /// The key currently held back, if any.
    pub fn held(&self) -> Option<Held<K>> {
        self.held
    }

    /// Handle a character key.
    pub fn on_char(&mut self, ch: char, key: K) -> BufferStep<K> {
        let mut release = None;

        if let Some(h) = self.held.take() {
            if let Some(rule) = match_rule(h.ch, ch, self.enable_ng) {
                self.last = None;
                return BufferStep::new(
                    None,
                    BufferAction::Convert {
                        prev: h.ch,
                        replacement: rule.replacement,
                    },
                );
            }
            // Not a pair: the held key is typed first, then `ch` is handled as if idle.
            self.last = Some(h.ch);
            release = Some(h);
        }

        BufferStep::new(release, self.on_char_idle(ch, key))
    }

    /// Handle `ch` while nothing is held.
    fn on_char_idle(&mut self, ch: char, key: K) -> BufferAction {
        // The first letter already reached the app, so the engine converts the pair itself.
        if self
            .last
            .is_some_and(|l| match_rule(l, ch, self.enable_ng).is_some())
        {
            self.last = None;
            return BufferAction::Pass;
        }

        if is_first_trigger(ch, self.enable_ng) {
            self.held = Some(Held { ch, key });
            return BufferAction::Hold;
        }

        self.last = if is_word_delimiter(ch) {
            None
        } else {
            Some(ch)
        };
        BufferAction::Pass
    }

    /// Handle a non-character key.
    pub fn on_special(&mut self, key: SpecialKey) -> BufferStep<K> {
        match self.held.take() {
            // The app never saw the held key, so erasing it means just forgetting it.
            Some(_) if key == SpecialKey::Backspace => BufferStep::new(None, BufferAction::Cancel),
            Some(h) => {
                self.last = None;
                BufferStep::new(Some(h), BufferAction::Pass)
            }
            None => {
                self.last = None;
                BufferStep::new(None, BufferAction::Pass)
            }
        }
    }

    /// Give up the held key so it can be typed (timeout, modifier key, shutdown...).
    pub fn release(&mut self) -> Option<Held<K>> {
        let h = self.held.take()?;
        self.last = Some(h.ch);
        Some(h)
    }

    /// Forget everything without typing anything.
    pub fn reset(&mut self) {
        self.held = None;
        self.last = None;
    }
}

impl<K: Copy> Default for TriggerBuffer<K> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Buf = TriggerBuffer<u32>;

    fn buf(enable_ng: bool) -> Buf {
        let mut b = Buf::new();
        b.set_enable_ng(enable_ng);
        b
    }

    fn held(ch: char, key: u32) -> Option<Held<u32>> {
        Some(Held { ch, key })
    }

    fn step(release: Option<Held<u32>>, action: BufferAction) -> BufferStep<u32> {
        BufferStep { release, action }
    }

    fn convert(prev: char, replacement: &'static str) -> BufferAction {
        BufferAction::Convert { prev, replacement }
    }

    /// Feed `first` then `second` and expect a conversion to `replacement`.
    fn assert_pair(enable_ng: bool, first: char, second: char, replacement: &'static str) {
        let mut b = buf(enable_ng);
        assert_eq!(b.on_char(first, 1), step(None, BufferAction::Hold));
        assert_eq!(
            b.on_char(second, 2),
            step(None, convert(first, replacement)),
            "{first}{second}"
        );
        assert!(!b.is_holding());
    }

    #[test]
    fn static_new_is_const() {
        static B: std::sync::Mutex<TriggerBuffer<u32>> =
            std::sync::Mutex::new(TriggerBuffer::new());
        assert!(!B.lock().unwrap().is_holding());
    }

    #[test]
    fn default_is_idle() {
        let b = Buf::default();
        assert!(!b.is_holding());
        assert_eq!(b.held(), None);
    }

    #[test]
    fn sh_pairs_convert() {
        assert_pair(false, 's', 'h', "ş");
        assert_pair(false, 's', 'H', "ş");
        assert_pair(false, 'S', 'h', "Ş");
        assert_pair(false, 'S', 'H', "Ş");
    }

    #[test]
    fn ch_pairs_convert() {
        assert_pair(false, 'c', 'h', "ç");
        assert_pair(false, 'c', 'H', "ç");
        assert_pair(false, 'C', 'h', "Ç");
        assert_pair(false, 'C', 'H', "Ç");
    }

    #[test]
    fn o_apostrophe_pairs_convert() {
        for apos in ['\'', '`', '‘', '’', 'ʻ', 'ʼ'] {
            assert_pair(false, 'o', apos, "ö");
            assert_pair(false, 'O', apos, "Ö");
        }
    }

    #[test]
    fn g_apostrophe_pairs_convert() {
        for apos in ['\'', '`', '‘', '’', 'ʻ'] {
            assert_pair(false, 'g', apos, "ğ");
            assert_pair(false, 'G', apos, "Ğ");
        }
    }

    #[test]
    fn ng_converts_when_enabled() {
        assert_pair(true, 'n', 'g', "ñ");
        assert_pair(true, 'n', 'G', "ñ");
        assert_pair(true, 'N', 'g', "Ñ");
        assert_pair(true, 'N', 'G', "Ñ");
    }

    #[test]
    fn n_is_not_held_when_ng_disabled() {
        let mut b = buf(false);
        assert_eq!(b.on_char('n', 1), step(None, BufferAction::Pass));
        assert!(!b.is_holding());
        // `g` is a trigger in its own right (g').
        assert_eq!(b.on_char('g', 2), step(None, BufferAction::Hold));
        assert_eq!(b.held(), held('g', 2));
    }

    #[test]
    fn set_enable_ng_toggles_n_trigger() {
        let mut b = buf(false);
        assert_eq!(b.on_char('n', 1).action, BufferAction::Pass);
        b.set_enable_ng(true);
        b.reset();
        assert_eq!(b.on_char('n', 1).action, BufferAction::Hold);
    }

    #[test]
    fn conversion_clears_last() {
        let mut b = buf(false);
        b.on_char('s', 1);
        b.on_char('h', 2);
        // After `ş`, `h` must not pair with anything.
        assert_eq!(b.on_char('h', 3), step(None, BufferAction::Pass));
    }

    #[test]
    fn trigger_then_non_trigger_releases() {
        let mut b = buf(false);
        assert_eq!(b.on_char('s', 7), step(None, BufferAction::Hold));
        assert_eq!(b.on_char('a', 8), step(held('s', 7), BufferAction::Pass));
        assert!(!b.is_holding());
    }

    #[test]
    fn trigger_then_delimiter_releases() {
        let mut b = buf(false);
        b.on_char('o', 1);
        assert_eq!(b.on_char(' ', 2), step(held('o', 1), BufferAction::Pass));
        assert!(!b.is_holding());
    }

    #[test]
    fn cs_releases_c_and_holds_s() {
        let mut b = buf(false);
        assert_eq!(b.on_char('c', 1), step(None, BufferAction::Hold));
        assert_eq!(b.on_char('s', 2), step(held('c', 1), BufferAction::Hold));
        assert_eq!(b.held(), held('s', 2));
        // The newly held `s` still pairs with `h`.
        assert_eq!(b.on_char('h', 3), step(None, convert('s', "ş")));
    }

    #[test]
    fn oo_and_ss_chains_release_previous_and_hold_new() {
        let mut b = buf(false);
        b.on_char('o', 1);
        assert_eq!(b.on_char('o', 2), step(held('o', 1), BufferAction::Hold));
        assert_eq!(b.on_char('o', 3), step(held('o', 2), BufferAction::Hold));
        assert_eq!(b.on_char('\'', 4), step(None, convert('o', "ö")));

        let mut b = buf(false);
        b.on_char('s', 1);
        assert_eq!(b.on_char('s', 2), step(held('s', 1), BufferAction::Hold));
        assert_eq!(b.on_char('h', 3), step(None, convert('s', "ş")));
    }

    #[test]
    fn released_n_then_g_passes_when_ng_enabled() {
        // `nn` + `g`: the first `n` is released by the second, which is held and pairs.
        let mut b = buf(true);
        b.on_char('n', 1);
        assert_eq!(b.on_char('n', 2), step(held('n', 1), BufferAction::Hold));
        assert_eq!(b.on_char('g', 3), step(None, convert('n', "ñ")));
    }

    #[test]
    fn non_trigger_updates_last() {
        // `a` reached the app; an apostrophe after it is not a rule, so it just passes.
        let mut b = buf(false);
        assert_eq!(b.on_char('a', 1).action, BufferAction::Pass);
        assert_eq!(b.on_char('\'', 2).action, BufferAction::Pass);
        assert!(!b.is_holding());
    }

    #[test]
    fn word_delimiter_clears_last() {
        let mut b = buf(false);
        b.on_char('s', 1);
        assert_eq!(b.release(), held('s', 1));
        // A delimiter after the released `s` forgets it...
        assert_eq!(b.on_char(' ', 2), step(None, BufferAction::Pass));
        // ...so `h` is a plain letter again.
        assert_eq!(b.on_char('h', 3), step(None, BufferAction::Pass));
        assert!(!b.is_holding());
    }

    #[test]
    fn apostrophe_is_not_a_word_delimiter() {
        let mut b = buf(false);
        b.on_char('m', 1);
        b.on_char('a', 2);
        assert_eq!(b.on_char('\'', 3).action, BufferAction::Pass);
        // `n` alone is not a trigger with ng disabled.
        assert_eq!(b.on_char('n', 4).action, BufferAction::Pass);
        assert_eq!(b.on_char('o', 5).action, BufferAction::Hold);
    }

    #[test]
    fn backspace_cancels_held_key() {
        let mut b = buf(false);
        b.on_char('a', 1);
        b.on_char('s', 2);
        assert_eq!(
            b.on_special(SpecialKey::Backspace),
            step(None, BufferAction::Cancel)
        );
        assert!(!b.is_holding());
        // `last` is unchanged (`a`), so `h` is not converted against the cancelled `s`.
        assert_eq!(b.on_char('h', 3), step(None, BufferAction::Pass));
    }

    #[test]
    fn backspace_keeps_last_from_earlier_release() {
        // `s` released by timeout, then `o` held and erased: `last` stays `s`.
        let mut b = buf(false);
        b.on_char('s', 1);
        b.release();
        // `o` does not pair with `s`, so it is held.
        assert_eq!(b.on_char('o', 2).action, BufferAction::Hold);
        assert_eq!(
            b.on_special(SpecialKey::Backspace).action,
            BufferAction::Cancel
        );
        assert_eq!(b.on_char('h', 3), step(None, BufferAction::Pass));
        assert!(!b.is_holding());
    }

    #[test]
    fn backspace_when_idle_passes() {
        let mut b = buf(false);
        b.on_char('a', 1);
        assert_eq!(
            b.on_special(SpecialKey::Backspace),
            step(None, BufferAction::Pass)
        );
    }

    #[test]
    fn other_specials_release_held_key() {
        for key in [
            SpecialKey::Enter,
            SpecialKey::Tab,
            SpecialKey::ArrowLeft,
            SpecialKey::Escape,
            SpecialKey::Delete,
            SpecialKey::Other,
        ] {
            let mut b = buf(false);
            b.on_char('s', 42);
            assert_eq!(
                b.on_special(key),
                step(held('s', 42), BufferAction::Pass),
                "{key:?}"
            );
            assert!(!b.is_holding());
            // `last` is cleared, so `h` does not pair with the released `s`.
            assert_eq!(
                b.on_char('h', 43),
                step(None, BufferAction::Pass),
                "{key:?}"
            );
        }
    }

    #[test]
    fn special_when_idle_clears_last() {
        let mut b = buf(false);
        b.on_char('s', 1);
        b.release();
        assert_eq!(
            b.on_special(SpecialKey::ArrowRight),
            step(None, BufferAction::Pass)
        );
        // `s` is no longer the previous char: `h` passes as a plain letter.
        assert_eq!(b.on_char('h', 2), step(None, BufferAction::Pass));
    }

    #[test]
    fn timeout_release_then_h_passes() {
        let mut b = buf(false);
        b.on_char('s', 5);
        assert_eq!(b.release(), held('s', 5));
        assert!(!b.is_holding());
        // The engine converts `sh` itself; `h` must not be held.
        assert_eq!(b.on_char('h', 6), step(None, BufferAction::Pass));
        assert!(!b.is_holding());
    }

    #[test]
    fn released_n_then_g_passes_with_ng() {
        let mut b = buf(true);
        assert_eq!(b.on_char('n', 1).action, BufferAction::Hold);
        assert_eq!(b.release(), held('n', 1));
        assert_eq!(b.on_char('g', 2), step(None, BufferAction::Pass));
        assert!(!b.is_holding());
    }

    #[test]
    fn passed_n_then_g_holds_without_ng() {
        let mut b = buf(false);
        assert_eq!(b.on_char('n', 1).action, BufferAction::Pass);
        assert_eq!(b.on_char('g', 2), step(None, BufferAction::Hold));
        assert_eq!(b.held(), held('g', 2));
    }

    #[test]
    fn released_o_then_apostrophe_passes() {
        let mut b = buf(false);
        b.on_char('O', 1);
        b.release();
        assert_eq!(b.on_char('’', 2), step(None, BufferAction::Pass));
        // The pair is consumed: a following apostrophe pairs with nothing.
        assert_eq!(b.on_char('\'', 3), step(None, BufferAction::Pass));
    }

    #[test]
    fn released_g_then_g_holds_new_g() {
        // `gg` is not a rule, so the second `g` is held after the first was released.
        let mut b = buf(false);
        b.on_char('g', 1);
        b.release();
        assert_eq!(b.on_char('g', 2), step(None, BufferAction::Hold));
    }

    #[test]
    fn release_when_idle_returns_none() {
        let mut b = buf(false);
        assert_eq!(b.release(), None);
        b.on_char('a', 1);
        assert_eq!(b.release(), None);
    }

    #[test]
    fn held_and_is_holding_track_state() {
        let mut b = buf(false);
        assert!(!b.is_holding());
        assert_eq!(b.held(), None);
        b.on_char('c', 9);
        assert!(b.is_holding());
        assert_eq!(b.held(), held('c', 9));
        // Inspecting does not consume.
        assert_eq!(b.held(), held('c', 9));
        b.release();
        assert!(!b.is_holding());
        assert_eq!(b.held(), None);
    }

    #[test]
    fn key_payload_is_returned_unchanged() {
        let mut b: TriggerBuffer<(u16, u32)> = TriggerBuffer::new();
        b.on_char('S', (0x1F, 0xDEAD_BEEF));
        assert_eq!(
            b.on_char('x', (0x2D, 1)).release,
            Some(Held {
                ch: 'S',
                key: (0x1F, 0xDEAD_BEEF)
            })
        );

        b.on_char('g', (0x22, u32::MAX));
        assert_eq!(
            b.release(),
            Some(Held {
                ch: 'g',
                key: (0x22, u32::MAX)
            })
        );

        b.on_char('o', (0x18, 0));
        assert_eq!(
            b.on_special(SpecialKey::Tab).release,
            Some(Held {
                ch: 'o',
                key: (0x18, 0)
            })
        );
    }

    #[test]
    fn reset_forgets_held_and_last() {
        let mut b = buf(false);
        b.on_char('s', 1);
        b.reset();
        assert!(!b.is_holding());
        assert_eq!(b.release(), None);

        b.on_char('s', 2);
        b.release();
        b.reset();
        // `last` was forgotten: `h` is not part of a pair.
        assert_eq!(b.on_char('h', 3), step(None, BufferAction::Pass));
    }

    #[test]
    fn typing_shahar() {
        // s h a h a r: `sh` converts, the rest passes.
        let mut b = buf(false);
        assert_eq!(b.on_char('s', 1).action, BufferAction::Hold);
        assert_eq!(b.on_char('h', 2).action, convert('s', "ş"));
        for (i, ch) in "ahar".chars().enumerate() {
            assert_eq!(
                b.on_char(ch, 3 + i as u32),
                step(None, BufferAction::Pass),
                "{ch}"
            );
        }
    }
}
