//! GitHub Releases self-update and version checking for Harfiy.

use self_update::backends::github::{ReleaseList, Update};

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub release_url: String,
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
        let release_url = format!("https://github.com/{}/{}/releases/tag/v{}", owner, repo, latest_ver);

        Ok(UpdateInfo {
            current_version: current_version.to_string(),
            latest_version: latest_ver,
            has_update,
            release_url,
            release_name: latest.name().to_string(),
            body: latest.body().map(|s| s.to_string()),
        })
    } else {
        Ok(UpdateInfo {
            current_version: current_version.to_string(),
            latest_version: current_version.to_string(),
            has_update: false,
            release_url: format!("https://github.com/{}/{}/releases", owner, repo),
            release_name: format!("v{}", current_version),
            body: None,
        })
    }
}

/// Perform in-place binary update via GitHub Releases self-update.
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
}
