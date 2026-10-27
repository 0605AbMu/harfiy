//! Plain text files (UTF-8 with or without BOM, UTF-16 with BOM).

use std::borrow::Cow;

use crate::planner::{splice_str, Planner};
use crate::{ConvertOptions, DocError};

enum Encoding {
    Utf8 { bom: bool },
    Utf16Le,
    Utf16Be,
}

/// Extensions of files that Windows tools open in the ANSI code page unless they carry a
/// BOM, so converted (non-ASCII) output gets one.
const NEEDS_BOM_EXTENSIONS: &[&str] = &["txt", "csv", "tsv"];

fn decode_utf16(body: &[u8], to_unit: fn([u8; 2]) -> u16) -> Result<String, DocError> {
    if !body.len().is_multiple_of(2) {
        return Err(DocError::Encoding);
    }
    let (pairs, _) = body.as_chunks::<2>();
    let units: Vec<u16> = pairs.iter().map(|&pair| to_unit(pair)).collect();
    String::from_utf16(&units).map_err(|_| DocError::Encoding)
}

/// Convert `text`, planning line by line (a word never spans a line break), so memory use
/// stays proportional to the longest line instead of the whole file.
fn convert_str(text: &str, opts: ConvertOptions) -> Result<Option<(String, usize)>, DocError> {
    let encode = |replacement: &'static str, _aux: u8| Cow::Borrowed(replacement);
    let mut planner = Planner::new(opts, &encode);
    for (index, ch) in text.char_indices() {
        planner.push(ch, index..index + ch.len_utf8());
        if ch == '\n' {
            planner.flush();
        }
    }
    let (replacements, count) = planner.finish()?;
    if count == 0 {
        return Ok(None);
    }
    Ok(Some((splice_str(text, &replacements), count)))
}

pub(crate) fn convert_plain(
    extension: &str,
    data: &[u8],
    opts: ConvertOptions,
) -> Result<(Vec<u8>, usize), DocError> {
    let (encoding, text): (Encoding, Cow<str>) =
        if let Some(body) = data.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
            let text = std::str::from_utf8(body).map_err(|_| DocError::Encoding)?;
            (Encoding::Utf8 { bom: true }, Cow::Borrowed(text))
        } else if let Some(body) = data.strip_prefix(&[0xFF, 0xFE]) {
            (
                Encoding::Utf16Le,
                Cow::Owned(decode_utf16(body, u16::from_le_bytes)?),
            )
        } else if let Some(body) = data.strip_prefix(&[0xFE, 0xFF]) {
            (
                Encoding::Utf16Be,
                Cow::Owned(decode_utf16(body, u16::from_be_bytes)?),
            )
        } else {
            let text = std::str::from_utf8(data).map_err(|_| DocError::Encoding)?;
            (Encoding::Utf8 { bom: false }, Cow::Borrowed(text))
        };

    let Some((converted, count)) = convert_str(&text, opts)? else {
        return Ok((data.to_vec(), 0));
    };

    let out = match encoding {
        Encoding::Utf8 { bom } => {
            let add_bom = bom || (text.is_ascii() && NEEDS_BOM_EXTENSIONS.contains(&extension));
            let mut out = Vec::with_capacity(converted.len() + 3);
            if add_bom {
                out.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
            }
            out.extend_from_slice(converted.as_bytes());
            out
        }
        Encoding::Utf16Le => {
            let mut out = vec![0xFF, 0xFE];
            out.extend(converted.encode_utf16().flat_map(u16::to_le_bytes));
            out
        }
        Encoding::Utf16Be => {
            let mut out = vec![0xFE, 0xFF];
            out.extend(converted.encode_utf16().flat_map(u16::to_be_bytes));
            out
        }
    };
    Ok((out, count))
}
