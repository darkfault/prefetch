//! Manifest-based provider for user-defined file layouts.
//!
//! Users can describe any file's structure in a `.prefetch.toml` file:
//!
//! ```toml
//! format = "my-database"
//!
//! [[segments]]
//! name = "index"
//! offset = 0
//! length = 4096
//! priority = 0
//!
//! [[segments]]
//! name = "hot-table"
//! offset = 8192
//! length = 52428800
//! priority = 1
//! ```
//!
//! Place the manifest alongside the target file as `<filename>.prefetch.toml`,
//! or pass it explicitly with `--manifest <path>`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::{FileLayout, FileProvider, Segment};

/// Provider that reads prefetch layout from a TOML manifest.
pub struct ManifestProvider;

impl FileProvider for ManifestProvider {
    fn name(&self) -> &str {
        "manifest"
    }

    fn can_handle(&self, path: &Path) -> bool {
        path.extension().map_or(false, |e| e == "toml")
            && path.to_string_lossy().contains(".prefetch")
    }

    fn analyze(&self, path: &Path) -> anyhow::Result<FileLayout> {
        parse_manifest(path)
    }
}

/// Derive the manifest path for a given target file.
/// e.g., `/data/mydb.dat` → `/data/mydb.dat.prefetch.toml`
pub fn manifest_path_for(target: &Path) -> PathBuf {
    let mut manifest = target.as_os_str().to_owned();
    manifest.push(".prefetch.toml");
    PathBuf::from(manifest)
}

/// Parse a manifest file and resolve relative to the manifest's directory.
pub fn parse_manifest(manifest_path: &Path) -> anyhow::Result<FileLayout> {
    let content = std::fs::read_to_string(manifest_path)?;
    let manifest: ManifestFile = toml::from_str(&content)?;

    // Resolve the target file path relative to the manifest location
    let target_path = if let Some(file) = &manifest.file {
        let p = PathBuf::from(file);
        if p.is_absolute() {
            p
        } else {
            manifest_path.parent().unwrap_or(Path::new(".")).join(p)
        }
    } else {
        // Default: strip .prefetch.toml from the manifest filename
        let name = manifest_path.to_string_lossy();
        let target = name.strip_suffix(".prefetch.toml")
            .ok_or_else(|| anyhow::anyhow!("cannot determine target file from manifest name"))?;
        PathBuf::from(target)
    };

    let file_size = if target_path.exists() {
        std::fs::metadata(&target_path)?.len()
    } else {
        0
    };

    let segments: Vec<Segment> = manifest
        .segments
        .into_iter()
        .enumerate()
        .map(|(i, s)| Segment {
            name: s.name,
            offset: s.offset,
            length: s.length,
            priority: s.priority.unwrap_or(i as u32),
        })
        .collect();

    let mut metadata = HashMap::new();
    if let Some(file) = &manifest.file {
        metadata.insert("target_file".to_string(), file.clone());
    }

    Ok(FileLayout {
        file_size,
        format_name: manifest.format.unwrap_or_else(|| "custom".to_string()),
        segments,
        metadata,
    })
}

#[derive(Deserialize)]
struct ManifestFile {
    /// Optional format name.
    format: Option<String>,
    /// Optional explicit path to the target file.
    file: Option<String>,
    /// Segments to prefetch.
    segments: Vec<ManifestSegment>,
}

#[derive(Deserialize)]
struct ManifestSegment {
    name: String,
    offset: u64,
    length: u64,
    priority: Option<u32>,
}
