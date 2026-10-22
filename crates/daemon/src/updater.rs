//! Custom Server & GitHub Releases updater and version check for Harfiy.

use serde::Deserialize;

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct CustomUpdateFeed {
    pub version: String,
    pub name: Option<String>,
    pub url: Option<String>,
    pub windows: Option<String>,
    pub linux: Option<String>,
    pub macos: Option<String>,
    pub windows_installer: Option<String>,
    pub linux_installer: Option<String>,
    pub macos_installer: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub html_url: String,
    pub name: Option<String>,
    pub body: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub release_url: String,
    #[allow(dead_code)]
    pub release_name: String,
}

/// Parse a SemVer string into (major, minor, patch).
pub fn parse_semver(v: &str) -> Option<(u64, u64, u64)> {
    let s = v.trim().trim_start_matches('v').trim_start_matches('V');
    let mut parts = s.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch_str = parts.next().unwrap_or("0");
    let patch = patch_str
        .split(|c: char| !c.is_numeric())
        .next()?
        .parse()
        .ok()?;
    Some((major, minor, patch))
}

/// Compare two versions. Returns true if `latest` is newer than `current`.
pub fn is_newer_version(latest: &str, current: &str) -> bool {
    if let (Some(l), Some(c)) = (parse_semver(latest), parse_semver(current)) {
        l > c
    } else {
        latest.trim_start_matches('v') != current.trim_start_matches('v')
    }
}

/// Check for updates from custom server URL (e.g. https://abdumannon-shamsiyev.uz/harfiy/updates)
/// with graceful fallback to GitHub Releases.
pub fn check_for_updates(update_url: &str, github_repo: &str) -> Result<UpdateInfo, String> {
    let current_version = env!("CARGO_PKG_VERSION");
    let base_url = update_url.trim().trim_end_matches('/');

    // 1. Try custom server if it is an HTTP/HTTPS URL
    if base_url.starts_with("http://") || base_url.starts_with("https://") {
        if let Some(info) = check_custom_server(base_url, current_version) {
            return Ok(info);
        }
    }

    // 2. Fallback to GitHub Releases API if custom server is not reachable
    check_github_releases(github_repo, current_version)
}

fn check_custom_server(base_url: &str, current_version: &str) -> Option<UpdateInfo> {
    // A. Try version.json or latest.json
    for filename in &["version.json", "latest.json"] {
        let json_url = format!("{}/{}", base_url, filename);
        if let Ok(res) = ureq::get(&json_url)
            .set("User-Agent", &format!("harfiy/{}", current_version))
            .set("Accept", "application/json")
            .timeout(std::time::Duration::from_secs(5))
            .call()
        {
            if let Ok(feed) = res.into_json::<CustomUpdateFeed>() {
                let latest_ver = feed.version.trim().trim_start_matches('v').to_string();
                let has_update = is_newer_version(&latest_ver, current_version);

                let platform_rel = if cfg!(windows) {
                    feed.windows.or(feed.windows_installer)
                } else if cfg!(target_os = "macos") {
                    feed.macos.or(feed.macos_installer)
                } else {
                    feed.linux.or(feed.linux_installer)
                };

                let release_url = if let Some(path) = platform_rel {
                    if path.starts_with("http://") || path.starts_with("https://") {
                        path
                    } else {
                        format!("{}/{}", base_url, path.trim_start_matches('/'))
                    }
                } else if let Some(url) = feed.url {
                    url
                } else {
                    format!("{}/", base_url)
                };

                return Some(UpdateInfo {
                    current_version: current_version.to_string(),
                    latest_version: latest_ver,
                    has_update,
                    release_url,
                    release_name: feed.name.unwrap_or_else(|| format!("Harfiy v{}", feed.version)),
                });
            }
        }
    }

    // B. Try version.txt as simple text file
    let txt_url = format!("{}/version.txt", base_url);
    if let Ok(res) = ureq::get(&txt_url)
        .set("User-Agent", &format!("harfiy/{}", current_version))
        .timeout(std::time::Duration::from_secs(5))
        .call()
    {
        if let Ok(text) = res.into_string() {
            let line = text.lines().next().unwrap_or("").trim();
            if let Some((_, _, _)) = parse_semver(line) {
                let latest_ver = line.trim_start_matches('v').to_string();
                let has_update = is_newer_version(&latest_ver, current_version);

                // Default installer name per platform
                let installer = if cfg!(windows) {
                    format!("harfiy-{}-windows-x86_64-setup.exe", latest_ver)
                } else if cfg!(target_os = "macos") {
                    format!("harfiy-{}-macos-x86_64.dmg", latest_ver)
                } else {
                    format!("harfiy_{}_amd64.deb", latest_ver)
                };

                let release_url = format!("{}/{}/{}", base_url, latest_ver, installer);

                return Some(UpdateInfo {
                    current_version: current_version.to_string(),
                    latest_version: latest_ver,
                    has_update,
                    release_url,
                    release_name: format!("Harfiy v{}", line),
                });
            }
        }
    }

    None
}

fn check_github_releases(repo: &str, current_version: &str) -> Result<UpdateInfo, String> {
    let url = format!("https://api.github.com/repos/{}/releases/latest", repo);

    let res = ureq::get(&url)
        .set("User-Agent", &format!("harfiy/{}", current_version))
        .set("Accept", "application/vnd.github.v3+json")
        .timeout(std::time::Duration::from_secs(5))
        .call();

    match res {
        Ok(response) => {
            let release: GitHubRelease = response
                .into_json()
                .map_err(|e| format!("Javobni tahlil qilishda xatolik: {}", e))?;

            let latest_ver = release.tag_name.trim_start_matches('v').to_string();
            let has_update = is_newer_version(&latest_ver, current_version);

            Ok(UpdateInfo {
                current_version: current_version.to_string(),
                latest_version: latest_ver,
                has_update,
                release_url: release.html_url,
                release_name: release.name.unwrap_or(release.tag_name),
            })
        }
        Err(ureq::Error::Status(404, _)) => {
            Ok(UpdateInfo {
                current_version: current_version.to_string(),
                latest_version: current_version.to_string(),
                has_update: false,
                release_url: format!("https://github.com/{}/releases", repo),
                release_name: format!("v{}", current_version),
            })
        }
        Err(e) => Err(format!("Yangilanish serveriga ulanishda xatolik: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semver_parsing() {
        assert_eq!(parse_semver("0.1.0"), Some((0, 1, 0)));
        assert_eq!(parse_semver("v0.2.5"), Some((0, 2, 5)));
        assert_eq!(parse_semver("V1.0.0-rc1"), Some((1, 0, 0)));
        assert_eq!(parse_semver("2.1"), Some((2, 1, 0)));
    }

    #[test]
    fn test_version_comparison() {
        assert!(is_newer_version("v0.2.0", "0.1.0"));
        assert!(is_newer_version("0.1.1", "0.1.0"));
        assert!(is_newer_version("1.0.0", "0.9.9"));
        assert!(!is_newer_version("0.1.0", "0.1.0"));
        assert!(!is_newer_version("0.1.0", "0.2.0"));
        assert!(!is_newer_version("v0.1.0", "0.1.1"));
    }
}
