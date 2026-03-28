use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::paths;

/// A discovered model with its resolved file path.
#[derive(Debug, Clone)]
pub struct DiscoveredModel {
    /// Human-readable name (e.g., "llama3:latest").
    pub name: String,
    /// Absolute path to the GGUF blob file.
    pub path: PathBuf,
    /// File size in bytes.
    pub size: u64,
}

/// Discover all Ollama models by reading manifest files.
///
/// Ollama stores models as content-addressed blobs:
/// ```text
/// ~/.ollama/models/
///   manifests/registry.ollama.ai/library/{model}/{tag}  (JSON)
///   blobs/sha256-{digest}                                (GGUF data)
/// ```
pub fn discover_ollama_models() -> Vec<DiscoveredModel> {
    let models_dir = paths::ollama_models_dir();
    discover_ollama_models_in(&models_dir)
}

/// Discover Ollama models in a specific directory.
pub fn discover_ollama_models_in(models_dir: &Path) -> Vec<DiscoveredModel> {
    let manifests_dir = models_dir.join("manifests");
    let blobs_dir = models_dir.join("blobs");

    if !manifests_dir.exists() || !blobs_dir.exists() {
        tracing::debug!(
            path = %models_dir.display(),
            "Ollama models directory not found or incomplete"
        );
        return Vec::new();
    }

    let mut models = Vec::new();

    // Walk the manifests directory tree
    if let Ok(entries) = walk_manifests(&manifests_dir) {
        for (display_name, manifest_path) in entries {
            match parse_manifest_and_resolve(&manifest_path, &blobs_dir) {
                Ok(Some(model)) => {
                    models.push(DiscoveredModel {
                        name: display_name,
                        path: model.path,
                        size: model.size,
                    });
                }
                Ok(None) => {
                    tracing::debug!(
                        manifest = %manifest_path.display(),
                        "no model layer found in manifest"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        manifest = %manifest_path.display(),
                        error = %e,
                        "failed to parse Ollama manifest"
                    );
                }
            }
        }
    }

    models.sort_by(|a, b| a.name.cmp(&b.name));
    models
}

/// Walk manifests directory to find all manifest files.
/// Returns (display_name, path) pairs.
fn walk_manifests(manifests_dir: &Path) -> anyhow::Result<Vec<(String, PathBuf)>> {
    let mut results = Vec::new();
    walk_manifests_recursive(manifests_dir, manifests_dir, &mut results)?;
    Ok(results)
}

fn walk_manifests_recursive(
    base: &Path,
    dir: &Path,
    results: &mut Vec<(String, PathBuf)>,
) -> anyhow::Result<()> {
    let entries = std::fs::read_dir(dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk_manifests_recursive(base, &path, results)?;
        } else if path.is_file() {
            // Build display name from path relative to manifests dir
            // e.g., registry.ollama.ai/library/llama3/latest -> llama3:latest
            if let Some(rel) = path.strip_prefix(base).ok() {
                let parts: Vec<_> = rel.components()
                    .map(|c| c.as_os_str().to_string_lossy().to_string())
                    .collect();
                // Typical: registry.ollama.ai / library / {model} / {tag}
                let display_name = if parts.len() >= 4 {
                    format!("{}:{}", parts[parts.len() - 2], parts[parts.len() - 1])
                } else {
                    rel.display().to_string()
                };
                results.push((display_name, path));
            }
        }
    }
    Ok(())
}

/// Ollama manifest structure (simplified).
#[derive(Deserialize)]
struct OllamaManifest {
    layers: Vec<ManifestLayer>,
}

#[derive(Deserialize)]
struct ManifestLayer {
    digest: String,
    #[serde(rename = "mediaType")]
    media_type: String,
    #[allow(dead_code)]
    size: u64,
}

struct ResolvedBlob {
    path: PathBuf,
    size: u64,
}

/// Parse a manifest file and resolve the model blob path.
fn parse_manifest_and_resolve(
    manifest_path: &Path,
    blobs_dir: &Path,
) -> anyhow::Result<Option<ResolvedBlob>> {
    let content = std::fs::read_to_string(manifest_path)?;
    let manifest: OllamaManifest = serde_json::from_str(&content)?;

    // Find the model layer (contains the actual GGUF weights)
    let model_layer = manifest.layers.iter().find(|l| {
        l.media_type.contains("model")
            || l.media_type.contains("gguf")
            || l.media_type == "application/vnd.ollama.image.model"
    });

    let layer = match model_layer {
        Some(l) => l,
        None => return Ok(None),
    };

    // Digest format: "sha256:abc123..." -> blob file "sha256-abc123..."
    let blob_name = layer.digest.replace(':', "-");

    // SECURITY: Validate digest doesn't contain path traversal characters.
    // A crafted manifest with digest "sha256:../../../etc/shadow" would
    // otherwise resolve to an arbitrary path.
    if blob_name.contains('/')
        || blob_name.contains('\\')
        || blob_name.contains("..")
        || blob_name.contains('\0')
    {
        tracing::warn!(
            digest = &layer.digest,
            "rejecting digest with path traversal characters"
        );
        return Ok(None);
    }

    let blob_path = blobs_dir.join(&blob_name);

    // SECURITY: Canonicalize and verify the resolved path is still under blobs_dir.
    // This catches symlink-based traversal attacks.
    let canonical_blob = match blob_path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            tracing::warn!(
                digest = &layer.digest,
                path = %blob_path.display(),
                "blob file not found"
            );
            return Ok(None);
        }
    };
    let canonical_blobs_dir = blobs_dir.canonicalize().unwrap_or_else(|_| blobs_dir.to_path_buf());

    if !canonical_blob.starts_with(&canonical_blobs_dir) {
        tracing::warn!(
            digest = &layer.digest,
            resolved = %canonical_blob.display(),
            expected_under = %canonical_blobs_dir.display(),
            "rejecting blob path outside blobs directory (possible path traversal)"
        );
        return Ok(None);
    }

    let actual_size = canonical_blob.metadata()?.len();

    Ok(Some(ResolvedBlob {
        path: canonical_blob,
        size: actual_size,
    }))
}
