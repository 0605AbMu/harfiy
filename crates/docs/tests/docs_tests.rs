use std::io::{Cursor, Read, Write};
use std::path::PathBuf;

use harfiy_docs::{convert_bytes, convert_file, is_supported, ConvertOptions, DocError};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

fn opts() -> ConvertOptions {
    ConvertOptions::default()
}

/// Build a zip archive. `stored` entries are written uncompressed (like ODF `mimetype`).
fn make_zip(entries: &[(&str, &str)], stored: &[&str]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    // Every real Office file has [Content_Types].xml (ODF has mimetype); add it when a test omits it.
    let mut entries = entries.to_vec();
    if !entries
        .iter()
        .any(|(n, _)| *n == "[Content_Types].xml" || *n == "mimetype")
    {
        entries.insert(0, ("[Content_Types].xml", "<Types/>"));
    }
    for (name, content) in entries {
        let method = if stored.contains(&name) {
            CompressionMethod::Stored
        } else {
            CompressionMethod::Deflated
        };
        writer
            .start_file(
                name,
                SimpleFileOptions::default().compression_method(method),
            )
            .unwrap();
        writer.write_all(content.as_bytes()).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn entry(zip_bytes: &[u8], name: &str) -> String {
    let mut archive = ZipArchive::new(Cursor::new(zip_bytes)).unwrap();
    let mut text = String::new();
    archive
        .by_name(name)
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    text
}

fn entry_names(zip_bytes: &[u8]) -> Vec<String> {
    let mut archive = ZipArchive::new(Cursor::new(zip_bytes)).unwrap();
    (0..archive.len())
        .map(|i| {
            archive
                .by_index_raw(i)
                .unwrap()
                .name()
                .unwrap()
                .into_owned()
        })
        .collect()
}

// ---------------------------------------------------------------- docx

const DOCX_STYLES: &str = r#"<w:styles><w:style w:styleId="Heading1"><w:rPr><w:rFonts w:ascii="Trebuchet MS"/></w:rPr></w:style></w:styles>"#;

#[test]
fn docx_converts_words_split_across_runs_and_keeps_formatting() {
    let document = concat!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
        r#"<w:document><w:body><w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr>"#,
        r#"<w:r><w:rPr><w:rFonts w:ascii="Trebuchet MS" w:hAnsi="Trebuchet MS"/><w:b/></w:rPr><w:t>s</w:t></w:r>"#,
        r#"<w:proofErr w:type="spellStart"/>"#,
        r#"<w:r><w:rPr><w:i/></w:rPr><w:t xml:space="preserve">hahar o</w:t></w:r>"#,
        r#"<w:r><w:t>&#8217;rmon va g&apos;isht</w:t></w:r>"#,
        r#"<w:r><w:instrText xml:space="preserve"> HYPERLINK "https://shahar.uz/choy" </w:instrText></w:r>"#,
        r#"<w:del><w:r><w:delText>shahar</w:delText></w:r></w:del>"#,
        r#"</w:p></w:body></w:document>"#,
    );
    let input = make_zip(
        &[
            ("[Content_Types].xml", "<Types/>"),
            ("word/document.xml", document),
            ("word/styles.xml", DOCX_STYLES),
        ],
        &[],
    );

    let out = convert_bytes("docx", &input, &opts()).unwrap();
    assert_eq!(out.conversions, 4); // sh, o', g', and the smart apostrophe is part of o'

    let doc = entry(&out.data, "word/document.xml");
    // The digraph "sh" spans two runs: "ş" lands in the first (bold) run, "h" leaves the second.
    assert!(doc.contains(r#"<w:b/></w:rPr><w:t>ş</w:t>"#), "{doc}");
    assert!(
        doc.contains(r#"<w:t xml:space="preserve">ahar ö</w:t>"#),
        "{doc}"
    );
    assert!(doc.contains(r#"<w:t>rmon va ğişt</w:t>"#), "{doc}");
    // Formatting, field instructions and deleted text are untouched.
    assert!(doc.contains(r#"<w:rFonts w:ascii="Trebuchet MS" w:hAnsi="Trebuchet MS"/>"#));
    assert!(doc.contains(r#" HYPERLINK "https://shahar.uz/choy" "#));
    assert!(doc.contains("<w:delText>shahar</w:delText>"));

    // Other parts are copied verbatim and entry order is preserved.
    assert_eq!(entry(&out.data, "word/styles.xml"), DOCX_STYLES);
    assert_eq!(
        entry_names(&out.data),
        [
            "[Content_Types].xml",
            "word/document.xml",
            "word/styles.xml"
        ]
    );
}

#[test]
fn docx_does_not_join_words_across_tabs_and_breaks() {
    let document = concat!(
        r#"<w:document><w:body>"#,
        r#"<w:p><w:r><w:t>s</w:t></w:r><w:r><w:tab/></w:r><w:r><w:t>hahar</w:t></w:r></w:p>"#,
        r#"<w:p><w:r><w:t>c</w:t></w:r></w:p><w:p><w:r><w:t>hoy</w:t></w:r></w:p>"#,
        r#"</w:body></w:document>"#,
    );
    let input = make_zip(&[("word/document.xml", document)], &[]);
    let out = convert_bytes("docx", &input, &opts()).unwrap();
    assert_eq!(out.conversions, 0);
}

#[test]
fn docx_without_changes_is_reported_as_zero_conversions() {
    let input = make_zip(
        &[(
            "word/document.xml",
            "<w:p><w:r><w:t>salom dunyo</w:t></w:r></w:p>",
        )],
        &[],
    );
    assert_eq!(
        convert_bytes("docx", &input, &opts()).unwrap().conversions,
        0
    );
}

#[test]
fn password_protected_office_file_is_rejected() {
    let mut ole = vec![0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
    ole.extend_from_slice(&[0; 64]);
    assert!(matches!(
        convert_bytes("docx", &ole, &opts()),
        Err(DocError::Encrypted)
    ));
}

#[test]
fn truncated_archive_is_reported_as_corrupt() {
    assert!(matches!(
        convert_bytes("docx", b"PK truncated", &opts()),
        Err(DocError::Corrupt(_))
    ));
}

// ---------------------------------------------------------------- xlsx / pptx

#[test]
fn xlsx_converts_strings_but_not_formulas() {
    let shared = concat!(
        r#"<sst><si><t>shahar</t></si>"#,
        r#"<si><r><t>c</t></r><r><rPr><b/></rPr><t>hoy</t></r></si></sst>"#,
    );
    let sheet = concat!(
        r#"<worksheet><sheetData><row><c><f>IF(A1="shahar",1,0)</f></c>"#,
        r#"<c t="inlineStr"><is><t>o'rmon</t></is></c></row></sheetData></worksheet>"#,
    );
    let input = make_zip(
        &[
            ("xl/sharedStrings.xml", shared),
            ("xl/worksheets/sheet1.xml", sheet),
        ],
        &[],
    );

    let out = convert_bytes("xlsx", &input, &opts()).unwrap();
    assert_eq!(out.conversions, 3);

    let shared_out = entry(&out.data, "xl/sharedStrings.xml");
    assert!(shared_out.contains("<t>şahar</t>"), "{shared_out}");
    assert!(shared_out.contains("<t>ç</t></r>"), "{shared_out}");
    assert!(shared_out.contains("<t>oy</t>"), "{shared_out}");

    let sheet_out = entry(&out.data, "xl/worksheets/sheet1.xml");
    assert!(
        sheet_out.contains(r#"<f>IF(A1="shahar",1,0)</f>"#),
        "{sheet_out}"
    );
    assert!(sheet_out.contains("<t>örmon</t>"), "{sheet_out}");
}

#[test]
fn pptx_converts_slide_text() {
    let slide = r#"<p:sld><a:p><a:r><a:rPr lang="uz"/><a:t>ch</a:t></a:r><a:r><a:t>oy</a:t></a:r></a:p></p:sld>"#;
    let input = make_zip(&[("ppt/slides/slide1.xml", slide)], &[]);
    let out = convert_bytes("pptx", &input, &opts()).unwrap();
    assert_eq!(out.conversions, 1);
    let slide_out = entry(&out.data, "ppt/slides/slide1.xml");
    assert!(
        slide_out.contains("<a:t>ç</a:t></a:r><a:r><a:t>oy</a:t>"),
        "{slide_out}"
    );
    assert!(slide_out.contains(r#"<a:rPr lang="uz"/>"#));
}

// ---------------------------------------------------------------- OpenDocument

#[test]
fn odt_converts_text_inside_spans_and_keeps_mimetype_first_and_stored() {
    let content = concat!(
        r#"<office:document-content><office:automatic-styles>"#,
        r#"<style:font-face style:name="Trebuchet MS" svg:font-family="Trebuchet MS"/>"#,
        r#"</office:automatic-styles><office:body><office:text>"#,
        r#"<text:p text:style-name="P1">s<text:span text:style-name="T1">h</text:span>ahar o'rmon</text:p>"#,
        r#"<text:p>c<text:s/>h</text:p>"#,
        r#"</office:text></office:body></office:document-content>"#,
    );
    let input = make_zip(
        &[
            ("mimetype", "application/vnd.oasis.opendocument.text"),
            ("content.xml", content),
            ("META-INF/manifest.xml", "<manifest:manifest/>"),
        ],
        &["mimetype"],
    );

    let out = convert_bytes("odt", &input, &opts()).unwrap();
    assert_eq!(out.conversions, 2);

    let content_out = entry(&out.data, "content.xml");
    assert!(
        content_out.contains(r#"<text:p text:style-name="P1">ş<text:span text:style-name="T1"></text:span>ahar örmon</text:p>"#),
        "{content_out}"
    );
    // Not joined across <text:s/>, and the font declaration is untouched.
    assert!(
        content_out.contains("<text:p>c<text:s/>h</text:p>"),
        "{content_out}"
    );
    assert!(content_out.contains(r#"style:name="Trebuchet MS""#));

    let mut archive = ZipArchive::new(Cursor::new(&out.data)).unwrap();
    let first = archive.by_index(0).unwrap();
    assert_eq!(first.name().unwrap(), "mimetype");
    assert_eq!(first.compression(), CompressionMethod::Stored);
}

#[test]
fn encrypted_odf_is_rejected() {
    let manifest = r#"<manifest:manifest><manifest:file-entry><manifest:encryption-data/></manifest:file-entry></manifest:manifest>"#;
    let input = make_zip(
        &[
            ("mimetype", "application/vnd.oasis.opendocument.text"),
            (
                "content.xml",
                "<office:text><text:p>shahar</text:p></office:text>",
            ),
            ("META-INF/manifest.xml", manifest),
        ],
        &["mimetype"],
    );
    assert!(matches!(
        convert_bytes("odt", &input, &opts()),
        Err(DocError::Encrypted)
    ));
}

// ---------------------------------------------------------------- RTF

fn rtf(input: &str) -> (String, usize) {
    let out = convert_bytes("rtf", input.as_bytes(), &opts()).unwrap();
    (String::from_utf8(out.data).unwrap(), out.conversions)
}

#[test]
fn rtf_keeps_every_font_in_the_font_table() {
    let input = r"{\rtf1\ansi\ansicpg1252\deff0{\fonttbl{\f0 Arial;}{\f1 Trebuchet MS;}{\f2 Bahnschrift;}}\pard\f1\fs24 shahar\par}";
    let (out, count) = rtf(input);
    assert_eq!(count, 1);
    assert!(
        out.contains(r"{\f1 Trebuchet MS;}{\f2 Bahnschrift;}"),
        "{out}"
    );
    assert!(out.contains(r"\u351?ahar\par"), "{out}");
}

#[test]
fn rtf_converts_words_split_across_formatting_groups() {
    let (out, count) = rtf(r"{\rtf1\ansi\pard s{\b h}ahar}");
    assert_eq!(count, 1);
    assert_eq!(out, r"{\rtf1\ansi\pard \u351?{\b }ahar}");
}

#[test]
fn rtf_recognizes_every_encoding_of_a_smart_apostrophe() {
    for body in [
        r"o\rquote zbek",
        r"o\'92zbek",
        concat!(r"o\u82", "17", r"\'92zbek"),
        concat!(r"o\u82", "17", "?zbek"),
        "o'zbek",
    ] {
        let (out, count) = rtf(&format!(r"{{\rtf1\ansi\ansicpg1252\pard {}}}", body));
        assert_eq!(count, 1, "{body}");
        assert_eq!(out, r"{\rtf1\ansi\ansicpg1252\pard \u246?zbek}", "{body}");
    }
}

#[test]
fn rtf_leaves_field_instructions_alone() {
    let input =
        r#"{\rtf1\ansi{\field{\*\fldinst HYPERLINK "https://shahar.uz"}{\fldrslt shahar}}}"#;
    let (out, count) = rtf(input);
    assert_eq!(count, 1);
    assert!(out.contains(r#"HYPERLINK "https://shahar.uz""#), "{out}");
    assert!(out.contains(r"{\fldrslt \u351?ahar}"), "{out}");
}

#[test]
fn rtf_respects_unicode_skip_count() {
    let (out, count) = rtf(r"{\rtf1\ansi\uc0\pard sh}");
    assert_eq!(count, 1);
    assert_eq!(out, r"{\rtf1\ansi\uc0\pard \u351 }");
}

#[test]
fn rtf_does_not_join_words_across_paragraphs() {
    let (_, count) = rtf(r"{\rtf1\ansi\pard s\par h}");
    assert_eq!(count, 0);
}

#[test]
fn non_rtf_content_is_rejected() {
    assert!(matches!(
        convert_bytes("rtf", b"plain text", &opts()),
        Err(DocError::WrongType { .. })
    ));
}

// ---------------------------------------------------------------- HTML

fn html(input: &str) -> (String, usize) {
    let out = convert_bytes("html", input.as_bytes(), &opts()).unwrap();
    (String::from_utf8(out.data).unwrap(), out.conversions)
}

#[test]
fn html_converts_visible_text_only() {
    let input = concat!(
        r#"<meta charset="utf-8"><p class="shahar">o&#39;rmon <b>sh</b>ahar</p>"#,
        r#"<script>var s = "shahar";</script><style>.choy{}</style><code>choy</code>"#,
        r#"<a href="https://shahar.uz" title="shahar">choy</a><!-- shahar -->"#,
    );
    let (out, count) = html(input);
    assert_eq!(count, 3);
    assert!(
        out.contains(r#"<p class="shahar">örmon <b>ş</b>ahar</p>"#),
        "{out}"
    );
    assert!(out.contains(r#"<script>var s = "shahar";</script>"#));
    assert!(out.contains("<style>.choy{}</style><code>choy</code>"));
    assert!(out.contains(r#"<a href="https://shahar.uz" title="shahar">çoy</a><!-- shahar -->"#));
}

#[test]
fn html_without_utf8_declaration_gets_numeric_references() {
    assert_eq!(html("<p>shahar</p>").0, "<p>&#351;ahar</p>");
    assert_eq!(
        html(r#"<meta charset="windows-1251"><p>shahar</p>"#).0,
        r#"<meta charset="windows-1251"><p>&#351;ahar</p>"#
    );
}

#[test]
fn html_that_is_not_utf8_is_rejected() {
    assert!(matches!(
        convert_bytes("html", b"<p>sh\xE0har</p>", &opts()),
        Err(DocError::Encoding)
    ));
}

// ---------------------------------------------------------------- plain text

fn convert_text(ext: &str, data: &[u8]) -> (Vec<u8>, usize) {
    let out = convert_bytes(ext, data, &opts()).unwrap();
    (out.data, out.conversions)
}

#[test]
fn text_keeps_line_endings_and_adds_bom_to_ascii_txt() {
    let (out, count) = convert_text("txt", b"shahar\r\nchoy\r\n");
    assert_eq!(count, 2);
    assert_eq!(out, b"\xEF\xBB\xBF\xC5\x9Fahar\r\n\xC3\xA7oy\r\n");
}

#[test]
fn text_without_bom_requirement_stays_plain_utf8() {
    let (out, _) = convert_text("md", b"shahar");
    assert_eq!(String::from_utf8(out).unwrap(), "şahar");
}

#[test]
fn text_keeps_existing_bom_and_utf16() {
    let (out, _) = convert_text("txt", b"\xEF\xBB\xBFshahar");
    assert_eq!(out, "\u{feff}şahar".as_bytes());

    let mut utf16 = vec![0xFF, 0xFE];
    utf16.extend("shahar".encode_utf16().flat_map(u16::to_le_bytes));
    let (out, count) = convert_text("txt", &utf16);
    assert_eq!(count, 1);
    let mut expected = vec![0xFF, 0xFE];
    expected.extend("şahar".encode_utf16().flat_map(u16::to_le_bytes));
    assert_eq!(out, expected);
}

#[test]
fn text_with_nothing_to_convert_is_returned_unchanged() {
    let (out, count) = convert_text("txt", b"salom dunyo");
    assert_eq!(count, 0);
    assert_eq!(out, b"salom dunyo");
}

#[test]
fn text_in_a_legacy_encoding_is_rejected() {
    assert!(matches!(
        convert_bytes("txt", b"sh\xE0har", &opts()),
        Err(DocError::Encoding)
    ));
}

// ---------------------------------------------------------------- files on disk

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("harfiy-docs-{}-{}", name, std::process::id()));
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
fn convert_file_writes_a_new_file_and_never_overwrites() {
    let dir = TempDir::new("convert");
    let input = dir.0.join("hisobot.v2.md");
    std::fs::write(&input, "shahar").unwrap();

    let first = convert_file(&input, &opts()).unwrap();
    assert_eq!(first.conversions, 1);
    assert_eq!(
        first.output.as_deref(),
        Some(dir.0.join("hisobot.v2.harfiy.md").as_path())
    );

    let second = convert_file(&input, &opts()).unwrap();
    assert_eq!(
        second.output.as_deref(),
        Some(dir.0.join("hisobot.v2.harfiy (2).md").as_path())
    );

    assert_eq!(std::fs::read_to_string(&input).unwrap(), "shahar");
    assert_eq!(
        std::fs::read_to_string(first.output.unwrap()).unwrap(),
        "şahar"
    );
}

#[test]
fn convert_file_writes_nothing_when_there_is_nothing_to_convert() {
    let dir = TempDir::new("noop");
    let input = dir.0.join("salom.txt");
    std::fs::write(&input, "salom").unwrap();

    let report = convert_file(&input, &opts()).unwrap();
    assert!(report.output.is_none());
    assert_eq!(report.conversions, 0);
    assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 1);
}

#[test]
fn legacy_and_unknown_formats_get_a_helpful_error() {
    let dir = TempDir::new("legacy");
    let doc = dir.0.join("eski.doc");
    std::fs::write(&doc, b"x").unwrap();

    let err = convert_file(&doc, &opts()).unwrap_err();
    assert!(matches!(err, DocError::UnsupportedFormat(ref e) if e == "doc"));
    assert!(err.to_string().contains(".docx"), "{err}");

    assert!(is_supported(&dir.0.join("A.DOCX")));
    assert!(!is_supported(&doc));
    assert!(!is_supported(&dir.0.join("rasm.png")));
}
