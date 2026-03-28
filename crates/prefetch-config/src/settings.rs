use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::paths;

/// Top-level application configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub prefetch: PrefetchConfig,
    #[serde(default)]
    pub memory: MemoryConfig,
    #[serde(default)]
    pub watch: WatchConfig,
    #[serde(default)]
    pub prediction: PredictionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default = "default_db_path")]
    pub db_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefetchConfig {
    #[serde(default = "default_strategy")]
    pub strategy: String,
    #[serde(default = "default_first_n_layers")]
    pub first_n_layers: u32,
    #[serde(default = "default_chunk_size_mb")]
    pub chunk_size_mb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    #[serde(default = "default_max_cache_percent")]
    pub max_cache_percent: u32,
    #[serde(default = "default_min_free_ram_gb")]
    pub min_free_ram_gb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchConfig {
    #[serde(default = "default_directories")]
    pub directories: Vec<PathBuf>,
    #[serde(default = "default_poll_interval")]
    pub poll_interval_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_lookahead")]
    pub lookahead_minutes: u32,
    #[serde(default = "default_min_frequency")]
    pub min_frequency: u32,
}

// Defaults
fn default_log_level() -> String { "info".to_string() }
fn default_db_path() -> PathBuf { paths::data_dir().join("history.db") }
fn default_strategy() -> String { "inference-order".to_string() }
fn default_first_n_layers() -> u32 { 8 }
fn default_chunk_size_mb() -> u64 { 64 }
fn default_max_cache_percent() -> u32 { 50 }
fn default_min_free_ram_gb() -> u64 { 2 }
fn default_directories() -> Vec<PathBuf> {
    vec![paths::ollama_models_dir()]
}
fn default_poll_interval() -> u64 { 300 }
fn default_true() -> bool { true }
fn default_lookahead() -> u32 { 30 }
fn default_min_frequency() -> u32 { 3 }

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            prefetch: PrefetchConfig::default(),
            memory: MemoryConfig::default(),
            watch: WatchConfig::default(),
            prediction: PredictionConfig::default(),
        }
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            log_level: default_log_level(),
            db_path: default_db_path(),
        }
    }
}

impl Default for PrefetchConfig {
    fn default() -> Self {
        Self {
            strategy: default_strategy(),
            first_n_layers: default_first_n_layers(),
            chunk_size_mb: default_chunk_size_mb(),
        }
    }
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            max_cache_percent: default_max_cache_percent(),
            min_free_ram_gb: default_min_free_ram_gb(),
        }
    }
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            directories: default_directories(),
            poll_interval_secs: default_poll_interval(),
        }
    }
}

impl Default for PredictionConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            lookahead_minutes: default_lookahead(),
            min_frequency: default_min_frequency(),
        }
    }
}

impl AppConfig {
    /// Load config from the default path, or return defaults if not found.
    pub fn load() -> anyhow::Result<Self> {
        let path = paths::config_path();
        Self::load_from(&path)
    }

    /// Load config from a specific path.
    pub fn load_from(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() {
            tracing::debug!(path = %path.display(), "no config file found, using defaults");
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        tracing::info!(path = %path.display(), "loaded config");
        Ok(config)
    }

    /// Generate an example config string.
    pub fn example_toml() -> String {
        toml::to_string_pretty(&Self::default()).unwrap_or_default()
    }
}
