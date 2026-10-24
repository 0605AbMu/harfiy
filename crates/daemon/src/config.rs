//! Configuration file handling for Harfiy daemon.

use std::fs;
use std::path::{Path, PathBuf};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Enable 'ng' / 'NG' conversion to 'ñ' / 'Ñ' (default: false)
    #[serde(default = "default_enable_ng")]
    pub enable_ng: bool,

    /// Smart apostrophe (hamza / tutuq belgisi) auto-conversion to ʻ (default: true)
    #[serde(default = "default_smart_apostrophe")]
    pub smart_apostrophe: bool,

    /// Automatic game mode detection for fullscreen applications (default: true)
    #[serde(default = "default_game_mode")]
    pub game_mode: bool,

    /// Global hotkey combination to toggle on/off (default: "Ctrl+Shift+U")
    #[serde(default = "default_hotkey")]
    pub hotkey: String,

    /// Global hotkey combination to quickly convert selected text (default: "Ctrl+Alt+U")
    #[serde(default = "default_convert_hotkey")]
    pub convert_hotkey: String,

    /// Executable names where auto-conversion should be temporarily suspended
    #[serde(default = "default_excluded_apps")]
    pub excluded_apps: Vec<String>,

    /// Total number of converted digraphs and apostrophes
    #[serde(default)]
    pub total_conversions: u64,
}

fn default_enable_ng() -> bool {
    false
}

fn default_smart_apostrophe() -> bool {
    true
}

fn default_game_mode() -> bool {
    true
}

fn default_hotkey() -> String {
    "Ctrl+Shift+U".to_string()
}

fn default_convert_hotkey() -> String {
    "Ctrl+Alt+U".to_string()
}

fn default_excluded_apps() -> Vec<String> {
    vec![]
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            enable_ng: default_enable_ng(),
            smart_apostrophe: default_smart_apostrophe(),
            game_mode: default_game_mode(),
            hotkey: default_hotkey(),
            convert_hotkey: default_convert_hotkey(),
            excluded_apps: default_excluded_apps(),
            total_conversions: 0,
        }
    }
}

impl AppConfig {
    /// Determine the path to config.toml:
    /// 1. Local `./config.toml` if it exists.
    /// 2. User project config directory (`%APPDATA%\harfiy\config.toml`).
    pub fn resolve_path() -> PathBuf {
        let local = Path::new("config.toml");
        if local.exists() {
            return local.to_path_buf();
        }

        if let Some(dirs) = ProjectDirs::from("uz", "Harfiy", "harfiy") {
            let config_dir = dirs.config_dir();
            let _ = fs::create_dir_all(config_dir);
            return config_dir.join("config.toml");
        }

        PathBuf::from("config.toml")
    }

    /// Load config from disk, or create and save defaults if it does not exist.
    pub fn load_or_create() -> (Self, PathBuf) {
        let path = Self::resolve_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(cfg) = toml::from_str::<AppConfig>(&content) {
                    return (cfg, path);
                }
            }
        }

        // Save default config
        let default_cfg = AppConfig::default();
        if let Ok(serialized) = toml::to_string_pretty(&default_cfg) {
            let header = "# Harfiy Konfiguratsiya Fayli\n# Rasmiy veb-sahifa: https://abdumannon-shamsiyev.uz/harfiy\n# Relizlar: https://github.com/0605AbMu/harfiy/releases\n# Blog: https://t.me/abdumannon_notes\n# Support: https://t.me/abdumannon_shamsiyev\n\n";
            let full_text = format!("{}{}", header, serialized);
            let _ = fs::write(&path, full_text);
        }

        (default_cfg, path)
    }

    /// Save current configuration back to disk.
    pub fn save(&self) -> Result<(), std::io::Error> {
        let path = Self::resolve_path();
        if let Ok(serialized) = toml::to_string_pretty(self) {
            let header = "# Harfiy Konfiguratsiya Fayli\n# Rasmiy veb-sahifa: https://abdumannon-shamsiyev.uz/harfiy\n# Relizlar: https://github.com/0605AbMu/harfiy/releases\n# Blog: https://t.me/abdumannon_notes\n# Support: https://t.me/abdumannon_shamsiyev\n\n";
            let full_text = format!("{}{}", header, serialized);
            fs::write(path, full_text)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_serialization_with_excluded_apps() {
        let mut cfg = AppConfig::default();
        cfg.excluded_apps = vec!["code.exe".to_string(), "telegram.exe".to_string()];
        cfg.total_conversions = 42;
        cfg.game_mode = true;
        cfg.smart_apostrophe = true;
        cfg.convert_hotkey = "Ctrl+Alt+U".to_string();

        let serialized = toml::to_string_pretty(&cfg).unwrap();
        assert!(serialized.contains("code.exe"));
        assert!(serialized.contains("telegram.exe"));
        assert!(serialized.contains("total_conversions = 42"));

        let deserialized: AppConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.excluded_apps, vec!["code.exe", "telegram.exe"]);
        assert_eq!(deserialized.total_conversions, 42);
        assert!(deserialized.game_mode);
        assert!(deserialized.smart_apostrophe);
        assert_eq!(deserialized.convert_hotkey, "Ctrl+Alt+U");
    }
}
