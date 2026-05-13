use std::path::PathBuf;

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Theme {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSettings {
    pub offline_mode: bool,
    pub cache_dir: PathBuf,
    pub max_cache_size_mb: u64,
    pub theme: Theme,
    pub gtk_compact_mode: bool,
    pub lan_discovery_enabled: bool,
    pub bluetooth_experimental_enabled: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("max cache size must be greater than zero")]
    EmptyCacheLimit,
    #[error("cache directory must not be empty")]
    EmptyCacheDir,
    #[error("project directories are unavailable on this platform")]
    ProjectDirsUnavailable,
}

impl AppSettings {
    pub fn default_for_project() -> Result<Self, ConfigError> {
        let dirs = ProjectDirs::from("dev", "meowify", "Meowify")
            .ok_or(ConfigError::ProjectDirsUnavailable)?;

        Ok(Self {
            offline_mode: false,
            cache_dir: dirs.cache_dir().to_path_buf(),
            max_cache_size_mb: 512,
            theme: Theme::System,
            gtk_compact_mode: false,
            lan_discovery_enabled: false,
            bluetooth_experimental_enabled: false,
        })
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.max_cache_size_mb == 0 {
            return Err(ConfigError::EmptyCacheLimit);
        }

        if self.cache_dir.as_os_str().is_empty() {
            return Err(ConfigError::EmptyCacheDir);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_settings() -> AppSettings {
        AppSettings {
            offline_mode: false,
            cache_dir: PathBuf::from("/tmp/meowify-cache"),
            max_cache_size_mb: 1,
            theme: Theme::System,
            gtk_compact_mode: false,
            lan_discovery_enabled: true,
            bluetooth_experimental_enabled: false,
        }
    }

    #[test]
    fn validates_default_settings_shape() {
        valid_settings().validate().unwrap();
    }

    #[test]
    fn rejects_zero_cache_limit() {
        let mut settings = valid_settings();
        settings.max_cache_size_mb = 0;

        assert_eq!(settings.validate(), Err(ConfigError::EmptyCacheLimit));
    }
}
