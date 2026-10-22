//! GitHub Releases updater and version check for Harfiy.

use serde::Deserialize;

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub html_url: String,
    pub name: Option<String>,
    pub body: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub release_url: String,
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

/// Query the latest release from the GitHub Releases API.
pub fn check_for_updates(repo: &str) -> Result<UpdateInfo, String> {
    let current_version = env!("CARGO_PKG_VERSION");
    let url = format!("https://api.github.com/repos/{}/releases/latest", repo);

    let res = ureq::get(&url)
        .set("User-Agent", &format!("harfiy/{}", current_version))
        .set("Accept", "application/vnd.github.v3+json")
        .timeout(std::time::Duration::from_secs(6))
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
            // 404 indicates either no releases published yet or repository is private.
            // Treat the currently running version as the latest.
            Ok(UpdateInfo {
                current_version: current_version.to_string(),
                latest_version: current_version.to_string(),
                has_update: false,
                release_url: format!("https://github.com/{}/releases", repo),
                release_name: format!("v{}", current_version),
            })
        }
        Err(e) => Err(format!("GitHub API xatoligi: {}", e)),
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
