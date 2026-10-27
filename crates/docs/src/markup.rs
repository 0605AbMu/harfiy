//! Small helpers shared by the XML and HTML scanners: tag parsing and entity-aware text.

use std::ops::Range;

use crate::planner::Planner;

pub(crate) struct Tag<'a> {
    pub name: &'a str,
    pub closing: bool,
    pub self_closing: bool,
    /// Byte index of the `>` that ends the tag.
    pub end: usize,
}

/// Longest tag we look for the closing `>` of. Real tags are far shorter; the limit keeps a
/// file full of stray `<` from being rescanned to its end again and again.
const MAX_TAG_LEN: usize = 64 * 1024;

/// Index of the `>` closing the tag that starts at `start`, skipping quoted attribute values
/// (which may legally contain `>`).
fn find_tag_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut quote: Option<u8> = None;
    let limit = bytes.len().min(start.saturating_add(MAX_TAG_LEN));
    for (offset, &c) in bytes[start + 1..limit].iter().enumerate() {
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                }
            }
            None => match c {
                b'"' | b'\'' => quote = Some(c),
                b'>' => return Some(start + 1 + offset),
                _ => {}
            },
        }
    }
    None
}

/// Finds tags in a document. It remembers where the next `>` is, so a file full of `<` that
/// are not tags is scanned once overall instead of once per `<`.
pub(crate) struct TagScanner<'a> {
    src: &'a str,
    /// Position of a `>` such that no other `>` lies between the last query and it.
    next_gt: Option<usize>,
    /// From this index on there is no `>` at all (`usize::MAX` while unknown).
    no_gt_from: usize,
}

impl<'a> TagScanner<'a> {
    pub fn new(src: &'a str) -> Self {
        Self {
            src,
            next_gt: None,
            no_gt_from: usize::MAX,
        }
    }

    fn gt_at_or_after(&mut self, from: usize) -> Option<usize> {
        if from >= self.no_gt_from {
            return None;
        }
        if let Some(gt) = self.next_gt {
            if gt >= from {
                return Some(gt);
            }
        }
        match self.src.as_bytes()[from..].iter().position(|&b| b == b'>') {
            Some(offset) => {
                self.next_gt = Some(from + offset);
                self.next_gt
            }
            None => {
                self.no_gt_from = from;
                None
            }
        }
    }

    /// Parse the tag whose `<` is at `start`. Returns `None` for something that is not a tag.
    pub fn parse(&mut self, start: usize) -> Option<Tag<'a>> {
        let src = self.src;
        let bytes = src.as_bytes();
        let mut pos = start + 1;
        let closing = bytes.get(pos) == Some(&b'/');
        if closing {
            pos += 1;
        }
        // Text such as "a < b" or "<3" is rejected here without scanning for a `>`.
        match bytes.get(pos) {
            Some(&c) if c.is_ascii_alphabetic() || c == b'_' || c == b':' || c >= 0x80 => {}
            _ => return None,
        }
        // No `>` close enough: not a tag, and no need to look any further.
        let nearest_gt = self.gt_at_or_after(start + 1)?;
        if nearest_gt - start > MAX_TAG_LEN {
            return None;
        }
        let end = find_tag_end(bytes, start)?;
        let name_start = pos;
        while pos < end && !bytes[pos].is_ascii_whitespace() && bytes[pos] != b'/' {
            pos += 1;
        }
        if pos == name_start {
            return None;
        }
        Some(Tag {
            name: &src[name_start..pos],
            closing,
            self_closing: !closing && bytes[end - 1] == b'/',
            end,
        })
    }
}

/// Index just past the next occurrence of `pattern` at or after `from` (end of input if none).
pub(crate) fn skip_past(src: &str, from: usize, pattern: &str) -> usize {
    match src[from..].find(pattern) {
        Some(p) => from + p + pattern.len(),
        None => src.len(),
    }
}

/// Byte index of the next ASCII case-insensitive occurrence of `needle` at or after `from`.
pub(crate) fn find_ignore_case(hay: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() || from > hay.len() - needle.len() {
        return None;
    }
    (from..=hay.len() - needle.len())
        .find(|&i| hay[i..i + needle.len()].eq_ignore_ascii_case(needle))
}

pub(crate) fn xml_named_entity(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => None,
    }
}

pub(crate) fn html_named_entity(name: &str) -> Option<char> {
    xml_named_entity(name).or(match name {
        "nbsp" => Some('\u{a0}'),
        "lsquo" => Some('\u{2018}'),
        "rsquo" => Some('\u{2019}'),
        "sbquo" => Some('\u{201a}'),
        "ldquo" => Some('\u{201c}'),
        "rdquo" => Some('\u{201d}'),
        "bdquo" => Some('\u{201e}'),
        "ndash" => Some('\u{2013}'),
        "mdash" => Some('\u{2014}'),
        "hellip" => Some('\u{2026}'),
        "laquo" => Some('\u{ab}'),
        "raquo" => Some('\u{bb}'),
        "bull" => Some('\u{2022}'),
        "middot" => Some('\u{b7}'),
        "ouml" => Some('ö'),
        "Ouml" => Some('Ö'),
        "ccedil" => Some('ç'),
        "Ccedil" => Some('Ç'),
        "ntilde" => Some('ñ'),
        "Ntilde" => Some('Ñ'),
        _ => None,
    })
}

fn decode_entity(name: &str, named: fn(&str) -> Option<char>) -> Option<char> {
    if let Some(num) = name.strip_prefix('#') {
        let code = match num.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => num.parse::<u32>().ok()?,
        };
        return char::from_u32(code).filter(|&c| c != '\0');
    }
    named(name)
}

/// Feed the text in `range` of `src` to the planner as logical characters, decoding entities
/// (`&#39;`, `&rsquo;`, ...). An entity that cannot be decoded ends the current group, so a
/// word is never joined across something we do not understand.
pub(crate) fn push_text(
    planner: &mut Planner,
    src: &str,
    range: Range<usize>,
    named: fn(&str) -> Option<char>,
) {
    let base = range.start;
    let text = &src[range];
    let mut iter = text.char_indices().peekable();

    while let Some((idx, ch)) = iter.next() {
        if ch != '&' {
            planner.push(ch, base + idx..base + idx + ch.len_utf8());
            continue;
        }

        // Look for the `;` only a few bytes ahead, so text full of bare `&` stays linear.
        let rest = &text[idx + 1..];
        let window = &rest.as_bytes()[..rest.len().min(11)];
        let entity = window
            .iter()
            .position(|&b| b == b';')
            .filter(|&p| p >= 1)
            .map(|p| &rest[..p])
            .filter(|name| name.chars().all(|c| c.is_ascii_alphanumeric() || c == '#'));

        match entity {
            Some(name) => {
                let end = idx + 1 + name.len() + 1;
                match decode_entity(name, named) {
                    Some(decoded) => planner.push(decoded, base + idx..base + end),
                    None => planner.flush(),
                }
                while iter.peek().is_some_and(|&(j, _)| j < end) {
                    iter.next();
                }
            }
            // A bare ampersand is ordinary text.
            None => planner.push('&', base + idx..base + idx + 1),
        }
    }
}
