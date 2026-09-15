use std::path::PathBuf;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use converter_core::format::MediaFormat;
use crate::i18n::Language;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    pub language: Language,
    pub output_directory: Option<PathBuf>,
    pub max_concurrent_jobs: usize,
    pub default_target_format: MediaFormat,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            language: Language::English,
            output_directory: None,
            max_concurrent_jobs: 2,
            default_target_format: MediaFormat::Mp4,
        }
    }
}

pub fn get_config_path() -> Option<PathBuf> {
    ProjectDirs::from("com", "GalenYost", "file-converter")
        .map(|dirs| dirs.config_dir().join("config.json"))
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
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }
}
