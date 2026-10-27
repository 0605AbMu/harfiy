//! Maps conversion edits planned over a logical character stream back onto the bytes of the
//! source file, so everything that is not converted text stays byte-for-byte identical.

use std::borrow::Cow;
use std::ops::Range;

use harfiy_engine::plan_edits;

use crate::{ConvertOptions, DocError, MAX_EDITS, MAX_GROUP_CHARS};

/// A source byte range and the text that replaces it.
pub(crate) type Replacement = (Range<usize>, Cow<'static, str>);

struct Unit {
    ch: char,
    span: Range<usize>,
    aux: u8,
}

/// Collects the logical characters of one text run group and plans conversions over them.
///
/// Every logical character remembers the byte span it occupies in the source (an entity such
/// as `&#39;` or an RTF escape such as `\'92` is one character spanning several bytes). A
/// group is a stretch of text that reads continuously, even if it is split across several
/// runs or formatting groups; `flush` ends the group wherever the reading flow is broken.
pub(crate) struct Planner<'a> {
    opts: ConvertOptions,
    encode: &'a dyn Fn(&'static str, u8) -> Cow<'static, str>,
    units: Vec<Unit>,
    replacements: Vec<Replacement>,
    count: usize,
    /// Set when the file needs more edits than we are willing to hold in memory.
    overflow: bool,
}

impl<'a> Planner<'a> {
    /// `encode` turns replacement text into the source format's syntax; it also receives the
    /// `aux` value of the character it replaces (RTF uses it for the `\uc` fallback length).
    pub fn new(
        opts: ConvertOptions,
        encode: &'a dyn Fn(&'static str, u8) -> Cow<'static, str>,
    ) -> Self {
        Self {
            opts,
            encode,
            units: Vec::new(),
            replacements: Vec::new(),
            count: 0,
            overflow: false,
        }
    }

    pub fn push(&mut self, ch: char, span: Range<usize>) {
        self.push_aux(ch, span, 0);
    }

    pub fn push_aux(&mut self, ch: char, span: Range<usize>, aux: u8) {
        // A group is normally a paragraph. A single run of text without any break that is
        // longer than this is cut here, so memory use stays bounded whatever the file holds.
        if self.units.len() >= MAX_GROUP_CHARS {
            // Cut after the last whitespace: no rule spans whitespace, so nothing is lost.
            // Without any whitespace (one giant token) the cut is made right here.
            let keep_from = self
                .units
                .iter()
                .rposition(|u| u.ch.is_whitespace())
                .map_or(self.units.len(), |i| i + 1);
            let tail = self.units.split_off(keep_from);
            self.flush();
            self.units = tail;
        }
        self.units.push(Unit { ch, span, aux });
    }

    /// End the current group: plan its conversions and start a new one.
    pub fn flush(&mut self) {
        if self.units.is_empty() {
            return;
        }
        if self.overflow {
            self.units.clear();
            return;
        }
        let chars: Vec<char> = self.units.iter().map(|u| u.ch).collect();
        let edits = plan_edits(&chars, self.opts.enable_ng, self.opts.smart_apostrophe);
        if self.count + edits.len() > MAX_EDITS {
            self.overflow = true;
            self.replacements = Vec::new();
            self.units.clear();
            return;
        }
        for edit in edits {
            let first = &self.units[edit.start];
            let encoded = (self.encode)(edit.replacement, first.aux);
            self.replacements.push((first.span.clone(), encoded));
            for unit in &self.units[edit.start + 1..edit.end] {
                self.replacements
                    .push((unit.span.clone(), Cow::Borrowed("")));
            }
            self.count += 1;
        }
        self.units.clear();
    }

    /// Replacements in ascending source order, and the number of conversions made.
    pub fn finish(mut self) -> Result<(Vec<Replacement>, usize), DocError> {
        self.flush();
        if self.overflow {
            return Err(DocError::TooLarge(
                "o'zgartiriladigan joylar soni haddan tashqari ko'p".to_string(),
            ));
        }
        Ok((self.replacements, self.count))
    }
}

/// Apply replacements (ascending, non-overlapping) to `src`.
pub(crate) fn splice(src: &[u8], replacements: &[Replacement]) -> Vec<u8> {
    let mut out = Vec::with_capacity(src.len());
    let mut pos = 0;
    for (range, text) in replacements {
        if range.start < pos {
            continue; // overlapping span; never produced by the planner
        }
        out.extend_from_slice(&src[pos..range.start]);
        out.extend_from_slice(text.as_bytes());
        pos = range.end;
    }
    out.extend_from_slice(&src[pos..]);
    out
}

/// `splice` for text sources. All spans start and end on character boundaries.
pub(crate) fn splice_str(src: &str, replacements: &[Replacement]) -> String {
    let mut out = String::with_capacity(src.len());
    let mut pos = 0;
    for (range, text) in replacements {
        if range.start < pos {
            continue;
        }
        out.push_str(&src[pos..range.start]);
        out.push_str(text);
        pos = range.end;
    }
    out.push_str(&src[pos..]);
    out
}
