//! GitHub Releases self-update and version checking for Harfiy.

use self_update::backends::github::{ReleaseList, Update};

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub release_url: String,
    pub installer_asset_name: Option<String>,
    pub installer_download_url: Option<String>,
    #[allow(dead_code)]
    pub release_name: String,
    #[allow(dead_code)]
    pub body: Option<String>,
}

/// Parse owner and repo name from "owner/repo" string (default: "0605AbMu", "harfiy-release").
pub fn parse_repo_owner_name(github_repo: &str) -> (&str, &str) {
    let mut parts = github_repo.split('/');
    let owner = parts.next().unwrap_or("0605AbMu").trim();
    let name = parts.next().unwrap_or("harfiy-release").trim();
    let owner = if owner.is_empty() { "0605AbMu" } else { owner };
    let name = if name.is_empty() { "harfiy-release" } else { name };
    (owner, name)
}

/// Find the most suitable installer or package asset for the current OS/architecture from the release.
pub fn find_installer_asset(assets: &[self_update::update::ReleaseAsset]) -> Option<(String, String)> {
    #[cfg(target_os = "windows")]
    {
        // 1. Inno Setup installer (.exe)
        if let Some(asset) = assets.iter().find(|a| {
            let n = a.name().to_lowercase();
            n.ends_with(".exe") && (n.contains("setup") || n.contains("installer") || n.contains("windows"))
        }) {
            return Some((asset.name().to_string(), asset.download_url().to_string()));
        }
        // 2. Any .exe
        if let Some(asset) = assets.iter().find(|a| a.name().to_lowercase().ends_with(".exe")) {
            return Some((asset.name().to_string(), asset.download_url().to_string()));
        }
        // 3. Portable .zip
        if let Some(asset) = assets.iter().find(|a| a.name().to_lowercase().ends_with(".zip") && a.name().contains("windows")) {
            return Some((asset.name().to_string(), asset.download_url().to_string()));
        }
    }

    #[cfg(target_os = "macos")]
    {
        #[cfg(target_arch = "aarch64")]
        let arch_match = "arm64";
        #[cfg(not(target_arch = "aarch64"))]
        let arch_match = "x86_64";

        // 1. Native macOS installer wizard (.pkg) matching architecture
        if let Some(asset) = assets.iter().find(|a| {
            let n = a.name().to_lowercase();
            n.ends_with(".pkg") && (n.contains(arch_match) || n.contains("universal"))
        }) {
            return Some((asset.name().to_string(), asset.download_url().to_string()));
        }
        // 2. Any .pkg
        if let Some(asset) = assets.iter().find(|a| a.name().to_lowercase().ends_with(".pkg")) {
            return Some((asset.name().to_string(), asset.download_url().to_string()));
        }
        // 3. DMG matching architecture
        if let Some(asset) = assets.iter().find(|a| {
            let n = a.name().to_lowercase();
            n.ends_with(".dmg") && (n.contains(arch_match) || n.contains("universal"))
        }) {
            return Some((asset.name().to_string(), asset.download_url().to_string()));
        }
        // 4. Any .dmg
        if let Some(asset) = assets.iter().find(|a| a.name().to_lowercase().ends_with(".dmg")) {
            return Some((asset.name().to_string(), asset.download_url().to_string()));
        }
    }

    #[cfg(target_os = "linux")]
    {
        // 1. Look for .deb package
        if let Some(asset) = assets.iter().find(|a| a.name().to_lowercase().ends_with(".deb")) {
            return Some((asset.name().to_string(), asset.download_url().to_string()));
        }
        // 2. Portable .tar.gz
        if let Some(asset) = assets.iter().find(|a| a.name().to_lowercase().ends_with(".tar.gz") && a.name().contains("linux")) {
            return Some((asset.name().to_string(), asset.download_url().to_string()));
        }
    }

    None
}

/// Check for updates from GitHub Releases using self_update.
pub fn check_for_updates(github_repo: &str) -> Result<UpdateInfo, String> {
    let current_version = env!("CARGO_PKG_VERSION");
    let (owner, repo) = parse_repo_owner_name(github_repo);

    let rel_list = ReleaseList::configure()
        .repo_owner(owner)
        .repo_name(repo)
        .build()
        .map_err(|e| format!("Konfiguratsiya xatoligi: {}", e))?
        .fetch()
        .map_err(|e| format!("GitHub yangilanishlarini tekshirishda xatolik: {}", e))?
        .with_current_version(current_version);

    let has_update = rel_list.is_update_available().unwrap_or(false);

    if let Some(latest) = rel_list.latest() {
        let latest_ver = latest.version().trim().trim_start_matches('v').to_string();

        // 1. Get exact release URL directly from GitHub Release object without hardcoding 'v'
        let release_url = latest
            .release_notes_url()
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                let tag = latest.version().trim();
                format!("https://github.com/{}/{}/releases/tag/{}", owner, repo, tag)
            });

        // 2. Find installer asset for current OS/architecture
        let (installer_asset_name, installer_download_url) = match find_installer_asset(latest.assets()) {
            Some((name, url)) => (Some(name), Some(url)),
            None => (None, None),
        };

        Ok(UpdateInfo {
            current_version: current_version.to_string(),
            latest_version: latest_ver,
            has_update,
            release_url,
            installer_asset_name,
            installer_download_url,
            release_name: latest.name().to_string(),
            body: latest.body().map(|s| s.to_string()),
        })
    } else {
        Ok(UpdateInfo {
            current_version: current_version.to_string(),
            latest_version: current_version.to_string(),
            has_update: false,
            release_url: format!("https://github.com/{}/{}/releases", owner, repo),
            installer_asset_name: None,
            installer_download_url: None,
            release_name: format!("v{}", current_version),
            body: None,
        })
    }
}

/// Download the installer package with progress feedback to a temporary file.
pub fn download_installer<F>(
    download_url: &str,
    file_name: &str,
    progress_callback: F,
) -> Result<std::path::PathBuf, String>
where
    F: Fn(u64, Option<u64>) + Send + Sync + 'static,
{
    let temp_dir = std::env::temp_dir();
    let dest_path = temp_dir.join(file_name);

    if dest_path.exists() {
        let _ = std::fs::remove_file(&dest_path);
    }

    let mut dest_file = std::fs::File::create(&dest_path)
        .map_err(|e| format!("Vaqtinchalik fayl yaratishda xatolik ({}): {}", dest_path.display(), e))?;

    self_update::Download::from_url(download_url)
        .show_download_progress(false)
        .progress_callback(progress_callback)
        .download_to(&mut dest_file)
        .map_err(|e| format!("Yuklab olishda xatolik yuz berdi: {}", e))?;

    Ok(dest_path)
}

/// Execute the downloaded installer wizard and cleanly terminate Harfiy.
pub fn launch_installer_and_exit(installer_path: &std::path::Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new(installer_path)
            .spawn()
            .map_err(|e| format!("O'rnatuvchi dasturni ishga tushirishda xatolik: {}", e))?;
        std::process::exit(0);
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("/usr/bin/open")
            .arg(installer_path)
            .spawn()
            .map_err(|e| format!("macOS o'rnatuvchisini ochishda xatolik: {}", e))?;
        std::process::exit(0);
    }

    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open")
            .arg(installer_path)
            .spawn();
        std::process::exit(0);
    }

    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        let _ = open::that(installer_path);
        Ok(())
    }
}

/// Perform in-place binary update via GitHub Releases self-update (fallback).
#[allow(dead_code)]
pub fn perform_self_update(github_repo: &str, show_progress: bool) -> Result<String, String> {
    let current_version = env!("CARGO_PKG_VERSION");
    let (owner, repo) = parse_repo_owner_name(github_repo);

    let status = Update::configure()
        .repo_owner(owner)
        .repo_name(repo)
        .bin_name("harfiy")
        .show_download_progress(show_progress)
        .current_version(current_version)
        .no_confirm(true)
        .build()
        .map_err(|e| format!("Yangilanish konfiguratsiyasida xatolik: {}", e))?
        .update()
        .map_err(|e| format!("Ilovani yangilashda xatolik yuz berdi: {}", e))?;

    if status.is_updated() {
        Ok(status.version().to_string())
    } else {
        Ok(current_version.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_repo() {
        assert_eq!(parse_repo_owner_name("0605AbMu/harfiy-release"), ("0605AbMu", "harfiy-release"));
        assert_eq!(parse_repo_owner_name("myuser/myrepo"), ("myuser", "myrepo"));
        assert_eq!(parse_repo_owner_name(""), ("0605AbMu", "harfiy-release"));
    }

    #[test]
    fn test_find_installer_asset() {
        use self_update::update::ReleaseAsset;
        let assets = vec![
            ReleaseAsset::new("harfiy-0.7.0-aarch64-apple-darwin.tar.gz", "https://example.com/tar"),
            ReleaseAsset::new("harfiy-v0.7.0-macos-arm64.pkg", "https://example.com/pkg_arm64"),
            ReleaseAsset::new("harfiy-v0.7.0-macos-x86_64.pkg", "https://example.com/pkg_x86_64"),
            ReleaseAsset::new("harfiy-v0.7.0-windows-x86_64-setup.exe", "https://example.com/setup_exe"),
            ReleaseAsset::new("harfiy_0.7.0_amd64.deb", "https://example.com/deb"),
        ];

        let found = find_installer_asset(&assets);
        assert!(found.is_some());
        let (name, url) = found.unwrap();
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            assert_eq!(name, "harfiy-v0.7.0-macos-arm64.pkg");
            assert_eq!(url, "https://example.com/pkg_arm64");
        }
        #[cfg(target_os = "windows")]
        {
            assert_eq!(name, "harfiy-v0.7.0-windows-x86_64-setup.exe");
            assert_eq!(url, "https://example.com/setup_exe");
        }
    }
}
