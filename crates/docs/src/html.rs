//! Text conversion for HTML files. Tags, attributes, comments, scripts, styles and code
//! blocks are left untouched; only visible text is converted.

use std::borrow::Cow;

use crate::markup::{find_ignore_case, html_named_entity, push_text, skip_past, TagScanner};
use crate::planner::{splice_str, Planner};
use crate::{ConvertOptions, DocError};

/// Elements whose content is not prose and must never be converted.
const RAW_ELEMENTS: &[&str] = &["script", "style", "code", "kbd", "samp"];

/// Inline elements: a word may continue across their start and end tags.
const INLINE_ELEMENTS: &[&str] = &[
    "a", "abbr", "acronym", "b", "bdi", "bdo", "big", "cite", "data", "del", "dfn", "em", "font",
    "i", "ins", "label", "mark", "nobr", "q", "s", "small", "span", "strike", "strong", "sub",
    "sup", "time", "tt", "u", "wbr",
];

/// The charset an HTML or XML prolog declares: `Some(true)` for UTF-8.
fn declares_utf8(html: &str) -> Option<bool> {
    let head: String = html
        .chars()
        .take(4096)
        .collect::<String>()
        .to_ascii_lowercase();
    let value = ["charset", "encoding"].iter().find_map(|key| {
        let after = &head[head.find(key)? + key.len()..];
        let after = after.trim_start().strip_prefix('=')?;
        Some(after.trim_start_matches([' ', '"', '\'']).to_string())
    })?;
    Some(value.starts_with("utf-8") || value.starts_with("utf8"))
}

/// Non-ASCII replacement text as numeric character references, valid in any charset.
fn to_numeric_references(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_ascii() {
                c.to_string()
            } else {
                format!("&#{};", c as u32)
            }
        })
        .collect()
}

/// Convert the visible text of an HTML document. Returns `None` when nothing needed converting.
pub(crate) fn convert_html(
    html: &str,
    opts: ConvertOptions,
) -> Result<Option<(String, usize)>, DocError> {
    // A page that declares a legacy charset (or declares nothing and is pure ASCII) may be
    // read by a browser in a charset that has no ş or ğ, so write references instead.
    let use_references = match declares_utf8(html) {
        Some(utf8) => !utf8,
        None => html.is_ascii(),
    };
    let encode = move |text: &'static str, _aux: u8| {
        if use_references {
            Cow::Owned(to_numeric_references(text))
        } else {
            Cow::Borrowed(text)
        }
    };

    let mut planner = Planner::new(opts, &encode);
    let bytes = html.as_bytes();
    let mut scanner = TagScanner::new(html);
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] != b'<' {
            let end = html[i..].find('<').map_or(bytes.len(), |p| i + p);
            push_text(&mut planner, html, i..end, html_named_entity);
            i = end;
            continue;
        }

        let rest = &html[i..];
        if rest.starts_with("<!--") {
            planner.flush();
            i = skip_past(html, i + 4, "-->");
            continue;
        }
        if rest.starts_with("<!") || rest.starts_with("<?") {
            planner.flush();
            i = skip_past(html, i + 2, ">");
            continue;
        }

        let starts_tag = bytes
            .get(i + 1)
            .is_some_and(|&c| c.is_ascii_alphabetic() || c == b'/');
        let Some(tag) = starts_tag.then(|| scanner.parse(i)).flatten() else {
            // A lone '<' is just text.
            planner.push('<', i..i + 1);
            i += 1;
            continue;
        };

        let name = tag.name.to_ascii_lowercase();
        if !tag.closing && !tag.self_closing && RAW_ELEMENTS.contains(&name.as_str()) {
            planner.flush();
            let closer = format!("</{}", name);
            i = match find_ignore_case(bytes, tag.end + 1, closer.as_bytes()) {
                Some(p) => skip_past(html, p, ">"),
                None => bytes.len(),
            };
            continue;
        }
        if !INLINE_ELEMENTS.contains(&name.as_str()) {
            planner.flush();
        }
        i = tag.end + 1;
    }

    let (replacements, count) = planner.finish()?;
    if count == 0 {
        return Ok(None);
    }
    Ok(Some((splice_str(html, &replacements), count)))
}

/// File-level entry point: the file must be UTF-8 (a UTF-8 BOM is kept).
pub(crate) fn convert_html_bytes(
    data: &[u8],
    opts: ConvertOptions,
) -> Result<(Vec<u8>, usize), DocError> {
    let (bom, body) = match data.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        Some(rest) => (true, rest),
        None => (false, data),
    };
    let html = std::str::from_utf8(body).map_err(|_| DocError::Encoding)?;

    let Some((converted, count)) = convert_html(html, opts)? else {
        return Ok((data.to_vec(), 0));
    };
    let mut out = Vec::with_capacity(converted.len() + 3);
    if bom {
        out.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    }
    out.extend_from_slice(converted.as_bytes());
    Ok((out, count))
}
