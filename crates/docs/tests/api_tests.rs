use std::io::{Cursor, Write};
use std::path::PathBuf;

use harfiy_docs::{
    convert_bytes_with_progress, convert_file_to_memory, is_same_file, save_unique, ConvertOptions,
    DocError,
};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

fn opts() -> ConvertOptions {
    ConvertOptions::default()
}

/// A docx with `parts` extra stored parts, so the converter has several steps to report.
fn docx_with_parts(parts: usize) -> Vec<u8> {
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer.start_file("[Content_Types].xml", options).unwrap();
    writer.write_all(b"<Types/>").unwrap();
    writer.start_file("word/document.xml", options).unwrap();
    writer
        .write_all(b"<w:p><w:r><w:t>shahar</w:t></w:r></w:p>")
        .unwrap();
    for i in 0..parts {
        writer
            .start_file(format!("word/media/image{i}.bin"), options)
            .unwrap();
        writer.write_all(b"binary").unwrap();
    }
    writer.finish().unwrap().into_inner()
}

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("harfiy-docs-api-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn progress_is_reported_part_by_part() {
    let docx = docx_with_parts(8);
    let mut fractions = Vec::new();
    let converted = convert_bytes_with_progress("docx", &docx, &opts(), &mut |f| {
        fractions.push(f);
        true
    })
    .unwrap();

    assert_eq!(converted.conversions, 1);
    assert_eq!(fractions.len(), 10, "one report per archive entry");
    assert_eq!(fractions[0], 0.0);
    assert!(fractions.windows(2).all(|w| w[0] < w[1]), "{fractions:?}");
    assert!(fractions.iter().all(|&f| (0.0..1.0).contains(&f)));
}

#[test]
fn returning_false_from_the_progress_callback_cancels() {
    let docx = docx_with_parts(8);
    let mut calls = 0;
    let result = convert_bytes_with_progress("docx", &docx, &opts(), &mut |f| {
        calls += 1;
        f < 0.3
    });
    assert!(matches!(result, Err(DocError::Cancelled)));
    assert!(
        calls < 10,
        "conversion must stop at once, not run to the end"
    );
    assert_eq!(DocError::Cancelled.to_string(), "Bekor qilindi");
}

#[test]
fn single_pass_formats_still_convert_with_a_callback() {
    let converted = convert_bytes_with_progress("txt", b"shahar", &opts(), &mut |_| true).unwrap();
    assert_eq!(converted.conversions, 1);
}

#[test]
fn convert_file_to_memory_leaves_the_disk_untouched() {
    let dir = TempDir::new("memory");
    let input = dir.0.join("hisobot.md");
    std::fs::write(&input, "shahar choy").unwrap();

    let done = convert_file_to_memory(&input, &opts(), &mut |_| true).unwrap();
    assert_eq!(done.conversions, 2);
    assert_eq!(done.suggested_name, "hisobot.harfiy.md");
    assert_eq!(String::from_utf8(done.data).unwrap(), "şahar çoy");

    // Nothing was written next to the original.
    assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 1);
    assert_eq!(std::fs::read_to_string(&input).unwrap(), "shahar choy");
}

#[test]
fn save_unique_never_overwrites() {
    let dir = TempDir::new("unique");

    let first = save_unique(&dir.0, "a.harfiy.txt", b"one").unwrap();
    let second = save_unique(&dir.0, "a.harfiy.txt", b"two").unwrap();
    let third = save_unique(&dir.0, "a.harfiy.txt", b"three").unwrap();
    assert_eq!(first, dir.0.join("a.harfiy.txt"));
    assert_eq!(second, dir.0.join("a.harfiy (2).txt"));
    assert_eq!(third, dir.0.join("a.harfiy (3).txt"));
    assert_eq!(std::fs::read(&first).unwrap(), b"one");
    assert_eq!(std::fs::read(&second).unwrap(), b"two");

    let bare_first = save_unique(&dir.0, "noext", b"x").unwrap();
    let bare_second = save_unique(&dir.0, "noext", b"y").unwrap();
    assert_eq!(bare_first, dir.0.join("noext"));
    assert_eq!(bare_second, dir.0.join("noext (2)"));
}

#[test]
fn save_unique_into_a_missing_folder_is_an_error() {
    let dir = TempDir::new("missing");
    let missing = dir.0.join("nope");
    assert!(save_unique(&missing, "a.txt", b"x").is_err());
}

#[test]
fn same_file_is_detected_through_different_spellings() {
    let dir = TempDir::new("same");
    let file = dir.0.join("a.txt");
    let other = dir.0.join("b.txt");
    std::fs::write(&file, "x").unwrap();
    std::fs::write(&other, "x").unwrap();

    let roundabout = dir.0.join(".").join("a.txt");
    assert!(is_same_file(&file, &roundabout));
    assert!(!is_same_file(&file, &other));
    assert!(!is_same_file(&file, &dir.0.join("missing.txt")));
}
