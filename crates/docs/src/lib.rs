//! Harfiy document conversion.
//!
//! Converts whole files to the new Uzbek Latin orthography without touching their layout:
//! plain text, HTML, RTF, Office Open XML (`.docx`, `.xlsx`, `.pptx`) and OpenDocument
//! (`.odt`, `.ods`, `.odp`). Only the characters of the text are changed; fonts, styles,
//! tables, images, fields and every other byte of the file stay as they were. The result is
//! always written to a new file next to the original.

mod html;
mod markup;
mod planner;
mod rtf;
mod sniff;
mod text;
mod xml;
mod zipdoc;

use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use sniff::Content;
use zipdoc::Family;

const MIB: u64 = 1024 * 1024;

/// Largest plain text, HTML or RTF file we load into memory.
pub const MAX_TEXT_FILE_BYTES: u64 = 64 * MIB;
/// Largest Office/OpenDocument file we accept (the media inside is never decompressed).
pub const MAX_ARCHIVE_FILE_BYTES: u64 = 256 * MIB;

// Safety limits: a damaged or hostile file must end in an error, never in exhausted memory.
/// Edits kept in memory per text part or file.
pub(crate) const MAX_EDITS: usize = 4_000_000;
/// Characters of uninterrupted text planned at once.
pub(crate) const MAX_GROUP_CHARS: usize = 1 << 20;
/// Entries in a ZIP container.
pub(crate) const MAX_ARCHIVE_ENTRIES: usize = 50_000;
/// Decompressed size of one XML part, and of all XML parts together.
pub(crate) const MAX_PART_BYTES: u64 = 64 * MIB;
pub(crate) const MAX_TOTAL_PART_BYTES: u64 = 256 * MIB;

/// Conversion rules, mirroring the options of the live converter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConvertOptions {
    /// Convert `ng` to `ñ`.
    pub enable_ng: bool,
    /// Convert a letter-apostrophe-letter apostrophe to `ʻ` (tutuq belgisi).
    pub smart_apostrophe: bool,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            enable_ng: false,
            smart_apostrophe: true,
        }
    }
}

#[derive(Debug)]
pub enum DocError {
    /// The extension is not one we can convert.
    UnsupportedFormat(String),
    Io(io::Error),
    /// A text file that is not UTF-8 or UTF-16.
    Encoding,
    /// The file is damaged or not what its extension says.
    Corrupt(String),
    /// Password-protected, or an old binary Office format.
    Encrypted,
    /// The content is a different kind of file than the extension claims.
    WrongType {
        extension: String,
        found: &'static str,
    },
    /// The file, or something inside it, exceeds a safety limit.
    TooLarge(String),
    /// The converter hit an unexpected internal error; the file was left untouched.
    Internal,
    /// The caller stopped the conversion through the progress callback.
    Cancelled,
}

impl fmt::Display for DocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DocError::UnsupportedFormat(ext) => match ext.as_str() {
                "" => write!(f, "Fayl kengaytmasi yo'q"),
                "doc" | "xls" | "ppt" => write!(
                    f,
                    "Eski .{0} formati qo'llab-quvvatlanmaydi. Faylni .{0}x formatida saqlang",
                    ext
                ),
                "pdf" => write!(f, "PDF fayllarni o'girib bo'lmaydi"),
                other => write!(f, "Qo'llab-quvvatlanmaydigan format: .{}", other),
            },
            DocError::Io(e) => write!(f, "Faylni o'qish yoki yozishda xatolik: {}", e),
            DocError::Encoding => write!(f, "Fayl kodlashi UTF-8 yoki UTF-16 emas"),
            DocError::Corrupt(msg) => write!(f, "Fayl buzilgan yoki formati noto'g'ri: {}", msg),
            DocError::Encrypted => write!(
                f,
                "Fayl parol bilan himoyalangan yoki eski ikkilik formatda"
            ),
            DocError::WrongType { extension, found } => write!(
                f,
                "Fayl kengaytmasi .{}, lekin mazmuni boshqa: {}. Fayl turini tekshiring",
                extension, found
            ),
            DocError::TooLarge(what) => write!(f, "Fayl juda katta yoki murakkab: {}", what),
            DocError::Internal => write!(
                f,
                "O'girishda kutilmagan xatolik yuz berdi. Fayl buzilgan bo'lishi mumkin"
            ),
            DocError::Cancelled => write!(f, "Bekor qilindi"),
        }
    }
}

impl std::error::Error for DocError {}

impl From<io::Error> for DocError {
    fn from(e: io::Error) -> Self {
        DocError::Io(e)
    }
}

#[derive(Clone, Copy)]
enum Kind {
    PlainText,
    Html,
    Rtf,
    Ooxml,
    OpenDocument,
}

const KINDS: &[(Kind, &[&str])] = &[
    (
        Kind::PlainText,
        &[
            "txt", "md", "markdown", "csv", "tsv", "srt", "vtt", "log", "rst", "lrc",
        ],
    ),
    (Kind::Html, &["html", "htm", "xhtml"]),
    (Kind::Rtf, &["rtf"]),
    (
        Kind::Ooxml,
        &[
            "docx", "docm", "dotx", "dotm", "xlsx", "xlsm", "xltx", "xltm", "pptx", "pptm", "potx",
            "potm", "ppsx", "ppsm",
        ],
    ),
    (
        Kind::OpenDocument,
        &["odt", "ods", "odp", "ott", "ots", "otp"],
    ),
];

fn kind_of(extension: &str) -> Option<Kind> {
    KINDS
        .iter()
        .find(|(_, extensions)| extensions.contains(&extension))
        .map(|(kind, _)| *kind)
}

/// Lowercase file extensions (without the dot) that can be converted.
pub fn supported_extensions() -> Vec<&'static str> {
    KINDS
        .iter()
        .flat_map(|(_, extensions)| extensions.iter().copied())
        .collect()
}

fn extension_of(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

/// Whether `path` has an extension that can be converted.
pub fn is_supported(path: &Path) -> bool {
    kind_of(&extension_of(path)).is_some()
}

/// Result of converting a file's contents in memory.
#[derive(Debug)]
pub struct Converted {
    /// The converted file. Identical to the input when `conversions` is 0.
    pub data: Vec<u8>,
    pub conversions: usize,
}

/// Size limit for a file of the given kind.
fn size_limit(kind: Kind) -> u64 {
    match kind {
        Kind::Ooxml | Kind::OpenDocument => MAX_ARCHIVE_FILE_BYTES,
        Kind::PlainText | Kind::Html | Kind::Rtf => MAX_TEXT_FILE_BYTES,
    }
}

fn check_size(kind: Kind, size: u64) -> Result<(), DocError> {
    let limit = size_limit(kind);
    if size > limit {
        return Err(DocError::TooLarge(format!(
            "fayl hajmi {} MB dan oshmasligi kerak",
            limit / MIB
        )));
    }
    Ok(())
}

/// The content (magic bytes) must be what the extension claims, so a renamed or damaged file
/// is rejected up front instead of being parsed as something it is not.
fn check_content(kind: Kind, extension: &str, data: &[u8]) -> Result<(), DocError> {
    let found = sniff::sniff(data);
    let accepted = match kind {
        Kind::Ooxml | Kind::OpenDocument => match found {
            Content::Zip => true,
            Content::Ole => return Err(DocError::Encrypted),
            Content::Empty => return Err(DocError::Corrupt("fayl bo'sh".to_string())),
            _ => false,
        },
        Kind::Rtf => match found {
            Content::Rtf => true,
            Content::Empty => return Err(DocError::Corrupt("fayl bo'sh".to_string())),
            _ => false,
        },
        Kind::PlainText | Kind::Html => matches!(found, Content::Text | Content::Empty),
    };
    if accepted {
        Ok(())
    } else {
        Err(DocError::WrongType {
            extension: extension.to_string(),
            found: found.label(),
        })
    }
}

fn convert_kind(
    kind: Kind,
    extension: &str,
    data: &[u8],
    opts: ConvertOptions,
    progress: &mut dyn FnMut(f32) -> bool,
) -> Result<(Vec<u8>, usize), DocError> {
    match kind {
        Kind::PlainText => text::convert_plain(extension, data, opts),
        Kind::Html => html::convert_html_bytes(data, opts),
        Kind::Rtf => rtf::convert_rtf(data, opts),
        Kind::Ooxml => zipdoc::convert_archive(data, Family::Ooxml, opts, progress),
        Kind::OpenDocument => zipdoc::convert_archive(data, Family::OpenDocument, opts, progress),
    }
}

/// Convert the contents of a file of type `extension` (with or without a leading dot).
///
/// The size and the content are validated first. Conversion code is written to never panic;
/// as a last line of defence a panic is turned into [`DocError::Internal`] where the build
/// unwinds (a release build with `panic = "abort"` cannot recover from one).
pub fn convert_bytes(
    extension: &str,
    data: &[u8],
    opts: &ConvertOptions,
) -> Result<Converted, DocError> {
    convert_bytes_with_progress(extension, data, opts, &mut |_| true)
}

/// [`convert_bytes`] that reports how far along it is.
///
/// `progress` is called with a fraction between 0 and 1 while the file is processed (once per
/// part for Office and OpenDocument files, which are the ones that take long) and stops the
/// conversion with [`DocError::Cancelled`] when it returns `false`.
pub fn convert_bytes_with_progress(
    extension: &str,
    data: &[u8],
    opts: &ConvertOptions,
    progress: &mut dyn FnMut(f32) -> bool,
) -> Result<Converted, DocError> {
    let extension = extension.trim_start_matches('.').to_ascii_lowercase();
    let Some(kind) = kind_of(&extension) else {
        return Err(DocError::UnsupportedFormat(extension));
    };
    check_size(kind, data.len() as u64)?;
    check_content(kind, &extension, data)?;

    let opts = *opts;
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        convert_kind(kind, &extension, data, opts, progress)
    }));
    match outcome {
        Ok(result) => result.map(|(data, conversions)| Converted { data, conversions }),
        Err(_) => Err(DocError::Internal),
    }
}

/// Outcome of converting one file on disk.
#[derive(Debug)]
pub struct FileReport {
    /// The new file, or `None` when the file had nothing to convert (nothing is written).
    pub output: Option<PathBuf>,
    pub conversions: usize,
}

/// `name.ext` -> `name.harfiy.ext`
fn suggested_name_for(input: &Path) -> String {
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "hujjat".to_string());
    match input.extension() {
        Some(ext) => format!("{}.harfiy.{}", stem, ext.to_string_lossy()),
        None => format!("{}.harfiy", stem),
    }
}

/// A converted file held in memory, ready to be saved wherever the caller decides.
#[derive(Debug)]
pub struct FileConversion {
    /// The converted file. Identical to the input when `conversions` is 0.
    pub data: Vec<u8>,
    pub conversions: usize,
    /// `name.harfiy.ext`: the name to offer when saving.
    pub suggested_name: String,
}

/// Whether two paths point at the same existing file.
pub fn is_same_file(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// Write `data` into `dir` under `file_name`, or under `name (2).ext`, `name (3).ext`, ... when
/// that name is taken. An existing file is never overwritten. Returns the path written.
pub fn save_unique(dir: &Path, file_name: &str, data: &[u8]) -> io::Result<PathBuf> {
    use std::io::Write;

    let name = Path::new(file_name);
    let stem = name
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "hujjat".to_string());
    let extension = name.extension().map(|e| e.to_string_lossy().into_owned());

    for attempt in 0..1000u32 {
        let candidate = match (attempt, &extension) {
            (0, _) => file_name.to_string(),
            (n, Some(ext)) => format!("{} ({}).{}", stem, n + 1, ext),
            (n, None) => format!("{} ({})", stem, n + 1),
        };
        let path = dir.join(candidate);
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                if let Err(e) = file.write_all(data) {
                    drop(file);
                    let _ = fs::remove_file(&path);
                    return Err(e);
                }
                return Ok(path);
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "bo'sh fayl nomi topilmadi",
    ))
}

/// Read and convert the file at `input` without writing anything. `progress` is described at
/// [`convert_bytes_with_progress`]. The original file is never modified.
pub fn convert_file_to_memory(
    input: &Path,
    opts: &ConvertOptions,
    progress: &mut dyn FnMut(f32) -> bool,
) -> Result<FileConversion, DocError> {
    let extension = extension_of(input);
    let Some(kind) = kind_of(&extension) else {
        return Err(DocError::UnsupportedFormat(extension));
    };

    // Reject directories, devices and oversized files before reading anything, and cap the
    // read itself in case the file grows while we read it.
    let file = fs::File::open(input)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(DocError::Corrupt("bu oddiy fayl emas".to_string()));
    }
    check_size(kind, metadata.len())?;
    let mut data = Vec::with_capacity(metadata.len() as usize);
    file.take(size_limit(kind) + 1).read_to_end(&mut data)?;
    check_size(kind, data.len() as u64)?;

    let converted = convert_bytes_with_progress(&extension, &data, opts, progress)?;
    Ok(FileConversion {
        data: converted.data,
        conversions: converted.conversions,
        suggested_name: suggested_name_for(input),
    })
}

/// Convert the file at `input` and write the result next to it as `name.harfiy.ext`.
/// The original file is never modified.
pub fn convert_file(input: &Path, opts: &ConvertOptions) -> Result<FileReport, DocError> {
    let converted = convert_file_to_memory(input, opts, &mut |_| true)?;
    if converted.conversions == 0 {
        return Ok(FileReport {
            output: None,
            conversions: 0,
        });
    }

    let dir = input.parent().unwrap_or_else(|| Path::new("."));
    let output = save_unique(dir, &converted.suggested_name, &converted.data)?;
    Ok(FileReport {
        output: Some(output),
        conversions: converted.conversions,
    })
}
