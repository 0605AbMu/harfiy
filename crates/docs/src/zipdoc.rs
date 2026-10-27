//! Office Open XML (.docx/.xlsx/.pptx) and OpenDocument (.odt/.ods/.odp) containers.
//!
//! Only the XML parts that carry text are rewritten. Every other entry (styles, media,
//! relationships, the ODF `mimetype`, ...) is copied across in its original compressed form,
//! so formatting and embedded content are preserved exactly.

use std::io::{Cursor, Read, Write};

use zip::{ZipArchive, ZipWriter};

use crate::xml::{convert_xml, Mode};
use crate::{ConvertOptions, DocError, MAX_ARCHIVE_ENTRIES, MAX_PART_BYTES, MAX_TOTAL_PART_BYTES};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Family {
    Ooxml,
    OpenDocument,
}

/// Word and DrawingML (PowerPoint, charts, shapes): text in `<w:t>` / `<a:t>`.
static OOXML_TEXT: Mode = Mode::Elements {
    text: &["w:t", "a:t"],
    boundary: &["w:p", "a:p"],
    breaks: &[
        "w:tab",
        "w:br",
        "w:cr",
        "w:noBreakHyphen",
        "w:softHyphen",
        "w:sym",
        "w:ptab",
        "w:drawing",
        "w:pict",
        "w:object",
        "w:footnoteReference",
        "w:endnoteReference",
        "w:commentReference",
        "w:footnoteRef",
        "w:endnoteRef",
        "w:annotationRef",
        "w:fldChar",
        "w:instrText",
        "w:fldSimple",
        "w:delText",
        "w:separator",
        "w:continuationSeparator",
        "w:pgNum",
        "w:txbxContent",
        "mc:AlternateContent",
        "mc:Choice",
        "mc:Fallback",
        "a:br",
        "a:fld",
    ],
};

/// Excel shared strings, inline strings and cell comments: text in `<t>`.
static XLSX_TEXT: Mode = Mode::Elements {
    text: &["t"],
    boundary: &["si", "is", "text"],
    breaks: &["rPh", "phoneticPr"],
};

/// OpenDocument content and styles (headers/footers): mixed content in `<text:p>`/`<text:h>`.
static ODF_TEXT: Mode = Mode::Mixed {
    paragraphs: &["text:p", "text:h"],
    inline: &[
        "text:span",
        "text:a",
        "text:bookmark",
        "text:bookmark-start",
        "text:bookmark-end",
        "text:soft-page-break",
        "text:change",
        "text:change-start",
        "text:change-end",
    ],
};

fn mode_for(family: Family, name: &str) -> Option<&'static Mode> {
    match family {
        Family::OpenDocument => matches!(name, "content.xml" | "styles.xml").then_some(&ODF_TEXT),
        Family::Ooxml => {
            if !name.ends_with(".xml") {
                return None;
            }
            if name == "xl/sharedStrings.xml"
                || name.starts_with("xl/worksheets/")
                || name.starts_with("xl/comments")
            {
                Some(&XLSX_TEXT)
            } else if name.starts_with("word/")
                || name.starts_with("ppt/")
                || name.starts_with("xl/")
            {
                Some(&OOXML_TEXT)
            } else {
                None
            }
        }
    }
}

fn corrupt(e: impl std::fmt::Display) -> DocError {
    DocError::Corrupt(e.to_string())
}

/// The manifest is a small index; anything bigger than this is not a real one.
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;

pub(crate) fn convert_archive(
    data: &[u8],
    family: Family,
    opts: ConvertOptions,
    progress: &mut dyn FnMut(f32) -> bool,
) -> Result<(Vec<u8>, usize), DocError> {
    let mut archive = ZipArchive::new(Cursor::new(data)).map_err(corrupt)?;
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err(DocError::TooLarge(
            "arxivdagi fayllar soni haddan tashqari ko'p".to_string(),
        ));
    }

    // A ZIP is only a document of the claimed kind if it has that kind's mandatory part.
    let (marker, kind) = match family {
        Family::Ooxml => ("[Content_Types].xml", "Office Open XML"),
        Family::OpenDocument => ("content.xml", "OpenDocument"),
    };
    if archive.by_name(marker).is_err() {
        return Err(DocError::Corrupt(format!(
            "{} hujjatining \"{}\" qismi topilmadi",
            kind, marker
        )));
    }

    if family == Family::OpenDocument {
        if let Ok(manifest) = archive.by_name("META-INF/manifest.xml") {
            let mut text = String::new();
            if manifest
                .take(MAX_MANIFEST_BYTES)
                .read_to_string(&mut text)
                .is_ok()
                && text.contains("encryption-data")
            {
                return Err(DocError::Encrypted);
            }
        }
    }

    let mut writer = ZipWriter::new(Cursor::new(Vec::with_capacity(data.len())));
    let mut total = 0;
    let mut decompressed_total = 0u64;

    let entry_count = archive.len();
    for index in 0..entry_count {
        if !progress(index as f32 / entry_count as f32) {
            return Err(DocError::Cancelled);
        }
        let (name, is_dir, options) = {
            let entry = archive.by_index_raw(index).map_err(corrupt)?;
            (
                entry.name().map_err(corrupt)?.into_owned(),
                entry.is_dir(),
                entry.options(),
            )
        };

        if !is_dir {
            if let Some(mode) = mode_for(family, &name) {
                // The declared size can lie, so the read itself is capped (zip bombs).
                let mut bytes = Vec::new();
                archive
                    .by_index(index)
                    .map_err(corrupt)?
                    .take(MAX_PART_BYTES + 1)
                    .read_to_end(&mut bytes)
                    .map_err(corrupt)?;
                decompressed_total += bytes.len() as u64;
                if bytes.len() as u64 > MAX_PART_BYTES || decompressed_total > MAX_TOTAL_PART_BYTES
                {
                    return Err(DocError::TooLarge(format!(
                        "\"{}\" ichidagi matn qismi haddan tashqari katta",
                        name
                    )));
                }

                if let Some((converted, count)) = std::str::from_utf8(&bytes)
                    .ok()
                    .map(|xml| convert_xml(xml, mode, opts))
                    .transpose()?
                    .flatten()
                {
                    writer.start_file(name, options).map_err(corrupt)?;
                    writer.write_all(converted.as_bytes()).map_err(corrupt)?;
                    total += count;
                    continue;
                }
            }
        }

        let entry = archive.by_index_raw(index).map_err(corrupt)?;
        writer.raw_copy_file(entry).map_err(corrupt)?;
    }

    let cursor = writer.finish().map_err(corrupt)?;
    Ok((cursor.into_inner(), total))
}
