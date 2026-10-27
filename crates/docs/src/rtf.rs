//! Text conversion for RTF files.
//!
//! The file is tokenized just enough to find the document text: control words, groups,
//! `\'hh` byte escapes and `\uN` Unicode escapes are understood, so a word split across
//! formatting groups still converts, smart quotes written as `\rquote`, `\'92` or a `\uN`
//! escape are recognized, and font tables, field instructions, pictures and other
//! destinations are never touched. Every byte that is not converted text is copied through
//! unchanged.

use std::borrow::Cow;

use crate::planner::{splice, Planner};
use crate::{ConvertOptions, DocError};

/// Destination groups that hold metadata or binary data, never document text.
const SKIPPED_DESTINATIONS: &[&str] = &[
    "fonttbl",
    "colortbl",
    "stylesheet",
    "info",
    "themedata",
    "colorschememapping",
    "datastore",
    "xmlnstbl",
    "pict",
    "object",
    "objdata",
    "objclass",
    "objname",
    "fldinst",
    "listtable",
    "listoverridetable",
    "rsidtbl",
    "latentstyles",
    "mmathPr",
    "pgdsctbl",
    "revtbl",
    "bkmkstart",
    "bkmkend",
    "wgrffmtfilter",
    "defchp",
    "defpap",
    "filetbl",
    "userprops",
    "blipuid",
    "generator",
    "sp",
    "sn",
    "sv",
    "atnauthor",
    "atnid",
    "atntime",
    "atnref",
];

/// Destination groups that hold document text separate from the surrounding flow.
const TEXT_DESTINATIONS: &[&str] = &[
    "footnote",
    "header",
    "headerl",
    "headerr",
    "headerf",
    "footer",
    "footerl",
    "footerr",
    "footerf",
    "ftnsep",
    "ftnsepc",
    "ftncn",
    "aftnsep",
    "aftnsepc",
    "aftncn",
    "field",
    "fldrslt",
    "shp",
    "shptxt",
    "annotation",
];

/// Control words that stand for a visible character or break: a word never continues across
/// them.
const BREAK_WORDS: &[&str] = &[
    "par",
    "line",
    "tab",
    "cell",
    "nestcell",
    "row",
    "nestrow",
    "sect",
    "page",
    "column",
    "lbr",
    "softline",
    "softcol",
    "softpage",
    "emdash",
    "endash",
    "emspace",
    "enspace",
    "qmspace",
    "bullet",
    "zwj",
    "zwnj",
    "zwbo",
    "zwnbo",
    "ltrmark",
    "rtlmark",
    "chdate",
    "chdpl",
    "chdpa",
    "chtime",
    "chpgn",
    "chftn",
    "chatn",
    "chftnsep",
    "chftnsepc",
];

/// Deepest group nesting we accept. Real documents stay far below this; the limit keeps the
/// group stack small for files made only of `{`.
const MAX_GROUP_DEPTH: usize = 4096;

struct Frame {
    /// Inside a metadata/binary destination: nothing here is document text.
    skip: bool,
    /// A text destination: reading flow is cut at its start and end.
    boundary: bool,
    /// Number of fallback characters after each `\uN` (`\ucN`).
    uc: u8,
}

struct Word<'a> {
    name: &'a str,
    param: Option<i32>,
    /// Index just past the word, its parameter and its single delimiter space.
    end: usize,
}

fn read_name(data: &[u8], at: usize) -> &str {
    let mut end = at;
    while end < data.len() && data[end].is_ascii_alphabetic() {
        end += 1;
    }
    std::str::from_utf8(&data[at..end]).unwrap_or("")
}

/// Parse the control word whose first letter is at `at`.
fn parse_word(data: &[u8], at: usize) -> Word<'_> {
    let name = read_name(data, at);
    let mut pos = at + name.len();

    let mut param = None;
    let negative = data.get(pos) == Some(&b'-');
    let digits_start = if negative { pos + 1 } else { pos };
    let mut digits_end = digits_start;
    while digits_end < data.len() && data[digits_end].is_ascii_digit() {
        digits_end += 1;
    }
    if digits_end > digits_start {
        let value: i64 = std::str::from_utf8(&data[digits_start..digits_end])
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let value = if negative { -value } else { value };
        param = Some(value.clamp(i32::MIN as i64, i32::MAX as i64) as i32);
        pos = digits_end;
    }

    // A single space after a control word belongs to it.
    if data.get(pos) == Some(&b' ') {
        pos += 1;
    }
    Word {
        name,
        param,
        end: pos,
    }
}

/// Skip the `uc` fallback characters that follow a `\uN` escape.
fn skip_fallback(data: &[u8], mut pos: usize, uc: u8) -> usize {
    let mut left = uc;
    while left > 0 && pos < data.len() {
        match data[pos] {
            b'\\' if data.get(pos + 1) == Some(&b'\'') => pos = (pos + 4).min(data.len()),
            b'\\' | b'{' | b'}' => break,
            b'\r' | b'\n' => {
                pos += 1;
                continue;
            }
            _ => pos += 1,
        }
        left -= 1;
    }
    pos
}

/// A `\uN` escape starting at `at`: its UTF-16 code unit and the index after its fallback.
fn parse_unicode(data: &[u8], at: usize, uc: u8) -> Option<(u32, usize)> {
    if data.get(at) != Some(&b'\\') || data.get(at + 1) != Some(&b'u') {
        return None;
    }
    let word = parse_word(data, at + 1);
    if word.name != "u" {
        return None;
    }
    let param = word.param?;
    let code = if param < 0 { param + 65536 } else { param } as u32;
    Some((code, skip_fallback(data, word.end, uc)))
}

/// Characters of the Windows-1252 range 0x80..=0x9F; the rest of 0xA0..=0xFF matches Latin-1.
const CP1252_HIGH: [u32; 32] = [
    0x20AC, 0, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039,
    0x0152, 0, 0x017D, 0, 0, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x02DC,
    0x2122, 0x0161, 0x203A, 0x0153, 0, 0x017E, 0x0178,
];

fn decode_byte(byte: u8, latin: bool) -> Option<char> {
    match byte {
        0x00..=0x7F => Some(byte as char),
        0x80..=0x9F if latin => {
            char::from_u32(CP1252_HIGH[(byte - 0x80) as usize]).filter(|&c| c != '\0')
        }
        0xA0..=0xFF if latin => Some(byte as char),
        _ => None,
    }
}

fn quote_char(name: &str) -> Option<char> {
    match name {
        "lquote" => Some('\u{2018}'),
        "rquote" => Some('\u{2019}'),
        "ldblquote" => Some('\u{201C}'),
        "rdblquote" => Some('\u{201D}'),
        _ => None,
    }
}

/// Replacement text as RTF: non-ASCII characters become `\uN` plus `uc` fallback characters.
fn encode_rtf(text: &str, uc: u8) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        match ch {
            '\\' | '{' | '}' => {
                out.push('\\');
                out.push(ch);
            }
            c if c.is_ascii() => out.push(c),
            c => {
                let mut units = [0u16; 2];
                for unit in c.encode_utf16(&mut units).iter() {
                    out.push_str(&format!("\\u{}", *unit as i16));
                    if uc == 0 {
                        out.push(' ');
                    } else {
                        out.push_str(&"?".repeat(uc as usize));
                    }
                }
            }
        }
    }
    out
}

pub(crate) fn convert_rtf(data: &[u8], opts: ConvertOptions) -> Result<(Vec<u8>, usize), DocError> {
    let start = data
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(data.len());
    if !data[start..].starts_with(b"{\\rtf") {
        return Err(DocError::Corrupt("RTF hujjati emas".to_string()));
    }

    let encode = |text: &'static str, uc: u8| Cow::Owned(encode_rtf(text, uc));
    let mut planner = Planner::new(opts, &encode);
    let mut stack = vec![Frame {
        skip: false,
        boundary: false,
        uc: 1,
    }];
    // Bytes above 0x7F in `\'hh` are decoded as Windows-1252 unless \ansicpg says otherwise.
    let mut latin = true;
    let mut i = 0;

    while i < data.len() {
        let skip = stack.last().is_some_and(|f| f.skip);
        let uc = stack.last().map_or(1, |f| f.uc);

        match data[i] {
            b'{' => {
                let mut frame = Frame {
                    skip,
                    boundary: false,
                    uc,
                };
                if data.get(i + 1) == Some(&b'\\') {
                    match data.get(i + 2) {
                        Some(b'*') => frame.skip = true,
                        Some(c) if c.is_ascii_alphabetic() => {
                            let name = read_name(data, i + 2);
                            if SKIPPED_DESTINATIONS.contains(&name) {
                                frame.skip = true;
                            } else if TEXT_DESTINATIONS.contains(&name) {
                                frame.boundary = true;
                            }
                        }
                        _ => {}
                    }
                }
                if frame.skip || frame.boundary {
                    planner.flush();
                }
                if stack.len() >= MAX_GROUP_DEPTH {
                    return Err(DocError::Corrupt(
                        "RTF guruhlari haddan tashqari chuqur ichma-ich joylashgan".to_string(),
                    ));
                }
                stack.push(frame);
                i += 1;
            }
            b'}' => {
                if stack.len() > 1 {
                    if let Some(frame) = stack.pop() {
                        if frame.skip || frame.boundary {
                            planner.flush();
                        }
                    }
                }
                i += 1;
            }
            b'\\' => {
                let Some(&next) = data.get(i + 1) else {
                    break;
                };
                match next {
                    b'\\' | b'{' | b'}' => {
                        if !skip {
                            planner.push_aux(next as char, i..i + 2, uc);
                        }
                        i += 2;
                    }
                    b'\'' => {
                        let hex = data
                            .get(i + 2..i + 4)
                            .and_then(|h| std::str::from_utf8(h).ok())
                            .and_then(|h| u8::from_str_radix(h, 16).ok());
                        match hex {
                            Some(byte) => {
                                if !skip {
                                    match decode_byte(byte, latin) {
                                        Some(c) => planner.push_aux(c, i..i + 4, uc),
                                        None => planner.flush(),
                                    }
                                }
                                i += 4;
                            }
                            None => {
                                planner.flush();
                                i += 2;
                            }
                        }
                    }
                    b'*' => i += 2,
                    b'a'..=b'z' | b'A'..=b'Z' => {
                        let word = parse_word(data, i + 1);
                        match (word.name, word.param) {
                            ("u", Some(_)) => {
                                let (code, mut end) =
                                    parse_unicode(data, i, uc).unwrap_or((0xFFFD, word.end));
                                let mut ch = char::from_u32(code);
                                // A high surrogate pairs with the \uN that follows it.
                                if (0xD800..0xDC00).contains(&code) {
                                    if let Some((low, low_end)) = parse_unicode(data, end, uc) {
                                        if (0xDC00..0xE000).contains(&low) {
                                            ch = char::from_u32(
                                                0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00),
                                            );
                                            end = low_end;
                                        }
                                    }
                                }
                                if !skip {
                                    match ch {
                                        Some(c) => planner.push_aux(c, i..end, uc),
                                        None => planner.flush(),
                                    }
                                }
                                i = end;
                            }
                            ("uc", Some(n)) => {
                                if let Some(frame) = stack.last_mut() {
                                    frame.uc = n.clamp(0, 255) as u8;
                                }
                                i = word.end;
                            }
                            ("ansicpg", Some(n)) => {
                                latin = n == 1252;
                                i = word.end;
                            }
                            ("bin", Some(n)) => {
                                planner.flush();
                                i = word.end.saturating_add(n.max(0) as usize).min(data.len());
                            }
                            (name, _) => {
                                if let Some(c) = quote_char(name) {
                                    if !skip {
                                        planner.push_aux(c, i..word.end, uc);
                                    }
                                } else if BREAK_WORDS.contains(&name) {
                                    planner.flush();
                                }
                                i = word.end;
                            }
                        }
                    }
                    // \~ \- \_ \: \| and "backslash + line break" (= \par) stand for characters.
                    _ => {
                        planner.flush();
                        i += 2;
                    }
                }
            }
            // Line breaks in the file are layout only, not text.
            b'\r' | b'\n' => i += 1,
            byte @ 0x00..=0x7F => {
                if !skip {
                    planner.push_aux(byte as char, i..i + 1, uc);
                }
                i += 1;
            }
            _ => {
                planner.flush();
                i += 1;
            }
        }
    }

    let (replacements, count) = planner.finish()?;
    if count == 0 {
        return Ok((data.to_vec(), 0));
    }
    Ok((splice(data, &replacements), count))
}
