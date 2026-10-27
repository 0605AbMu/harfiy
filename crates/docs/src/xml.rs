//! Text conversion inside the XML parts of Office Open XML and OpenDocument files.
//!
//! Only the characters of text nodes are touched. Tags, attributes, formatting, fields and
//! every other byte of the part are copied through unchanged.

use std::borrow::Cow;

use crate::markup::{push_text, skip_past, xml_named_entity, TagScanner};
use crate::planner::{splice_str, Planner};
use crate::{ConvertOptions, DocError};

/// How the text of an XML dialect is laid out.
pub(crate) enum Mode {
    /// Text lives in dedicated elements (`<w:t>`, `<a:t>`, `<t>`), grouped into paragraphs.
    Elements {
        text: &'static [&'static str],
        /// Elements that start or end a paragraph: words never continue across them.
        boundary: &'static [&'static str],
        /// Inline elements that stand for a character or object (tab, break, field, ...):
        /// words never continue across them either.
        breaks: &'static [&'static str],
    },
    /// Text is mixed content inside paragraph containers (OpenDocument `<text:p>`).
    Mixed {
        paragraphs: &'static [&'static str],
        /// Inline wrappers that are transparent to reading flow (`<text:span>`, ...).
        inline: &'static [&'static str],
    },
}

fn escape_xml(text: &'static str) -> Cow<'static, str> {
    if text.contains(['&', '<', '>']) {
        Cow::Owned(
            text.replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;"),
        )
    } else {
        Cow::Borrowed(text)
    }
}

/// Convert the text of one XML part. Returns `None` when nothing needed converting.
pub(crate) fn convert_xml(
    xml: &str,
    mode: &Mode,
    opts: ConvertOptions,
) -> Result<Option<(String, usize)>, DocError> {
    let encode = |text: &'static str, _aux: u8| escape_xml(text);
    let mut planner = Planner::new(opts, &encode);
    let bytes = xml.as_bytes();
    let mut scanner = TagScanner::new(xml);
    let mut in_paragraph = 0usize;
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] != b'<' {
            let end = xml[i..].find('<').map_or(bytes.len(), |p| i + p);
            if matches!(mode, Mode::Mixed { .. }) && in_paragraph > 0 {
                push_text(&mut planner, xml, i..end, xml_named_entity);
            }
            i = end;
            continue;
        }

        let rest = &xml[i..];
        if rest.starts_with("<!--") {
            i = skip_past(xml, i + 4, "-->");
            continue;
        }
        if rest.starts_with("<![CDATA[") {
            planner.flush();
            i = skip_past(xml, i + 9, "]]>");
            continue;
        }
        if rest.starts_with("<?") {
            i = skip_past(xml, i + 2, "?>");
            continue;
        }
        if rest.starts_with("<!") {
            i = skip_past(xml, i + 2, ">");
            continue;
        }

        let Some(tag) = scanner.parse(i) else {
            planner.flush();
            i += 1;
            continue;
        };

        match mode {
            Mode::Elements {
                text,
                boundary,
                breaks,
            } => {
                if boundary.contains(&tag.name) || breaks.contains(&tag.name) {
                    planner.flush();
                }
                if !tag.closing && !tag.self_closing && text.contains(&tag.name) {
                    let start = tag.end + 1;
                    let end = xml[start..].find('<').map_or(bytes.len(), |p| start + p);
                    push_text(&mut planner, xml, start..end, xml_named_entity);
                    i = end;
                    continue;
                }
            }
            Mode::Mixed { paragraphs, inline } => {
                if paragraphs.contains(&tag.name) {
                    planner.flush();
                    if tag.closing {
                        in_paragraph = in_paragraph.saturating_sub(1);
                    } else if !tag.self_closing {
                        in_paragraph += 1;
                    }
                } else if !inline.contains(&tag.name) {
                    planner.flush();
                }
            }
        }
        i = tag.end + 1;
    }

    let (replacements, count) = planner.finish()?;
    if count == 0 {
        return Ok(None);
    }
    Ok(Some((splice_str(xml, &replacements), count)))
}
