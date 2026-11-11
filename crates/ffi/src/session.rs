//! Edit session for hosts that convert text through an editor connection they share
//! with the user's keyboard (the Android accessibility service).
//!
//! The host and the keyboard both write to one text buffer and nothing serialises them:
//! the accessibility `InputConnection` has no batch edit and no atomic replace, and the
//! host learns about text and selection changes through two independent, unordered
//! channels. [`Session`] therefore follows one rule: **when in doubt, don't touch the
//! text**. A conversion is only written when
//!
//! - the keyboard has no composing region (it is not in the middle of a word),
//! - the field has been quiet for [`SETTLE_MS`] (keyboards that compose only),
//! - a fresh snapshot of the text shows the cursor exactly where the user last typed,
//! - the snapshot agrees with the last selection callback, when absolute offsets exist,
//!
//! and every write is read back. A read-back that doesn't match switches the session off
//! for the rest of the field ([`Session::start_input`] switches it on again), so one race
//! can never snowball into a corrupted message.
//!
//! Keyboards that never compose (plain key-by-key input) keep the immediate, per-key
//! conversion with instant undo; the settle delay only applies to composing keyboards.
//!
//! Writes never select text. `setSelection` + `commitText` was tried and measured on a
//! Samsung Keyboard: the keyboard sees the transient selection in `onUpdateSelection`
//! and, when the next key is already on its way, re-anchors its composing region on it
//! and overwrites the converted word ~80 ms *after* the write was verified. Deleting
//! before the cursor and committing never shows the keyboard a selection.
//!
//! The session never calls a platform API and knows nothing about time: the host passes
//! `now` (milliseconds, monotonic), arms its own timer for [`Tick::After`], and performs
//! the reads and writes the session asks for. All offsets are UTF-16 code units, as in
//! Java/Kotlin strings.

use std::collections::VecDeque;
use std::fmt::Write as _;

use harfiy_engine::{is_word_delimiter, plan_edits};

/// Quiet time required after the last text or selection change before converting text
/// typed with a composing keyboard. Lets the keyboard finish its commit sequence
/// (autocorrect, auto-space), lets both event channels catch up, and keeps writes out of
/// fast typing bursts, when the keyboard is most likely to act on stale positions.
pub const SETTLE_MS: i64 = 100;
/// Delay before re-reading the text when the snapshot disagreed with the events seen.
pub const RETRY_MS: i64 = 30;
/// Snapshots that may disagree with the events before the pending text is given up.
pub const MAX_RETRIES: u8 = 3;
/// Characters before the end of a text change the host passes as `tail`.
pub const TAIL_LEN: usize = 16;
/// UTF-16 units of text before the cursor the host should read for a snapshot.
pub const SNAPSHOT_LEN: i32 = 256;
/// Extra UTF-16 units read back after a write, so a keystroke that raced the read-back
/// is told apart from a write that went wrong.
pub const VERIFY_SLACK: i32 = 3;

/// How long an expected echo of our own write stays valid.
const ECHO_TTL_MS: i64 = 1500;
/// Circuit breaker: more than this many writes within the window switches the field off.
const BREAKER_WINDOW_MS: i64 = 500;
const BREAKER_MAX_WRITES: usize = 4;
/// Characters of untouched text before a replacement that the read-back must still show.
const VERIFY_CONTEXT: usize = 8;
const DIAG_CAPACITY: usize = 256;
const KIND_COUNT: usize = 5;

/// When the host should call [`Session::on_tick`] next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tick {
    /// Nothing to do until the next event.
    Idle,
    /// Call [`Session::on_tick`] after this many milliseconds (replacing any armed timer).
    After(i64),
    /// Take a snapshot now and pass it to [`Session::on_snapshot`].
    Snapshot,
}

/// How the host applies a [`Plan`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum WriteMode {
    /// `deleteSurroundingText(len(expected), 0)` then `commitText(replacement)`.
    Delete = 2,
    /// `commitText(replacement)` at the cursor (instant undo restoring a pair).
    Insert = 3,
}

/// A verified edit for the host to apply, then confirm with [`Session::on_written`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub mode: WriteMode,
    /// Absolute UTF-16 range that is replaced, for diagnostics; -1 when the editor
    /// reports no absolute offsets.
    pub start: i32,
    pub end: i32,
    /// The text right before the cursor that is replaced (empty for [`WriteMode::Insert`]).
    pub expected: String,
    pub replacement: String,
    /// UTF-16 units of text before the cursor to read back after the write.
    pub verify_len: i32,
    /// Conversions per `RuleKind` discriminant, for usage statistics.
    pub kinds: [u32; KIND_COUNT],
}

/// The outcome of [`Session::on_snapshot`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotResult {
    /// Apply this plan now, then call [`Session::on_written`].
    Write(Plan),
    /// The snapshot disagreed with the events seen so far; call [`Session::on_tick`]
    /// again after this many milliseconds.
    Retry(i64),
    /// Nothing to write.
    Done,
}

/// Text typed by the user that has not been converted yet: `from..end`, absolute.
#[derive(Debug, Clone)]
struct Pending {
    from: i32,
    end: i32,
    /// The last characters before `end` as the text event reported them; a snapshot
    /// must end with them. Empty when the event carried no text.
    tail: String,
}

/// The text event(s) our own write will cause, as `(from, removed, added)` shapes.
#[derive(Debug, Clone)]
struct Echo {
    shapes: Vec<(i32, i32, i32)>,
    /// After a split removal, the insertion that should follow it.
    then: Option<(i32, i32, i32)>,
    expires: i64,
}

/// A live pair conversion that Backspace may revert.
#[derive(Debug, Clone)]
struct Undo {
    /// Absolute offset of the converted character.
    pos: i32,
    original: String,
}

/// What the read-back after a write must show.
#[derive(Debug, Clone)]
struct Awaiting {
    tail: String,
    cursor_after: i32,
}

#[derive(Debug, Clone, Copy)]
struct DiagEntry {
    t: i64,
    what: &'static str,
    a: i32,
    b: i32,
    c: i32,
}

/// State machine deciding when and what the host may write. See the module docs.
#[derive(Debug, Clone)]
pub struct Session {
    enable_ng: bool,
    instant_undo: bool,

    field_on: bool,
    /// Last selection reported by the keyboard channel.
    sel: (i32, i32),
    /// Selection callbacks arrived in this field. They don't in every field (measured on
    /// Android 16: none for key events, none for Samsung Keyboard in no-suggestion
    /// fields), so they are a hint: without them the cursor is proven by the text event
    /// tail, and composing is inferred from the shape of the text events.
    seen_sel: bool,
    composing: bool,
    /// The keyboard composed at least once in this field: use settle-before-convert.
    keyboard_composes: bool,
    last_activity: i64,
    /// The last user insert was one plain character (no replacement).
    last_insert_single: bool,
    /// User inserts seen in this field, for [`Session::keyboard_mode`].
    inserts: u32,

    pending: Option<Pending>,
    retries: u8,
    echoes: Vec<Echo>,
    undo: Option<Undo>,
    restore: Option<Undo>,
    /// Absolute offsets of pairs the user kept via instant undo.
    kept: Vec<i32>,
    awaiting: Option<Awaiting>,
    writes: VecDeque<i64>,

    diag: VecDeque<DiagEntry>,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    /// A session with no field attached (switched off until [`Session::start_input`]).
    pub fn new() -> Self {
        Self {
            enable_ng: false,
            instant_undo: true,
            field_on: false,
            sel: (-1, -1),
            seen_sel: false,
            composing: false,
            keyboard_composes: false,
            last_activity: i64::MIN / 2,
            last_insert_single: false,
            inserts: 0,
            pending: None,
            retries: 0,
            echoes: Vec::new(),
            undo: None,
            restore: None,
            kept: Vec::new(),
            awaiting: None,
            writes: VecDeque::new(),
            diag: VecDeque::with_capacity(DIAG_CAPACITY),
        }
    }

    /// Update the user's settings without touching the field state.
    pub fn configure(&mut self, enable_ng: bool, instant_undo: bool) {
        self.enable_ng = enable_ng;
        self.instant_undo = instant_undo;
        if !instant_undo {
            self.undo = None;
            self.restore = None;
        }
    }

    /// A new field gained focus. `enabled` is false for fields the host never converts
    /// (passwords, excluded apps...).
    pub fn start_input(&mut self, enabled: bool, sel_start: i32, sel_end: i32, now: i64) {
        let (enable_ng, instant_undo) = (self.enable_ng, self.instant_undo);
        let diag = std::mem::take(&mut self.diag);
        *self = Self::new();
        self.enable_ng = enable_ng;
        self.instant_undo = instant_undo;
        self.diag = diag;
        self.field_on = enabled;
        self.sel = (sel_start, sel_end);
        self.log(now, "start", enabled as i32, sel_start, sel_end);
    }

    /// The field lost focus. Pending conversions are dropped: the connection is gone.
    pub fn finish_input(&mut self, now: i64) {
        self.log(now, "finish", self.pending.is_some() as i32, 0, 0);
        self.field_on = false;
        self.pending = None;
        self.undo = None;
        self.restore = None;
        self.awaiting = None;
    }

    /// How the keyboard typed into the current field: 1 when it composes words (conversion
    /// waits for the word to end, no instant undo), 0 when it commits key by key (live
    /// conversion), -1 when too little was typed to tell.
    pub fn keyboard_mode(&self) -> i32 {
        if self.keyboard_composes {
            1
        } else if self.inserts >= 3 {
            0
        } else {
            -1
        }
    }

    /// Whether the session will still write to the current field.
    pub fn is_field_on(&self) -> bool {
        self.field_on
    }

    /// The keyboard channel reported a selection / composing-region change
    /// (`comp_start`/`comp_end` are -1 when there is no composing region).
    pub fn on_selection(
        &mut self,
        sel_start: i32,
        sel_end: i32,
        comp_start: i32,
        comp_end: i32,
        now: i64,
    ) -> Tick {
        self.sel = (sel_start, sel_end);
        self.seen_sel = true;
        self.composing = comp_start >= 0 && comp_end > comp_start;
        if self.composing {
            self.keyboard_composes = true;
        }
        self.last_activity = now;
        let low = sel_start.min(sel_end);
        self.kept.retain(|&k| k < low);
        self.log(
            now,
            "sel",
            sel_start,
            sel_end,
            if self.composing {
                comp_end - comp_start
            } else {
                -1
            },
        );
        self.schedule()
    }

    /// The editor reported a text change: `removed` units at `from` replaced by `added`.
    /// `tail` is the text right before `from + added` as the event reported it (up to
    /// [`TAIL_LEN`] characters), or empty when the event carried no text.
    pub fn on_text_changed(
        &mut self,
        from: i32,
        removed: i32,
        added: i32,
        tail: &str,
        now: i64,
    ) -> Tick {
        if !self.field_on {
            return Tick::Idle;
        }
        self.last_activity = now;

        if self.echo_clobbered(from, removed, added, now) {
            // Our commit replaced a composing region the keyboard opened between our read
            // and our write: a keystroke was overwritten. Stop touching this field.
            self.log(now, "echo-clobber", from, removed, added);
            self.switch_off(now, "clobber");
            return Tick::Idle;
        }
        if self.consume_echo(from, removed, added, now) {
            self.log(now, "echo", from, removed, added);
            return self.schedule();
        }
        self.log(now, "text", from, removed, added);

        if from < 0 {
            // Without a position nothing can be verified: give up on what was pending.
            self.drop_pending(now, "nofrom");
            self.undo = None;
            return Tick::Idle;
        }

        if let Some(u) = self.undo.take() {
            if self.instant_undo && removed == 1 && added == 0 && from == u.pos {
                self.log(now, "undo", u.pos, 0, 0);
                self.restore = Some(u);
                return Tick::After(0);
            }
        }

        if removed > 0 {
            self.kept.retain(|&k| k < from);
        }
        // N units replaced by N + 1 at the same place is setComposingText growing a word:
        // the keyboard composes even if no selection callback ever says so.
        if removed > 0 && added == removed + 1 {
            self.keyboard_composes = true;
        }

        if added > 0 {
            let end = from + added;
            let start = self.pending.as_ref().map_or(from, |p| p.from.min(from));
            self.pending = Some(Pending {
                from: start,
                end,
                tail: last_chars(tail, TAIL_LEN),
            });
            self.retries = 0;
            self.last_insert_single = added == 1 && removed == 0;
            self.inserts = self.inserts.saturating_add(1);
        } else if let Some(p) = self.pending.as_mut() {
            if from < p.end {
                p.end = from;
            }
            if p.from > p.end {
                p.from = p.end;
            }
            p.tail = last_chars(tail, TAIL_LEN);
        }
        self.schedule()
    }

    /// The host's timer fired.
    pub fn on_tick(&mut self, now: i64) -> Tick {
        if !self.field_on {
            return Tick::Idle;
        }
        if self.restore.is_some() {
            return Tick::Snapshot;
        }
        if self.pending.is_none() || self.composing {
            return Tick::Idle;
        }
        if self.keyboard_composes {
            let quiet = now - self.last_activity;
            if quiet < SETTLE_MS {
                return Tick::After(SETTLE_MS - quiet);
            }
        }
        Tick::Snapshot
    }

    /// A snapshot of the field: `before` is the text right before the cursor (up to
    /// [`SNAPSHOT_LEN`] units), `collapsed` whether the selection is a plain cursor, and
    /// `abs_cursor` the cursor's absolute offset, or -1 when the editor doesn't report it.
    pub fn on_snapshot(
        &mut self,
        before: &str,
        collapsed: bool,
        abs_cursor: i32,
        now: i64,
    ) -> SnapshotResult {
        if !self.field_on {
            return SnapshotResult::Done;
        }
        if !collapsed {
            self.log(now, "snap-sel", abs_cursor, 0, 0);
            self.drop_pending(now, "selected");
            self.restore = None;
            return SnapshotResult::Done;
        }
        if abs_cursor >= 0
            && self.seen_sel
            && (self.sel.0 != abs_cursor || self.sel.1 != abs_cursor)
        {
            // The two channels disagree: some callback is still in flight.
            self.log(now, "snap-stale", abs_cursor, self.sel.0, self.sel.1);
            return self.retry(now);
        }
        let cursor = if abs_cursor >= 0 {
            abs_cursor
        } else if self.seen_sel {
            self.sel.0
        } else {
            // Neither channel gives a position: assume the end of the last insert. The
            // tail check below is what proves it.
            match &self.pending {
                Some(p) if self.restore.is_none() => p.end,
                _ => {
                    self.restore = None;
                    return SnapshotResult::Done;
                }
            }
        };
        if cursor < 0 || self.seen_sel && self.sel.0 != self.sel.1 && abs_cursor < 0 {
            return self.retry(now);
        }

        let chars: Vec<char> = before.chars().collect();
        let at = utf16_offsets(&chars);
        let len16 = at[chars.len()];
        // Absolute offset of `chars[i]`.
        let abs = |i: usize| cursor - (len16 - at[i]);

        if let Some(u) = self.restore.take() {
            if cursor != u.pos {
                self.log(now, "undo-stale", cursor, u.pos, 0);
                return SnapshotResult::Done;
            }
            self.kept.push(u.pos);
            let tail_start = chars.len().saturating_sub(VERIFY_CONTEXT);
            let context: String = chars[tail_start..].iter().collect();
            let plan = Plan {
                mode: WriteMode::Insert,
                start: -1,
                end: -1,
                expected: String::new(),
                replacement: u.original.clone(),
                verify_len: utf16_len(&context) + utf16_len(&u.original),
                kinds: [0; KIND_COUNT],
            };
            self.expect_write(now, cursor, "", &u.original, context);
            self.log(now, "write-undo", cursor, 0, utf16_len(&u.original));
            return SnapshotResult::Write(plan);
        }

        let Some(p) = self.pending.clone() else {
            return SnapshotResult::Done;
        };
        if cursor != p.end {
            self.log(now, "snap-moved", cursor, p.end, 0);
            return self.retry(now);
        }
        // The snapshot must show what the last text event showed. Without absolute
        // offsets this is the only proof the cursor is where we think it is.
        if p.tail.is_empty() && abs_cursor < 0 {
            self.log(now, "snap-blind", cursor, 0, 0);
            self.drop_pending(now, "blind");
            return SnapshotResult::Done;
        }
        if !before.ends_with(p.tail.as_str()) {
            self.log(now, "snap-tail", cursor, utf16_len(&p.tail), 0);
            return self.retry(now);
        }
        // A composing keyboard without selection callbacks: its composing region can't be
        // seen, so only finished words are converted (composing never spans a delimiter).
        if !self.seen_sel
            && self.keyboard_composes
            && !before.chars().last().is_some_and(is_word_delimiter)
        {
            self.log(now, "snap-word", cursor, 0, 0);
            return SnapshotResult::Done;
        }

        // Start of the pending text inside the snapshot, widened to its word's start so a
        // pair split across the boundary is still seen.
        let region_from = len16 - (cursor - p.from).clamp(0, len16);
        let mut seg = at.partition_point(|&o| o < region_from).min(chars.len());
        while seg > 0 && !is_word_delimiter(chars[seg - 1]) {
            seg -= 1;
        }

        let edits: Vec<_> = plan_edits(&chars[seg..], self.enable_ng, false)
            .into_iter()
            .filter(|e| !self.kept.contains(&abs(seg + e.start)))
            .collect();
        if edits.is_empty() {
            self.pending = None;
            return SnapshotResult::Done;
        }

        if !self.breaker_allows(now) {
            return SnapshotResult::Done;
        }

        // Replace from the start of the first converted word, so a composing region the
        // keyboard may still hold on that word is covered as a whole.
        let first = seg + edits[0].start;
        let mut ws = first;
        while ws > seg && !is_word_delimiter(chars[ws - 1]) {
            ws -= 1;
        }

        let mut replacement = String::new();
        let mut kinds = [0u32; KIND_COUNT];
        let mut pos = ws;
        let mut undo: Option<Undo> = None;
        for e in &edits {
            let (es, ee) = (seg + e.start, seg + e.end);
            replacement.extend(&chars[pos..es]);
            let at_repl = abs(ws) + utf16_len(&replacement);
            replacement.push_str(e.replacement);
            if let Some(k) = e.kind {
                kinds[k as usize] += 1;
            }
            undo = Some(Undo {
                pos: at_repl,
                original: chars[es..ee].iter().collect(),
            });
            pos = ee;
        }
        replacement.extend(&chars[pos..]);
        let expected: String = chars[ws..].iter().collect();

        // Only a single pair typed key by key, right at the cursor, is undoable: Backspace
        // after a word-sized conversion should just delete.
        let last = edits[edits.len() - 1];
        let undoable = self.instant_undo
            && !self.keyboard_composes
            && self.last_insert_single
            && edits.len() == 1
            && seg + last.end == chars.len()
            && last.replacement.chars().count() == 1;

        // Kept pairs after a converted pair move with the text.
        let delta_before = |k: i32| -> i32 {
            edits
                .iter()
                .filter(|e| abs(seg + e.start) < k)
                .map(|e| utf16_len(e.replacement) - (at[seg + e.end] - at[seg + e.start]))
                .sum()
        };
        let start_abs = abs(ws);
        self.kept = self
            .kept
            .iter()
            .map(|&k| {
                if k > start_abs {
                    k + delta_before(k)
                } else {
                    k
                }
            })
            .collect();

        let context_start = ws.saturating_sub(VERIFY_CONTEXT);
        let context: String = chars[context_start..ws].iter().collect();
        let known = abs_cursor >= 0;
        let plan = Plan {
            mode: WriteMode::Delete,
            start: if known { start_abs } else { -1 },
            end: if known { cursor } else { -1 },
            verify_len: utf16_len(&context) + utf16_len(&replacement),
            expected: expected.clone(),
            replacement: replacement.clone(),
            kinds,
        };
        self.expect_write(now, start_abs, &expected, &replacement, context);
        self.pending = None;
        self.retries = 0;
        self.undo = if undoable { undo } else { None };
        self.log(
            now,
            if known { "write" } else { "write-rel" },
            start_abs,
            utf16_len(&expected),
            utf16_len(&replacement),
        );
        SnapshotResult::Write(plan)
    }

    /// The host applied the last plan and read back the text before the cursor
    /// (`readback`, `None` when the read failed) and the absolute cursor (-1 if unknown).
    /// Returns false when the write could not be confirmed; the field is then switched off.
    pub fn on_written(&mut self, readback: Option<&str>, abs_cursor: i32, now: i64) -> bool {
        let Some(a) = self.awaiting.take() else {
            return true;
        };
        let verdict = match readback {
            Some(text) => match text.rfind(a.tail.as_str()) {
                Some(i) => {
                    let trailing = utf16_len(&text[i + a.tail.len()..]);
                    let cursor_ok = abs_cursor < 0 || abs_cursor - a.cursor_after == trailing;
                    if trailing == 0 && cursor_ok {
                        Verdict::Ok
                    } else if trailing <= VERIFY_SLACK && cursor_ok {
                        // A keystroke landed between the write and the read-back.
                        Verdict::Raced
                    } else {
                        Verdict::Bad
                    }
                }
                None => Verdict::Bad,
            },
            None => Verdict::Bad,
        };
        match verdict {
            Verdict::Ok => {
                self.log(now, "verify-ok", a.cursor_after, 0, 0);
                true
            }
            Verdict::Raced => {
                self.log(now, "verify-raced", a.cursor_after, abs_cursor, 0);
                true
            }
            Verdict::Bad => {
                self.log(
                    now,
                    "verify-FAIL",
                    a.cursor_after,
                    abs_cursor,
                    readback.map_or(-1, utf16_len),
                );
                self.switch_off(now, "verify");
                false
            }
        }
    }

    /// The last [`DIAG_CAPACITY`] decisions, one per line, without any text content.
    pub fn diagnostics(&self) -> String {
        let mut out = String::new();
        for d in &self.diag {
            let _ = writeln!(out, "{} {} {} {} {}", d.t, d.what, d.a, d.b, d.c);
        }
        out
    }

    fn schedule(&self) -> Tick {
        if !self.field_on {
            return Tick::Idle;
        }
        if self.restore.is_some() {
            return Tick::After(0);
        }
        if self.pending.is_none() || self.composing {
            return Tick::Idle;
        }
        Tick::After(if self.keyboard_composes { SETTLE_MS } else { 0 })
    }

    fn retry(&mut self, now: i64) -> SnapshotResult {
        self.retries += 1;
        if self.retries > MAX_RETRIES {
            self.drop_pending(now, "retries");
            return SnapshotResult::Done;
        }
        SnapshotResult::Retry(RETRY_MS)
    }

    fn drop_pending(&mut self, now: i64, why: &'static str) {
        if self.pending.take().is_some() {
            self.log(now, why, -1, 0, 0);
        }
        self.retries = 0;
    }

    fn switch_off(&mut self, now: i64, why: &'static str) {
        self.log(now, "field-off", 0, 0, 0);
        self.log(now, why, 0, 0, 0);
        self.field_on = false;
        self.pending = None;
        self.undo = None;
        self.restore = None;
    }

    fn breaker_allows(&mut self, now: i64) -> bool {
        while self
            .writes
            .front()
            .is_some_and(|&t| now - t > BREAKER_WINDOW_MS)
        {
            self.writes.pop_front();
        }
        if self.writes.len() >= BREAKER_MAX_WRITES {
            self.switch_off(now, "breaker");
            return false;
        }
        self.writes.push_back(now);
        true
    }

    /// Remember the event(s) a write of `replacement` over `expected` at `start` causes.
    fn expect_write(
        &mut self,
        now: i64,
        start: i32,
        expected: &str,
        replacement: &str,
        context: String,
    ) {
        let old = utf16_len(expected);
        let new = utf16_len(replacement);
        let (p, s) = common_affixes(expected, replacement);
        let mut shapes = vec![(start, old, new), (start + p, old - p - s, new - p - s)];
        let mut then = None;
        if old > 0 {
            shapes.push((start, old, 0));
            then = Some((start, 0, new));
        }
        self.echoes.retain(|e| e.expires > now);
        self.echoes.push(Echo {
            shapes,
            then,
            expires: now + ECHO_TTL_MS,
        });
        self.awaiting = Some(Awaiting {
            tail: context + replacement,
            cursor_after: start + new,
        });
    }

    /// The insertion half of a split write arrived replacing text instead of inserting.
    fn echo_clobbered(&mut self, from: i32, removed: i32, added: i32, now: i64) -> bool {
        removed > 0
            && self
                .echoes
                .iter()
                .any(|e| e.expires > now && e.then.is_none() && e.shapes == [(from, 0, added)])
    }

    fn consume_echo(&mut self, from: i32, removed: i32, added: i32, now: i64) -> bool {
        self.echoes.retain(|e| e.expires > now);
        let shape = (from, removed, added);
        let Some(i) = self.echoes.iter().position(|e| e.shapes.contains(&shape)) else {
            return false;
        };
        let echo = self.echoes.remove(i);
        if added == 0 && removed > 0 {
            if let Some(next) = echo.then {
                self.echoes.push(Echo {
                    shapes: vec![next],
                    then: None,
                    expires: echo.expires,
                });
            }
        }
        true
    }

    fn log(&mut self, t: i64, what: &'static str, a: i32, b: i32, c: i32) {
        if self.diag.len() == DIAG_CAPACITY {
            self.diag.pop_front();
        }
        self.diag.push_back(DiagEntry { t, what, a, b, c });
    }
}

enum Verdict {
    Ok,
    Raced,
    Bad,
}

fn last_chars(s: &str, n: usize) -> String {
    let count = s.chars().count();
    s.chars().skip(count.saturating_sub(n)).collect()
}

fn utf16_len(s: &str) -> i32 {
    s.encode_utf16().count() as i32
}

/// UTF-16 offset of each char, plus the total length at the end.
fn utf16_offsets(chars: &[char]) -> Vec<i32> {
    let mut at = Vec::with_capacity(chars.len() + 1);
    let mut o = 0i32;
    for c in chars {
        at.push(o);
        o += c.len_utf16() as i32;
    }
    at.push(o);
    at
}

/// UTF-16 lengths of the common prefix and (non-overlapping) common suffix.
fn common_affixes(a: &str, b: &str) -> (i32, i32) {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let p = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let max_s = a.len().min(b.len()) - p;
    let s = a
        .iter()
        .rev()
        .zip(b.iter().rev())
        .take(max_s)
        .take_while(|(x, y)| x == y)
        .count();
    let len = |c: &[char]| c.iter().map(|c| c.len_utf16() as i32).sum::<i32>();
    (len(&a[..p]), len(&a[a.len() - s..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        let mut s = Session::new();
        s.start_input(true, 0, 0, 0);
        s
    }

    #[test]
    fn plain_keyboard_converts_pair_immediately_and_undoes() {
        let mut s = session();
        s.on_selection(1, 1, -1, -1, 9);
        assert_eq!(s.on_text_changed(0, 0, 1, "s", 10), Tick::After(0));
        assert_eq!(s.on_tick(10), Tick::Snapshot);
        assert_eq!(s.on_snapshot("s", true, 1, 10), SnapshotResult::Done);
        s.on_selection(2, 2, -1, -1, 19);
        assert_eq!(s.on_text_changed(1, 0, 1, "sh", 20), Tick::After(0));
        assert_eq!(s.on_tick(20), Tick::Snapshot);
        let SnapshotResult::Write(plan) = s.on_snapshot("sh", true, 2, 20) else {
            panic!("expected a write");
        };
        assert_eq!((plan.mode, plan.start, plan.end), (WriteMode::Delete, 0, 2));
        assert_eq!(plan.replacement, "ş");
        assert!(s.on_written(Some("ş"), 1, 21));
        // Echo of our own edit is ignored...
        assert_eq!(s.on_text_changed(0, 2, 1, "ş", 22), Tick::Idle);
        s.on_selection(1, 1, -1, -1, 23);
        // ...and Backspace on the converted letter restores the pair.
        s.on_selection(0, 0, -1, -1, 99);
        assert_eq!(s.on_text_changed(0, 1, 0, "", 100), Tick::After(0));
        assert_eq!(s.on_tick(100), Tick::Snapshot);
        let SnapshotResult::Write(plan) = s.on_snapshot("", true, 0, 100) else {
            panic!("expected the undo write");
        };
        assert_eq!(
            (plan.mode, plan.replacement.as_str()),
            (WriteMode::Insert, "sh")
        );
    }

    #[test]
    fn unknown_offset_uses_delete_mode() {
        let mut s = session();
        s.on_selection(7, 7, -1, -1, 0);
        s.on_text_changed(0, 0, 7, "shahar ", 1);
        let SnapshotResult::Write(plan) = s.on_snapshot("shahar ", true, -1, 50) else {
            panic!("expected a write");
        };
        assert_eq!(plan.mode, WriteMode::Delete);
        assert_eq!(
            (plan.expected.as_str(), plan.replacement.as_str()),
            ("shahar ", "şahar ")
        );
    }

    #[test]
    fn stale_selection_retries_instead_of_writing() {
        let mut s = session();
        s.on_selection(6, 6, -1, -1, 0);
        s.on_text_changed(0, 0, 7, "shahar ", 1);
        assert_eq!(
            s.on_snapshot("shahar ", true, 7, 50),
            SnapshotResult::Retry(RETRY_MS)
        );
    }

    #[test]
    fn utf16_offsets_across_surrogates() {
        let mut s = session();
        s.on_selection(5, 5, -1, -1, 0);
        s.on_text_changed(2, 0, 3, "😀sh ", 1);
        let SnapshotResult::Write(plan) = s.on_snapshot("😀sh ", true, 5, 50) else {
            panic!("expected a write");
        };
        // The emoji is part of the word, so the replacement starts with it.
        assert_eq!((plan.start, plan.end), (0, 5));
        assert_eq!(plan.replacement, "😀ş ");
    }

    #[test]
    fn failed_readback_switches_the_field_off() {
        let mut s = session();
        s.on_selection(7, 7, -1, -1, 0);
        s.on_text_changed(0, 0, 7, "shahar ", 1);
        assert!(matches!(
            s.on_snapshot("shahar ", true, 7, 50),
            SnapshotResult::Write(_)
        ));
        assert!(!s.on_written(Some("shahar şahar "), 13, 51));
        assert!(!s.is_field_on());
        assert_eq!(s.on_text_changed(13, 0, 1, "c", 60), Tick::Idle);
    }

    #[test]
    fn keyboard_mode_from_event_shapes() {
        let mut s = session();
        assert_eq!(s.keyboard_mode(), -1);
        s.on_text_changed(0, 0, 1, "s", 1);
        s.on_text_changed(1, 0, 1, "sa", 2);
        s.on_text_changed(2, 0, 1, "sal", 3);
        assert_eq!(s.keyboard_mode(), 0);
        s.on_text_changed(0, 3, 4, "salo", 4);
        assert_eq!(s.keyboard_mode(), 1);
    }

    #[test]
    fn affixes() {
        assert_eq!(common_affixes("shahar ", "şahar "), (0, 5));
        assert_eq!(common_affixes("ish", "iş"), (1, 0));
    }
}
