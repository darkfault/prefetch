//! File format providers for intelligent prefetching.
//!
//! A provider understands a file format's internal structure and can
//! describe it as ordered byte-range segments for optimal prefetching.
//!
//! Built-in providers:
//! - GGUF (LLM model files) — via the `prefetch-gguf` crate
//! - Manifest (user-defined `.prefetch.toml` files) — for any format
//!
//! External providers can implement the `FileProvider` trait.

pub mod manifest;

use std::collections::HashMap;
use std::path::Path;

/// A provider that understands a file format and can describe
/// its logical structure as ordered byte-range segments.
pub trait FileProvider: Send + Sync {
    /// Name of this provider (e.g., "gguf", "sqlite", "bam").
    fn name(&self) -> &str;

    /// Check if this provider can handle the given file.
    /// Typically checks magic bytes or file extension.
    fn can_handle(&self, path: &Path) -> bool;

    /// Parse the file and return its logical segments in recommended
    /// prefetch order. Each segment has a name, byte range, and priority.
    fn analyze(&self, path: &Path) -> anyhow::Result<FileLayout>;
}

/// The logical layout of a file, described as ordered segments.
///
/// This is format-agnostic — a GGUF model, a SQLite database, and a
/// game asset pack all produce the same structure.
#[derive(Debug, Clone)]
pub struct FileLayout {
    /// Total file size in bytes.
    pub file_size: u64,
    /// Name of the detected format (e.g., "GGUF", "SQLite", "Custom").
    pub format_name: String,
    /// Ordered segments to prefetch. Sorted by priority (lower = first).
    pub segments: Vec<Segment>,
    /// Format-specific metadata for display (e.g., "architecture: llama").
    pub metadata: HashMap<String, String>,
}

/// A named byte range within a file, with a prefetch priority.
#[derive(Debug, Clone)]
pub struct Segment {
    /// Human-readable name (e.g., "token_embedding", "index", "header").
    pub name: String,
    /// Absolute byte offset in the file.
    pub offset: u64,
    /// Byte length of this segment.
    pub length: u64,
    /// Prefetch priority — lower values are prefetched first.
    pub priority: u32,
}

impl FileLayout {
    /// Get segments sorted by priority (lowest first = prefetch first).
    pub fn ordered_segments(&self) -> Vec<&Segment> {
        let mut segs: Vec<&Segment> = self.segments.iter().collect();
        segs.sort_by_key(|s| s.priority);
        segs
    }

    /// Total bytes across all segments.
    pub fn total_segment_bytes(&self) -> u64 {
        self.segments.iter().map(|s| s.length).sum()
    }
}

/// Registry of file format providers. Tries each provider in order
/// to find one that can handle the given file.
pub struct ProviderRegistry {
    providers: Vec<Box<dyn FileProvider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self { providers: Vec::new() }
    }

    /// Register a new provider.
    pub fn register(&mut self, provider: Box<dyn FileProvider>) {
        tracing::debug!(provider = provider.name(), "registered file provider");
        self.providers.push(provider);
    }

    /// Try to detect the file format and return its layout.
    /// Tries providers in registration order, returns the first match.
    pub fn analyze(&self, path: &Path) -> Option<FileLayout> {
        // First check for a manifest file alongside the target
        let manifest_path = manifest::manifest_path_for(path);
        if manifest_path.exists() {
            match manifest::ManifestProvider.analyze(&manifest_path) {
                Ok(layout) => return Some(layout),
                Err(e) => tracing::debug!(error = %e, "manifest provider failed"),
            }
        }

        // Try registered providers
        for provider in &self.providers {
            if provider.can_handle(path) {
                match provider.analyze(path) {
                    Ok(layout) => {
                        tracing::debug!(
                            provider = provider.name(),
                            segments = layout.segments.len(),
                            "file format detected"
                        );
                        return Some(layout);
                    }
                    Err(e) => {
                        tracing::debug!(
                            provider = provider.name(),
                            error = %e,
                            "provider failed to analyze"
                        );
                    }
                }
            }
        }

        None
    }

    /// List registered provider names.
    pub fn provider_names(&self) -> Vec<&str> {
        self.providers.iter().map(|p| p.name()).collect()
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}
