use std::path::PathBuf;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use converter_core::format::MediaFormat;
use crate::i18n::Language;

/// The window state the app should use when it starts up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum StartupWindowMode {
    /// Default windowed state.
    #[default]
    Normal,
    /// Start maximized.
    Maximized,
    /// Start minimized.
    Minimized,
}

impl StartupWindowMode {
    pub const ALL: &'static [StartupWindowMode] =
        &[StartupWindowMode::Normal, StartupWindowMode::Maximized, StartupWindowMode::Minimized];
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    pub language: Language,
    pub output_directory: Option<PathBuf>,
    pub max_concurrent_jobs: usize,
    pub default_target_format: MediaFormat,
    #[serde(default)]
    pub ui_scale: Option<f64>,
    #[serde(default)]
    pub window_mode: StartupWindowMode,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            language: Language::English,
            output_directory: None,
            max_concurrent_jobs: 2,
            default_target_format: MediaFormat::Mp4,
            ui_scale: None,
            window_mode: StartupWindowMode::Normal,
        }
    }
}

pub fn get_config_dir() -> Option<PathBuf> {
    ProjectDirs::from("com", "GalenYost", "file-converter")
        .map(|dirs| dirs.config_dir().to_path_buf())
}

pub fn get_config_path() -> Option<PathBuf> {
    get_config_dir().map(|dir| dir.join("config.json"))
}

#[allow(dead_code)]
pub fn get_log_path() -> Option<PathBuf> {
    get_config_dir().map(|dir| dir.join("file-converter.log"))
}

pub fn load_config() -> AppConfig {
    if let Some(path) = get_config_path() {
        if path.exists() {
            if let Ok(data) = std::fs::read_to_string(&path) {
                if let Ok(config) = serde_json::from_str::<AppConfig>(&data) {
                    return config;
                }
            }
        }
    }
    AppConfig::default()
}

pub fn save_config(config: &AppConfig) -> Result<(), String> {
    if let Some(path) = get_config_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let json = serde_json::to_string_pretty(config)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;
        std::fs::write(&path, json)
            .map_err(|e| format!("Failed to write config file: {}", e))?;
        Ok(())
    } else {
        Err("Could not determine config directory".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_serde() {
        let config = AppConfig {
            language: Language::Ukrainian,
            output_directory: Some(PathBuf::from("/test/path")),
            max_concurrent_jobs: 4,
            default_target_format: MediaFormat::Flac,
            ui_scale: Some(1.25),
            window_mode: StartupWindowMode::Maximized,
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_config_serde_backward_compat() {
        let json = r#"{"language":"English","output_directory":null,"max_concurrent_jobs":2,"default_target_format":"Mp4"}"#;
        let config: AppConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.ui_scale, None);
        assert_eq!(config.window_mode, StartupWindowMode::Normal);
    }
}
