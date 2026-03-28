use std::path::PathBuf;

/// Get the default config file path.
pub fn config_path() -> PathBuf {
    if let Some(proj_dirs) = directories::ProjectDirs::from("", "", "prefetch") {
        proj_dirs.config_dir().join("config.toml")
    } else {
        // Fallback
        dirs_home().join(".config/prefetch/config.toml")
    }
}

/// Get the default data directory (for SQLite DB, etc).
pub fn data_dir() -> PathBuf {
    if let Some(proj_dirs) = directories::ProjectDirs::from("", "", "prefetch") {
        proj_dirs.data_dir().to_path_buf()
    } else {
        dirs_home().join(".local/share/prefetch")
    }
}

/// Get the default Ollama models directory.
pub fn ollama_models_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("OLLAMA_MODELS") {
        return PathBuf::from(dir);
    }
    dirs_home().join(".ollama/models")
}

/// Home directory.
fn dirs_home() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}
