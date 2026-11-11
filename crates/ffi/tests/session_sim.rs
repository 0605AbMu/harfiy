//! Simulation tests for the accessibility edit session.
//!
//! A model editor implements Android's `BaseInputConnection` semantics (composing region,
//! `commitText` replacing the composing region before the selection,
//! `deleteSurroundingText` skipping the composing region...). A model keyboard types into
//! it, either composing every word (Samsung Keyboard, Gboard with suggestions) or key by
//! key. Every editor change produces a text event and a selection update, delivered to
//! the session over two independent channels with random delays, as on a real device.
//!
//! The invariant: the final text equals the batch conversion of what the keyboard typed
//! — never anything else. When a keystroke is forced between the session's snapshot and
//! its write (a race no Android API can rule out), the read-back or the shape of the
//! write's echo must catch it.

use std::collections::VecDeque;

use harfiy_engine::convert_text;
use harfiy_ffi::session::{
    Session, SnapshotResult, Tick, WriteMode, SNAPSHOT_LEN, TAIL_LEN, VERIFY_SLACK,
};

// ---------------------------------------------------------------------------------------
// Deterministic RNG (xorshift), so failures are reproducible from the seed.

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        if hi <= lo {
            return lo;
        }
        lo + (self.next() % (hi - lo + 1) as u64) as i64
    }
}

// ---------------------------------------------------------------------------------------
// Model editor (BaseInputConnection semantics; BMP text only, so chars == UTF-16 units).

#[derive(Debug, Clone)]
enum Out {
    /// from, removed, added, text before from+added
    Text(i32, i32, i32, String),
    /// sel start, sel end, composing start, composing end
    Sel(i32, i32, i32, i32),
}

struct Editor {
    text: Vec<char>,
    sel: (usize, usize),
    comp: Option<(usize, usize)>,
    offset_ok: bool,
    out: Vec<Out>,
}

impl Editor {
    fn new(offset_ok: bool) -> Self {
        Self {
            text: Vec::new(),
            sel: (0, 0),
            comp: None,
            offset_ok,
            out: Vec::new(),
        }
    }

    fn string(&self) -> String {
        self.text.iter().collect()
    }

    fn sel_range(&self) -> (usize, usize) {
        (self.sel.0.min(self.sel.1), self.sel.0.max(self.sel.1))
    }

    fn replace(&mut self, a: usize, b: usize, t: &str) -> usize {
        let t: Vec<char> = t.chars().collect();
        self.text.splice(a..b, t.iter().copied());
        if b > a || !t.is_empty() {
            let tail_end = a + t.len();
            let tail: String = self.text[tail_end.saturating_sub(TAIL_LEN)..tail_end]
                .iter()
                .collect();
            self.out
                .push(Out::Text(a as i32, (b - a) as i32, t.len() as i32, tail));
        }
        a + t.len()
    }

    fn emit_sel(&mut self) {
        let (cs, ce) = self.comp.map_or((-1, -1), |(a, b)| (a as i32, b as i32));
        self.out
            .push(Out::Sel(self.sel.0 as i32, self.sel.1 as i32, cs, ce));
    }

    fn commit_text(&mut self, t: &str) {
        let (a, b) = self.comp.unwrap_or_else(|| self.sel_range());
        let end = self.replace(a, b, t);
        self.comp = None;
        self.sel = (end, end);
        self.emit_sel();
    }

    fn set_composing_text(&mut self, t: &str) {
        let (a, b) = self.comp.unwrap_or_else(|| self.sel_range());
        let end = self.replace(a, b, t);
        self.comp = if t.is_empty() { None } else { Some((a, end)) };
        self.sel = (end, end);
        self.emit_sel();
    }

    fn finish_composing(&mut self) {
        if self.comp.take().is_some() {
            self.emit_sel();
        }
    }

    fn delete_surrounding(&mut self, n: usize) {
        let (mut a, _) = self.sel_range();
        if let Some((ca, _)) = self.comp {
            a = a.min(ca);
        }
        let from = a.saturating_sub(n);
        let removed = a - from;
        if removed == 0 {
            return;
        }
        self.replace(from, a, "");
        let shift = |p: usize| if p >= a { p - removed } else { p };
        self.sel = (shift(self.sel.0), shift(self.sel.1));
        self.comp = self.comp.map(|(x, y)| (shift(x), shift(y)));
        self.emit_sel();
    }

    /// getSurroundingText(n, 0, 0): text before the selection start, collapsed, offset.
    fn surrounding(&self, n: i32) -> (String, bool, i32) {
        let (a, b) = self.sel_range();
        let from = a.saturating_sub(n as usize);
        let before: String = self.text[from..a].iter().collect();
        let abs = if self.offset_ok { a as i32 } else { -1 };
        (before, a == b, abs)
    }
}

// ---------------------------------------------------------------------------------------
// Model keyboard.

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kb {
    /// Composes every word, commits it on a delimiter (Samsung Keyboard, Gboard).
    Composing,
    /// Commits every key immediately.
    Plain,
}

#[derive(Debug, Clone)]
enum Key {
    Char(char),
    Backspace,
    /// Tap a suggestion: commit `word` plus an auto-space in place of the composing word.
    Pick(&'static str),
}

fn keys(s: &str) -> Vec<Key> {
    s.chars()
        .map(|c| {
            if c == '<' {
                Key::Backspace
            } else {
                Key::Char(c)
            }
        })
        .collect()
}

fn is_delim(c: char) -> bool {
    harfiy_engine::is_word_delimiter(c)
}

struct Keyboard {
    kind: Kb,
    word: String,
}

impl Keyboard {
    fn press(&mut self, ed: &mut Editor, key: &Key) {
        if ed.comp.is_none() {
            // The keyboard noticed (via onUpdateSelection) that its word was committed.
            self.word.clear();
        }
        match (self.kind, key) {
            (Kb::Plain, Key::Char(c)) => ed.commit_text(&c.to_string()),
            (Kb::Plain, Key::Backspace) => ed.delete_surrounding(1),
            (Kb::Plain, Key::Pick(w)) => ed.commit_text(&format!("{w} ")),
            (Kb::Composing, Key::Char(c)) if is_delim(*c) => {
                ed.finish_composing();
                self.word.clear();
                ed.commit_text(&c.to_string());
            }
            (Kb::Composing, Key::Char(c)) => {
                self.word.push(*c);
                let w = self.word.clone();
                ed.set_composing_text(&w);
            }
            (Kb::Composing, Key::Backspace) => {
                if self.word.pop().is_some() && ed.comp.is_some() {
                    let w = self.word.clone();
                    ed.set_composing_text(&w);
                } else {
                    ed.delete_surrounding(1);
                }
            }
            (Kb::Composing, Key::Pick(w)) => {
                ed.commit_text(w);
                self.word.clear();
                ed.commit_text(" ");
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// The device: editor + keyboard + two delayed channels + the session's host loop.

#[derive(Clone, Copy)]
struct Config {
    kb: Kb,
    offset_ok: bool,
    /// Milliseconds between keystrokes (min, max).
    gap: (i64, i64),
    /// Delivery delay of text events and of selection updates (min, max).
    text_delay: (i64, i64),
    sel_delay: (i64, i64),
    /// Probability (percent) that a keystroke lands between snapshot and write.
    race_pct: u64,
    /// Whether the editor delivers selection callbacks at all (some fields don't).
    sel_callbacks: bool,
    seed: u64,
}

impl Config {
    fn new(kb: Kb) -> Self {
        Self {
            kb,
            offset_ok: true,
            gap: (60, 180),
            text_delay: (0, 25),
            sel_delay: (0, 25),
            race_pct: 0,
            sel_callbacks: true,
            seed: 0x9E37_79B9_7F4A_7C15,
        }
    }
}

struct Device {
    cfg: Config,
    rng: Rng,
    ed: Editor,
    kb: Keyboard,
    s: Session,
    now: i64,
    /// (deliver_at, seq, event) for each channel; FIFO within a channel.
    text_q: VecDeque<(i64, Out)>,
    sel_q: VecDeque<(i64, Out)>,
    tick_at: Option<i64>,
    script: VecDeque<(i64, Key)>,
    writes: usize,
    failed_verifications: usize,
}

impl Device {
    fn new(cfg: Config, script: Vec<Key>) -> Self {
        let mut rng = Rng(cfg.seed | 1);
        let mut t = 100;
        let mut timed = VecDeque::new();
        for k in script {
            t += rng.range(cfg.gap.0, cfg.gap.1);
            timed.push_back((t, k));
        }
        let mut s = Session::new();
        s.configure(false, true);
        s.start_input(true, 0, 0, 0);
        Self {
            cfg,
            rng,
            ed: Editor::new(cfg.offset_ok),
            kb: Keyboard {
                kind: cfg.kb,
                word: String::new(),
            },
            s,
            now: 0,
            text_q: VecDeque::new(),
            sel_q: VecDeque::new(),
            tick_at: None,
            script: timed,
            writes: 0,
            failed_verifications: 0,
        }
    }

    fn flush_editor_events(&mut self) {
        for ev in std::mem::take(&mut self.ed.out) {
            match ev {
                Out::Text(..) => {
                    let d = self.rng.range(self.cfg.text_delay.0, self.cfg.text_delay.1);
                    let at = (self.now + d).max(self.text_q.back().map_or(0, |e| e.0));
                    self.text_q.push_back((at, ev));
                }
                Out::Sel(..) if !self.cfg.sel_callbacks => {}
                Out::Sel(..) => {
                    let d = self.rng.range(self.cfg.sel_delay.0, self.cfg.sel_delay.1);
                    let at = (self.now + d).max(self.sel_q.back().map_or(0, |e| e.0));
                    self.sel_q.push_back((at, ev));
                }
            }
        }
    }

    fn apply_tick(&mut self, tick: Tick) {
        match tick {
            Tick::Idle => {}
            Tick::After(d) => self.tick_at = Some(self.now + d),
            Tick::Snapshot => self.tick_at = Some(self.now),
        }
    }

    fn press_next(&mut self) {
        if let Some((_, key)) = self.script.pop_front() {
            self.kb.press(&mut self.ed, &key);
            self.flush_editor_events();
        }
    }

    fn snapshot(&mut self) {
        let (before, collapsed, abs) = self.ed.surrounding(SNAPSHOT_LEN);
        match self.s.on_snapshot(&before, collapsed, abs, self.now) {
            SnapshotResult::Done => {}
            SnapshotResult::Retry(d) => self.tick_at = Some(self.now + d),
            SnapshotResult::Write(plan) => {
                if self.cfg.race_pct > 0 && (self.rng.next() % 100) < self.cfg.race_pct {
                    // A keystroke slips in between our read and our write.
                    self.press_next();
                }
                match plan.mode {
                    WriteMode::Delete => {
                        self.ed.delete_surrounding(plan.expected.chars().count());
                        self.ed.commit_text(&plan.replacement);
                    }
                    WriteMode::Insert => self.ed.commit_text(&plan.replacement),
                }
                self.writes += 1;
                self.flush_editor_events();
                let (rb, _, abs) = self.ed.surrounding(plan.verify_len + VERIFY_SLACK);
                if !self.s.on_written(Some(&rb), abs, self.now) {
                    self.failed_verifications += 1;
                }
            }
        }
    }

    /// Runs until the script is typed and every channel and timer is drained.
    fn run(&mut self) {
        loop {
            let next_key = self.script.front().map(|k| k.0);
            let next_text = self.text_q.front().map(|e| e.0);
            let next_sel = self.sel_q.front().map(|e| e.0);
            let candidates = [next_key, next_text, next_sel, self.tick_at];
            let Some(t) = candidates.iter().flatten().min().copied() else {
                break;
            };
            self.now = self.now.max(t);
            if next_text == Some(t) {
                let (_, ev) = self.text_q.pop_front().unwrap();
                if let Out::Text(from, rem, add, tail) = ev {
                    let tick = self.s.on_text_changed(from, rem, add, &tail, self.now);
                    self.apply_tick(tick);
                }
            } else if next_sel == Some(t) {
                let (_, ev) = self.sel_q.pop_front().unwrap();
                if let Out::Sel(a, b, c, d) = ev {
                    let tick = self.s.on_selection(a, b, c, d, self.now);
                    self.apply_tick(tick);
                }
            } else if self.tick_at == Some(t) {
                self.tick_at = None;
                match self.s.on_tick(self.now) {
                    Tick::Snapshot => self.snapshot(),
                    other => self.apply_tick(other),
                }
            } else {
                self.press_next();
            }
        }
    }
}

/// What the keyboard alone would have produced.
fn baseline(kb: Kb, script: &[Key]) -> String {
    let mut ed = Editor::new(true);
    let mut k = Keyboard {
        kind: kb,
        word: String::new(),
    };
    for key in script {
        k.press(&mut ed, key);
    }
    ed.string()
}

fn expected(kb: Kb, script: &[Key]) -> String {
    convert_text(&baseline(kb, script), false, false).0
}

fn run(cfg: Config, script: Vec<Key>) -> Device {
    let mut d = Device::new(cfg, script);
    d.run();
    d
}

fn assert_converted(cfg: Config, script: Vec<Key>) {
    let want = expected(cfg.kb, &script);
    let d = run(cfg, script);
    assert_eq!(
        d.ed.string(),
        want,
        "seed {:#x}, diagnostics:\n{}",
        cfg.seed,
        d.s.diagnostics()
    );
}

const SENTENCE: &str = "bugun shahar chetida choyxonada ishladik shu choy o'rik g'isht ";

// ---------------------------------------------------------------------------------------
// Scenarios.

#[test]
fn composing_keyboard_in_order() {
    let mut cfg = Config::new(Kb::Composing);
    cfg.text_delay = (0, 0);
    cfg.sel_delay = (0, 0);
    assert_converted(cfg, keys(SENTENCE));
}

#[test]
fn composing_keyboard_text_event_before_selection_update() {
    // The device bug: the space's text event arrives while composing still looks active.
    let mut cfg = Config::new(Kb::Composing);
    cfg.text_delay = (0, 0);
    cfg.sel_delay = (15, 15);
    assert_converted(cfg, keys(SENTENCE));
}

#[test]
fn composing_keyboard_selection_update_before_text_event() {
    let mut cfg = Config::new(Kb::Composing);
    cfg.text_delay = (15, 15);
    cfg.sel_delay = (0, 0);
    assert_converted(cfg, keys(SENTENCE));
}

#[test]
fn composing_keyboard_fast_typing() {
    let mut cfg = Config::new(Kb::Composing);
    cfg.gap = (35, 70);
    assert_converted(cfg, keys(SENTENCE));
}

#[test]
fn plain_keyboard_converts_live() {
    let mut cfg = Config::new(Kb::Plain);
    cfg.text_delay = (0, 3);
    cfg.sel_delay = (0, 3);
    assert_converted(cfg, keys(SENTENCE));
}

#[test]
fn plain_keyboard_converts_mid_word_before_the_word_ends() {
    let cfg = Config {
        text_delay: (0, 0),
        sel_delay: (0, 0),
        ..Config::new(Kb::Plain)
    };
    let d = run(cfg, keys("ish"));
    assert_eq!(d.ed.string(), "iş");
}

#[test]
fn plain_keyboard_instant_undo_keeps_the_pair() {
    let cfg = Config {
        text_delay: (0, 2),
        sel_delay: (0, 2),
        ..Config::new(Kb::Plain)
    };
    // "Is" + "h" converts to "Iş"; Backspace restores "sh", which stays unconverted
    // even after the word is finished.
    let d = run(cfg, keys("Ish<oq "));
    assert_eq!(d.ed.string(), "Ishoq ", "{}", d.s.diagnostics());
}

#[test]
fn unknown_offset_editor_never_touches_text_start() {
    // Jetpack Compose / custom editors: SurroundingText.offset == -1. The old service
    // selected [0..n] and replaced the start of the text ("çoy har chunyo shahar").
    for kb in [Kb::Composing, Kb::Plain] {
        let mut cfg = Config::new(kb);
        cfg.offset_ok = false;
        assert_converted(cfg, keys("salom dunyo shahar choy "));
    }
}

#[test]
fn unknown_offset_editor_with_text_longer_than_the_snapshot() {
    // Snapshot-relative positions only equal absolute ones while the whole text fits in
    // the snapshot; past that, a relative range lands somewhere else in the text.
    for kb in [Kb::Composing, Kb::Plain] {
        let mut cfg = Config::new(kb);
        cfg.offset_ok = false;
        let long = format!(
            "{}{}",
            "salom dunyo bu matn juda uzun ".repeat(5),
            "shahar choy "
        );
        assert_converted(cfg, keys(&long));
    }
}

#[test]
fn no_selection_callbacks_still_converts_safely() {
    // Measured on Android 16: key events, and Samsung Keyboard in no-suggestion fields,
    // reach the accessibility service without any onUpdateSelection.
    for kb in [Kb::Composing, Kb::Plain] {
        for offset_ok in [true, false] {
            for seed in 1..=100u64 {
                let cfg = Config {
                    offset_ok,
                    sel_callbacks: false,
                    gap: (30, 200),
                    text_delay: (0, 40),
                    seed: seed.wrapping_mul(0x2545_F491_4F6C_DD1D),
                    ..Config::new(kb)
                };
                let script = keys(SENTENCE);
                let typed = baseline(kb, &script);
                let d = run(cfg, script);
                let got = d.ed.string();
                assert_eq!(
                    unconvert(&got),
                    unconvert(&typed),
                    "{kb:?} offset_ok={offset_ok} seed {seed}: {got:?}"
                );
                assert_eq!(
                    got,
                    expected(kb, &keys(SENTENCE)),
                    "{kb:?} offset_ok={offset_ok} seed {seed}\n{}",
                    d.s.diagnostics()
                );
            }
        }
    }
}

#[test]
fn no_selection_callbacks_plain_keyboard_is_live() {
    let cfg = Config {
        sel_callbacks: false,
        text_delay: (0, 0),
        ..Config::new(Kb::Plain)
    };
    assert_eq!(run(cfg, keys("ish")).ed.string(), "iş");
}

#[test]
fn suggestion_pick_is_converted() {
    let mut cfg = Config::new(Kb::Composing);
    cfg.text_delay = (0, 0);
    cfg.sel_delay = (10, 10);
    let mut script = keys("qishl");
    script.push(Key::Pick("qishloqda"));
    script.extend(keys("ishladik "));
    assert_converted(cfg, script);
}

#[test]
fn backspace_and_retype_inside_composing_word() {
    let cfg = Config::new(Kb::Composing);
    assert_converted(cfg, keys("shaxar<<har choyx<y "));
}

#[test]
fn randomized_delivery_orders_never_corrupt_text() {
    for seed in 1..=300u64 {
        for kb in [Kb::Composing, Kb::Plain] {
            for offset_ok in [true, false] {
                let cfg = Config {
                    offset_ok,
                    gap: (30, 200),
                    text_delay: (0, 40),
                    sel_delay: (0, 40),
                    seed: seed.wrapping_mul(0x2545_F491_4F6C_DD1D),
                    ..Config::new(kb)
                };
                let script = keys(SENTENCE);
                let typed = baseline(kb, &script);
                let d = run(cfg, script);
                let got = d.ed.string();
                // Never corrupted: undoing every conversion gives back what was typed.
                assert_eq!(
                    unconvert(&got),
                    unconvert(&typed),
                    "seed {seed:#x} {kb:?} offset_ok={offset_ok}: {got:?}\n{}",
                    d.s.diagnostics()
                );
                assert_eq!(d.failed_verifications, 0, "seed {seed:#x}: {got:?}");
            }
        }
    }
}

#[test]
fn randomized_delivery_converts_everything_with_human_timing() {
    for seed in 1..=200u64 {
        for kb in [Kb::Composing, Kb::Plain] {
            let cfg = Config {
                gap: (80, 220),
                text_delay: (0, 20),
                sel_delay: (0, 20),
                seed: seed.wrapping_mul(0x9E37_79B9_7F4A_7C15),
                ..Config::new(kb)
            };
            assert_converted(cfg, keys(SENTENCE));
        }
    }
}

#[test]
fn forced_races_are_caught_by_the_read_back() {
    // A keystroke between snapshot and write can't be prevented through the
    // accessibility InputConnection. What matters is that it never goes unnoticed:
    // either the text is right, or the session switched itself off for the field.
    let mut caught = 0;
    for seed in 1..=200u64 {
        let cfg = Config {
            race_pct: 100,
            gap: (40, 120),
            seed: seed.wrapping_mul(0xD6E8_FEB8_6659_FD93),
            ..Config::new(Kb::Composing)
        };
        let script = keys(SENTENCE);
        let want = expected(cfg.kb, &script);
        let d = run(cfg, script);
        if d.s.is_field_on() {
            assert_eq!(
                d.ed.string(),
                want,
                "seed {seed:#x} silently corrupted\n{}",
                d.s.diagnostics()
            );
        } else {
            caught += 1;
        }
    }
    eprintln!("forced races caught by read-back: {caught}/200");
}

/// Maps converted letters back to their typed pairs.
fn unconvert(s: &str) -> String {
    s.replace('ş', "sh")
        .replace('Ş', "Sh")
        .replace('ç', "ch")
        .replace('Ç', "Ch")
        .replace('ö', "o'")
        .replace('Ö', "O'")
        .replace('ğ', "g'")
        .replace('Ğ', "G'")
}
