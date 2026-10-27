//! "Faylni o'girish": pick one file, convert it while a progress window is shown, then let
//! the user choose where to save the result.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use harfiy_docs::{ConvertOptions, DocError, FileConversion};
use harfiy_platform as platform;

/// Ask for the file (on the calling thread, like the other tray dialogs), then convert and
/// save on a worker thread so the tray stays responsive.
pub fn start(opts: ConvertOptions, counter: Arc<AtomicU64>) {
    let extensions = harfiy_docs::supported_extensions();
    let Some(path) = platform::pick_file_dialog(
        "Harfiy - O'giriladigan faylni tanlang",
        "Hujjatlar va matn fayllari",
        &extensions,
    ) else {
        return;
    };
    thread::spawn(move || run(path, opts, counter));
}

fn run(path: PathBuf, opts: ConvertOptions, counter: Arc<AtomicU64>) {
    let name = file_name_of(&path);

    let converted = match convert(&path, &name, &opts) {
        Ok(converted) => converted,
        Err(DocError::Cancelled) => {
            platform::show_custom_hud("O'girish bekor qilindi", false);
            return;
        }
        Err(e) => {
            println!("Faylni o'girib bo'lmadi: {}: {}", path.display(), e);
            platform::show_info_dialog("Harfiy - Faylni o'girish", &format!("{}\n\n{}", name, e));
            return;
        }
    };

    if converted.conversions == 0 {
        platform::show_custom_hud("O'zgartiriladigan harflar topilmadi", false);
        return;
    }

    match save(&path, &converted) {
        Some(saved) => {
            println!("Fayl saqlandi: {}", saved.display());
            counter.fetch_add(converted.conversions as u64, Ordering::SeqCst);
            platform::show_custom_hud(
                &format!("Fayl saqlandi ({} ta harf)", converted.conversions),
                true,
            );
        }
        None => platform::show_custom_hud("Saqlash bekor qilindi", false),
    }
}

fn file_name_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// Convert the file, showing a progress window (with a cancel button) if it takes a while.
/// The window closes when this returns.
fn convert(path: &Path, name: &str, opts: &ConvertOptions) -> Result<FileConversion, DocError> {
    // Quick conversions finish before the window would appear, so it never flashes.
    let progress = platform::ProgressWindow::open_after(
        "Harfiy - Fayl o'girilmoqda",
        Duration::from_millis(500),
    );

    // Office and OpenDocument files report progress part by part. Other formats are done in
    // one pass; for them the bar just keeps animating.
    progress.update(None, name, "O'girilmoqda...");
    let mut on_progress = |fraction: f32| {
        progress.update(Some(fraction), name, "O'girilmoqda...");
        !progress.is_cancelled()
    };
    let result = harfiy_docs::convert_file_to_memory(path, opts, &mut on_progress);

    // A single-pass conversion cannot be stopped midway; if cancel was pressed meanwhile, the
    // finished result is simply discarded.
    if progress.is_cancelled() {
        return Err(DocError::Cancelled);
    }
    result
}

/// Make sure `path` ends with the extension of `template` (a file name), as a Save As dialog
/// does not always add it when the user types a name with a dot in it.
fn with_extension_of(path: PathBuf, template: &str) -> PathBuf {
    let Some(extension) = Path::new(template).extension() else {
        return path;
    };
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(extension))
    {
        return path;
    }
    let mut name = path.into_os_string();
    name.push(".");
    name.push(extension);
    PathBuf::from(name)
}

/// Write to a sibling temp file and move it into place, so a failure halfway never leaves a
/// half-written file (or destroys the file being replaced).
fn write_atomically(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let name = file_name_of(path);
    let temp = path.with_file_name(format!(".{}.harfiy-tmp", name));
    let result = fs::write(&temp, data).and_then(|_| fs::rename(&temp, path));
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// Show the Save As dialog until the file is written or the user cancels.
fn save(source: &Path, converted: &FileConversion) -> Option<PathBuf> {
    let initial_dir = source.parent();
    loop {
        let chosen = platform::pick_save_file_dialog(
            "Harfiy - O'girilgan faylni saqlash",
            initial_dir,
            &converted.suggested_name,
        )?;
        let path = with_extension_of(chosen.clone(), &converted.suggested_name);

        // The original must never be replaced by its own conversion.
        if harfiy_docs::is_same_file(&path, source) {
            platform::show_info_dialog(
                "Harfiy",
                "Asl faylning ustiga yozib bo'lmaydi. Boshqa nom tanlang.",
            );
            continue;
        }
        // The dialog already asked about overwriting `chosen`; a name we completed ourselves
        // has not been confirmed yet.
        if path != chosen
            && path.exists()
            && !platform::show_confirm_dialog(
                "Harfiy",
                &format!(
                    "\"{}\" fayli allaqachon mavjud. Ustiga yozilsinmi?",
                    file_name_of(&path)
                ),
            )
        {
            continue;
        }

        match write_atomically(&path, &converted.data) {
            Ok(()) => return Some(path),
            Err(e) => platform::show_info_dialog(
                "Harfiy",
                &format!("Faylni saqlab bo'lmadi: {}\nBoshqa joyni tanlang.", e),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_is_added_only_when_missing() {
        let template = "hisobot.harfiy.docx";
        assert_eq!(
            with_extension_of(PathBuf::from("C:/x/hisobot"), template),
            PathBuf::from("C:/x/hisobot.docx")
        );
        assert_eq!(
            with_extension_of(PathBuf::from("C:/x/my.report"), template),
            PathBuf::from("C:/x/my.report.docx")
        );
        assert_eq!(
            with_extension_of(PathBuf::from("C:/x/hisobot.DOCX"), template),
            PathBuf::from("C:/x/hisobot.DOCX")
        );
        assert_eq!(
            with_extension_of(PathBuf::from("C:/x/hisobot.docx"), "noext"),
            PathBuf::from("C:/x/hisobot.docx")
        );
    }

    #[test]
    fn atomic_write_replaces_the_file_and_leaves_no_temp_file() {
        let dir = std::env::temp_dir().join(format!("harfiy-daemon-save-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let target = dir.join("natija.docx");

        fs::write(&target, b"old").unwrap();
        write_atomically(&target, b"new content").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"new content");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1, "no temp file left behind");

        // A target in a folder that does not exist fails cleanly.
        assert!(write_atomically(&dir.join("yo'q").join("a.docx"), b"x").is_err());

        let _ = fs::remove_dir_all(&dir);
    }
}
