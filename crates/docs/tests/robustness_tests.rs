//! Files that are wrong, damaged or hostile must end in an error (or a correct result),
//! never in a panic, a hang or exhausted memory.

use std::io::{Cursor, Write};
use std::time::{Duration, Instant};

use harfiy_docs::{convert_bytes, convert_file, ConvertOptions, DocError, MAX_TEXT_FILE_BYTES};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

fn opts() -> ConvertOptions {
    ConvertOptions::default()
}

fn make_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, content) in entries {
        writer
            .start_file(
                *name,
                SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
            )
            .unwrap();
        writer.write_all(content).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn docx_with(document_xml: &[u8]) -> Vec<u8> {
    make_zip(&[
        ("[Content_Types].xml", b"<Types/>"),
        ("word/document.xml", document_xml),
    ])
}

/// Run `f`, failing the test if it takes suspiciously long (a hang or quadratic blow-up).
fn within(seconds: u64, f: impl FnOnce()) {
    let start = Instant::now();
    f();
    assert!(
        start.elapsed() < Duration::from_secs(seconds),
        "took {:?}",
        start.elapsed()
    );
}

// ------------------------------------------------------------ magic bytes

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";

#[test]
fn content_must_match_the_extension() {
    let docx = docx_with(b"<w:p><w:r><w:t>shahar</w:t></w:r></w:p>");
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("txt", PNG.to_vec()),
        ("txt", docx.clone()),
        ("txt", b"%PDF-1.7 shahar".to_vec()),
        ("txt", b"{\\rtf1\\ansi shahar}".to_vec()),
        ("md", b"binary\x00\x01\x02 shahar".to_vec()),
        ("html", PNG.to_vec()),
        ("html", docx.clone()),
        ("docx", b"%PDF-1.7".to_vec()),
        ("docx", b"just text, not a zip".to_vec()),
        ("docx", PNG.to_vec()),
        ("xlsx", b"{\\rtf1 shahar}".to_vec()),
        ("odt", b"MZ\x90\x00\x03\x00\x00\x00".to_vec()),
        ("rtf", docx),
        ("rtf", PNG.to_vec()),
        ("rtf", b"plain text".to_vec()),
    ];
    for (ext, data) in cases {
        let result = convert_bytes(ext, &data, &opts());
        assert!(
            matches!(result, Err(DocError::WrongType { .. })),
            ".{ext} with {:?}: {:?}",
            &data[..data.len().min(12)],
            result.map(|c| c.conversions)
        );
    }
}

#[test]
fn wrong_type_message_names_what_was_found() {
    let err = convert_bytes("txt", PNG, &opts()).unwrap_err();
    let message = err.to_string();
    assert!(
        message.contains(".txt") && message.contains("rasm"),
        "{message}"
    );
}

#[test]
fn a_zip_that_is_not_the_claimed_document_is_rejected() {
    let zip = make_zip(&[("readme.txt", b"shahar")]);
    assert!(matches!(
        convert_bytes("docx", &zip, &opts()),
        Err(DocError::Corrupt(_))
    ));
    assert!(matches!(
        convert_bytes("odt", &zip, &opts()),
        Err(DocError::Corrupt(_))
    ));
}

#[test]
fn empty_files_are_handled_per_format() {
    let txt = convert_bytes("txt", b"", &opts()).unwrap();
    assert_eq!(txt.conversions, 0);
    assert_eq!(convert_bytes("html", b"", &opts()).unwrap().conversions, 0);
    for ext in ["docx", "odt", "rtf"] {
        assert!(
            matches!(convert_bytes(ext, b"", &opts()), Err(DocError::Corrupt(_))),
            ".{ext}"
        );
    }
}

#[test]
fn utf16_text_with_bom_is_not_mistaken_for_binary() {
    let mut data = vec![0xFF, 0xFE];
    data.extend("shahar".encode_utf16().flat_map(u16::to_le_bytes));
    assert_eq!(convert_bytes("txt", &data, &opts()).unwrap().conversions, 1);
}

// ------------------------------------------------------------ limits

#[test]
fn oversized_files_are_rejected_before_parsing() {
    let big = vec![b'a'; MAX_TEXT_FILE_BYTES as usize + 1];
    assert!(matches!(
        convert_bytes("txt", &big, &opts()),
        Err(DocError::TooLarge(_))
    ));
}

#[test]
fn a_decompression_bomb_is_stopped() {
    // 70 MiB of text that deflates to a few dozen kilobytes.
    let mut xml = b"<w:p><w:r><w:t>".to_vec();
    xml.extend(std::iter::repeat_n(b'a', 70 * 1024 * 1024));
    xml.extend_from_slice(b"</w:t></w:r></w:p>");
    let bomb = docx_with(&xml);
    assert!(
        bomb.len() < 1024 * 1024,
        "fixture should be tiny: {}",
        bomb.len()
    );

    within(60, || {
        assert!(matches!(
            convert_bytes("docx", &bomb, &opts()),
            Err(DocError::TooLarge(_))
        ));
    });
}

#[test]
fn an_archive_with_too_many_entries_is_rejected() {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    writer.start_file("[Content_Types].xml", options).unwrap();
    writer.write_all(b"<Types/>").unwrap();
    for i in 0..50_000 {
        writer
            .start_file(format!("media/{i}.bin"), options)
            .unwrap();
    }
    let zip = writer.finish().unwrap().into_inner();
    assert!(matches!(
        convert_bytes("docx", &zip, &opts()),
        Err(DocError::TooLarge(_))
    ));
}

#[test]
fn an_unreasonable_number_of_edits_is_an_error_not_an_allocation_spree() {
    let text = "sh".repeat(4_000_001);
    within(120, || {
        assert!(matches!(
            convert_bytes("md", text.as_bytes(), &opts()),
            Err(DocError::TooLarge(_))
        ));
    });
}

#[test]
fn one_enormous_line_is_processed_in_bounded_chunks() {
    let line = "shahar ".repeat(3_000_000); // 21 MB without a single line break
    within(120, || {
        let out = convert_bytes("md", line.as_bytes(), &opts()).unwrap();
        assert_eq!(out.conversions, 3_000_000);
    });
}

#[test]
fn extremely_nested_rtf_groups_are_rejected() {
    let mut rtf = b"{\\rtf1 ".to_vec();
    rtf.extend(std::iter::repeat_n(b'{', 100_000));
    rtf.extend_from_slice(b"shahar");
    assert!(matches!(
        convert_bytes("rtf", &rtf, &opts()),
        Err(DocError::Corrupt(_))
    ));
}

#[test]
fn directories_and_missing_files_are_errors() {
    let dir = std::env::temp_dir().join(format!("harfiy-docs-dir-{}.txt", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    assert!(convert_file(&dir, &opts()).is_err());
    std::fs::remove_dir_all(&dir).unwrap();

    let missing = std::env::temp_dir().join("harfiy-docs-missing-file.txt");
    assert!(matches!(
        convert_file(&missing, &opts()),
        Err(DocError::Io(_))
    ));
}

// ------------------------------------------------------------ pathological input

#[test]
fn stray_markup_characters_do_not_cause_quadratic_work() {
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("html", "< ".repeat(300_000).into_bytes()),
        ("html", "<<<<".repeat(250_000).into_bytes()),
        ("html", "&".repeat(1_000_000).into_bytes()),
        ("html", "&#".repeat(500_000).into_bytes()),
        ("html", "<a \"".repeat(100_000).into_bytes()),
        (
            "html",
            format!("<p>{}", "<!--".repeat(100_000)).into_bytes(),
        ),
        (
            "html",
            format!("<script>{}", "shahar".repeat(100_000)).into_bytes(),
        ),
        (
            "html",
            format!("<a title=\"{}\">shahar", "x".repeat(2_000_000)).into_bytes(),
        ),
        ("docx", docx_with("<w:t>".repeat(200_000).as_bytes())),
        ("docx", docx_with("<".repeat(1_000_000).as_bytes())),
        ("docx", docx_with("<w:t>&".repeat(200_000).as_bytes())),
        (
            "docx",
            docx_with(format!("<w:t>{}", "x".repeat(5_000_000)).as_bytes()),
        ),
        (
            "odt",
            make_zip(&[
                ("mimetype", b"x"),
                ("content.xml", "<text:p>".repeat(200_000).as_bytes()),
            ]),
        ),
        (
            "rtf",
            format!("{{\\rtf1 {}", "\\'".repeat(500_000)).into_bytes(),
        ),
        (
            "rtf",
            format!("{{\\rtf1 {}", "\\u".repeat(500_000)).into_bytes(),
        ),
        (
            "rtf",
            format!("{{\\rtf1 {}", "\\".repeat(500_000)).into_bytes(),
        ),
        (
            "rtf",
            format!("{{\\rtf1 {}", "\\u-99999999999 ".repeat(100_000)).into_bytes(),
        ),
        ("rtf", b"{\\rtf1 \\bin99999999999999999999 shahar}".to_vec()),
        (
            "rtf",
            format!("{{\\rtf1 {}}}", "}".repeat(500_000)).into_bytes(),
        ),
    ];
    for (index, (ext, data)) in cases.iter().enumerate() {
        let start = Instant::now();
        let result = convert_bytes(ext, data, &opts());
        let elapsed = start.elapsed();
        assert!(
            !matches!(result, Err(DocError::Internal)),
            "case {index} (.{ext}, {} bytes) caused an internal error",
            data.len()
        );
        assert!(
            elapsed < Duration::from_secs(10),
            "case {index} (.{ext}, {} bytes) took {elapsed:?}",
            data.len()
        );
    }
}

// ------------------------------------------------------------ mutation fuzzing

/// xorshift64*: small, deterministic and good enough to mutate inputs.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() % n as u64) as usize
        }
    }
}

fn mutate(rng: &mut Rng, data: &[u8]) -> Vec<u8> {
    let mut out = data.to_vec();
    for _ in 0..=rng.below(4) {
        if out.is_empty() {
            out.push(rng.next() as u8);
            continue;
        }
        let at = rng.below(out.len());
        match rng.below(7) {
            0 => out[at] = rng.next() as u8,
            1 => out[at] ^= 1 << rng.below(8),
            2 => out.truncate(at),
            3 => {
                let n = rng.below(16) + 1;
                let bytes: Vec<u8> = (0..n).map(|_| rng.next() as u8).collect();
                out.splice(at..at, bytes);
            }
            4 => {
                let end = (at + rng.below(32) + 1).min(out.len());
                out.drain(at..end);
            }
            5 => {
                let end = (at + rng.below(64) + 1).min(out.len());
                let chunk = out[at..end].to_vec();
                out.splice(at..at, chunk);
            }
            _ => {
                // Plant markup-significant characters.
                const NASTY: &[&[u8]] = &[
                    b"<",
                    b">",
                    b"&",
                    b";",
                    b"\"",
                    b"'",
                    b"{",
                    b"}",
                    b"\\",
                    b"\\'",
                    b"\\u",
                    b"\\bin",
                    b"<!--",
                    b"-->",
                    b"<![CDATA[",
                    b"]]>",
                    b"</",
                    b"/>",
                    b"&#x",
                    b"\xff\xfe",
                    b"\0",
                ];
                let piece = NASTY[rng.below(NASTY.len())];
                out.splice(at..at, piece.iter().copied());
            }
        }
    }
    out
}

const DOCUMENT_XML: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="x"><w:body>"#,
    r#"<w:p><w:pPr><w:pStyle w:val="H1"/></w:pPr><w:r><w:rPr><w:b/></w:rPr><w:t>s</w:t></w:r>"#,
    r#"<w:r><w:t xml:space="preserve">hahar o&apos;rmon &#8217;g&amp;h</w:t></w:r>"#,
    r#"<w:r><w:instrText> HYPERLINK "https://shahar.uz" </w:instrText></w:r>"#,
    r#"<!-- choy --><![CDATA[sh]]><w:tab/><w:t>choy</w:t></w:p></w:body></w:document>"#,
);

const HTML: &str = concat!(
    r#"<!DOCTYPE html><html><head><meta charset="utf-8"><title>Choy</title>"#,
    r#"<style>.sh{color:red}</style></head><body><p class="a>b">o&#39;rmon <b>sh</b>ahar &amp; "#,
    r#"&rsquo;g</p><script>var s="shahar"</script><code>choy</code><!-- sh --></body></html>"#,
);

const RTF: &str = concat!(
    r"{\rtf1\ansi\ansicpg1252\uc1\deff0{\fonttbl{\f0 Arial;}{\f1 Trebuchet MS;}}",
    r"{\colortbl;\red255\green0\blue0;}\pard\f1\fs24 s{\b h}ahar o\rquote zbek g\'92isht ",
    r"\u82",
    "17",
    r"\'92 \u-3841?x{\*\fldinst HYPERLINK shahar}{\fldrslt choy}\par}",
);

#[test]
fn mutated_inputs_never_cause_an_internal_error() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);

    let docx = docx_with(DOCUMENT_XML.as_bytes());
    let odt = make_zip(&[
        ("mimetype", b"application/vnd.oasis.opendocument.text"),
        (
            "content.xml",
            b"<office:text><text:p>s<text:span>h</text:span>ahar o'rmon &amp; &#39;</text:p></office:text>",
        ),
    ]);
    let mut utf16 = vec![0xFF, 0xFE];
    utf16.extend(
        "shahar o'rmon\r\nchoy"
            .encode_utf16()
            .flat_map(u16::to_le_bytes),
    );
    let bases: Vec<(&str, Vec<u8>)> = vec![
        ("docx", docx),
        ("odt", odt),
        ("html", HTML.as_bytes().to_vec()),
        ("rtf", RTF.as_bytes().to_vec()),
        ("txt", "shahar o'rmon\r\nchoy g'isht\n".as_bytes().to_vec()),
        ("txt", utf16),
    ];

    for (ext, base) in &bases {
        for round in 0..1500 {
            let input = mutate(&mut rng, base);
            let result = convert_bytes(ext, &input, &opts());
            assert!(
                !matches!(result, Err(DocError::Internal)),
                ".{ext} round {round}: internal error for input {:?}",
                String::from_utf8_lossy(&input)
            );
        }
    }
}

#[test]
fn mutated_xml_inside_a_valid_archive_never_panics_and_keeps_the_archive_valid() {
    let mut rng = Rng(0xD1B5_4A32_D192_ED03);

    for round in 0..2000 {
        let xml = mutate(&mut rng, DOCUMENT_XML.as_bytes());
        let docx = docx_with(&xml);
        match convert_bytes("docx", &docx, &opts()) {
            Ok(out) => {
                // Whatever came out must still be a readable archive.
                zip::ZipArchive::new(Cursor::new(out.data))
                    .unwrap_or_else(|e| panic!("round {round}: output is not a valid zip: {e}"));
            }
            Err(DocError::Internal) => panic!("round {round}: internal error"),
            Err(_) => {}
        }
    }
}

#[test]
fn text_and_html_output_stays_valid_utf8() {
    let mut rng = Rng(0xA076_1D64_78BD_642F);

    for round in 0..2000 {
        for (ext, base) in [("html", HTML), ("md", "shahar o'rmon &amp; g'isht\nchoy\n")] {
            let input = mutate(&mut rng, base.as_bytes());
            if let Ok(out) = convert_bytes(ext, &input, &opts()) {
                if std::str::from_utf8(&input).is_ok() {
                    assert!(
                        std::str::from_utf8(&out.data).is_ok(),
                        "round {round}: .{ext} output is not UTF-8"
                    );
                }
            }
        }
    }
}
