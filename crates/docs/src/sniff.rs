//! Identify what a file really is from its first bytes (magic numbers), so a file is never
//! parsed as a format it is not, whatever its extension says.

/// What the content of a file turned out to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Content {
    Empty,
    /// ZIP container: Office Open XML, OpenDocument, or any other archive.
    Zip,
    /// OLE compound file: legacy .doc/.xls/.ppt or a password-protected Office file.
    Ole,
    Rtf,
    Pdf,
    Image,
    /// Executables and other archives (gzip, 7z, rar, ...).
    Binary,
    /// Anything else that has no NUL bytes: plain text, HTML, CSV, ...
    Text,
}

impl Content {
    /// Short Uzbek description for error messages.
    pub fn label(self) -> &'static str {
        match self {
            Content::Empty => "bo'sh fayl",
            Content::Zip => "ZIP arxivi (hujjat)",
            Content::Ole => "eski Office formati",
            Content::Rtf => "RTF hujjati",
            Content::Pdf => "PDF hujjati",
            Content::Image => "rasm",
            Content::Binary => "ikkilik (binar) fayl",
            Content::Text => "matn",
        }
    }
}

const SIGNATURES: &[(&[u8], Content)] = &[
    (b"PK\x03\x04", Content::Zip),
    (b"PK\x05\x06", Content::Zip), // empty archive
    (b"PK\x07\x08", Content::Zip), // spanned archive
    (
        &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1],
        Content::Ole,
    ),
    (b"%PDF-", Content::Pdf),
    (b"\x89PNG\r\n\x1a\n", Content::Image),
    (&[0xFF, 0xD8, 0xFF], Content::Image),
    (b"GIF87a", Content::Image),
    (b"GIF89a", Content::Image),
    (b"II*\x00", Content::Image),
    (b"MM\x00*", Content::Image),
    (b"\x7fELF", Content::Binary),
    (&[0x1F, 0x8B], Content::Binary), // gzip
    (b"7z\xBC\xAF\x27\x1C", Content::Binary),
    (b"Rar!\x1A\x07", Content::Binary),
    (&[0xCF, 0xFA, 0xED, 0xFE], Content::Binary), // Mach-O
    (&[0xCE, 0xFA, 0xED, 0xFE], Content::Binary),
    (&[0xFE, 0xED, 0xFA, 0xCE], Content::Binary),
    (&[0xFE, 0xED, 0xFA, 0xCF], Content::Binary),
    (&[0xCA, 0xFE, 0xBA, 0xBE], Content::Binary),
];

/// How many leading bytes are inspected for NUL bytes.
const TEXT_PROBE: usize = 8192;

pub(crate) fn sniff(data: &[u8]) -> Content {
    if data.is_empty() {
        return Content::Empty;
    }

    // UTF-16 text legitimately contains NUL bytes; recognize it by its BOM first.
    if data.starts_with(&[0xFF, 0xFE]) || data.starts_with(&[0xFE, 0xFF]) {
        return Content::Text;
    }

    if let Some((_, content)) = SIGNATURES.iter().find(|(magic, _)| data.starts_with(magic)) {
        return *content;
    }

    // WebP and other RIFF images; Windows executables ("MZ" header followed by NULs).
    if data.len() >= 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        return Content::Image;
    }

    // RTF may be preceded by whitespace.
    let trimmed = data
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .map_or(&data[..0], |start| &data[start..]);
    if trimmed.starts_with(b"{\\rtf") {
        return Content::Rtf;
    }

    // A real text file has no NUL bytes in its first kilobytes.
    if data[..data.len().min(TEXT_PROBE)].contains(&0) {
        return Content::Binary;
    }
    Content::Text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_common_signatures() {
        assert_eq!(sniff(b""), Content::Empty);
        assert_eq!(sniff(b"PK\x03\x04rest"), Content::Zip);
        assert_eq!(sniff(b"%PDF-1.7"), Content::Pdf);
        assert_eq!(sniff(b"  \r\n{\\rtf1\\ansi"), Content::Rtf);
        assert_eq!(sniff(b"\x89PNG\r\n\x1a\nrest"), Content::Image);
        assert_eq!(sniff(b"MZ\x90\x00\x03\x00\x00\x00"), Content::Binary);
        assert_eq!(sniff(b"salom dunyo"), Content::Text);
        assert_eq!(sniff(b"<html>shahar</html>"), Content::Text);
    }

    #[test]
    fn utf16_with_bom_is_text() {
        assert_eq!(sniff(&[0xFF, 0xFE, b's', 0, b'h', 0]), Content::Text);
    }

    #[test]
    fn text_that_merely_starts_like_a_signature_is_still_text() {
        assert_eq!(sniff(b"MZ is a nickname"), Content::Text);
        assert_eq!(sniff(b"BM broker"), Content::Text);
    }
}
