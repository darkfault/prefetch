use std::path::Path;

use memmap2::Mmap;

use crate::platform::PrefetchBackend;

/// Page cache residency status for a model file.
#[derive(Debug, Clone)]
pub struct CacheStatus {
    /// Total file size in bytes.
    pub file_size: u64,
    /// Number of pages total.
    pub total_pages: usize,
    /// Number of pages currently in cache.
    pub cached_pages: usize,
    /// Per-layer cache status (if GGUF layout is available).
    pub layer_status: Vec<LayerCacheStatus>,
}

impl CacheStatus {
    /// Overall percentage of the file in page cache.
    pub fn cached_percent(&self) -> f64 {
        if self.total_pages == 0 {
            return 0.0;
        }
        (self.cached_pages as f64 / self.total_pages as f64) * 100.0
    }

    /// Bytes currently cached.
    pub fn cached_bytes(&self) -> u64 {
        if self.total_pages == 0 {
            return 0;
        }
        (self.file_size as f64 * self.cached_pages as f64 / self.total_pages as f64) as u64
    }
}

/// Cache status for a single logical layer.
#[derive(Debug, Clone)]
pub struct LayerCacheStatus {
    pub layer_name: String,
    pub total_bytes: u64,
    pub total_pages: usize,
    pub cached_pages: usize,
}

impl LayerCacheStatus {
    pub fn cached_percent(&self) -> f64 {
        if self.total_pages == 0 {
            return 0.0;
        }
        (self.cached_pages as f64 / self.total_pages as f64) * 100.0
    }
}

/// Validate a file path for safe mmap operations.
///
/// Checks:
/// - File is a regular file (not a device, pipe, socket, etc.)
/// - File is not a symlink pointing outside expected directories (if base_dir given)
/// - File size is non-zero and within reasonable bounds
fn validate_file_for_mmap(path: &Path) -> anyhow::Result<std::fs::File> {
    let metadata = std::fs::symlink_metadata(path)?;

    // SECURITY: Reject non-regular files (devices, pipes, sockets).
    // mmap'ing /dev/mem or similar could be dangerous.
    if !metadata.is_file() && !metadata.file_type().is_symlink() {
        anyhow::bail!(
            "refusing to mmap non-regular file: {} (type: {:?})",
            path.display(),
            metadata.file_type()
        );
    }

    // SECURITY: If it's a symlink, resolve and log the target.
    // We allow symlinks but warn about them.
    if metadata.file_type().is_symlink() {
        let resolved = std::fs::canonicalize(path)?;
        let resolved_meta = std::fs::metadata(&resolved)?;
        if !resolved_meta.is_file() {
            anyhow::bail!(
                "symlink {} resolves to non-regular file: {}",
                path.display(),
                resolved.display()
            );
        }
        tracing::debug!(
            symlink = %path.display(),
            target = %resolved.display(),
            "following symlink to model file"
        );
    }

    let file = std::fs::File::open(path)?;
    let file_size = file.metadata()?.len();

    if file_size == 0 {
        anyhow::bail!("file is empty: {}", path.display());
    }

    Ok(file)
}

/// Safely mmap a file and run mincore, handling potential SIGBUS from
/// file truncation during the operation.
///
/// We verify the file size before and after mmap to detect truncation.
/// A full SIGBUS handler would require `sigaction` + `siglongjmp` which
/// adds significant complexity; instead we use size validation as a
/// practical defense.
fn safe_mmap_query(
    file: &std::fs::File,
    backend: &dyn PrefetchBackend,
) -> anyhow::Result<(u64, Vec<bool>)> {
    let file_size = file.metadata()?.len();

    // Safety: we only use this mapping for mincore queries, and we've
    // validated the file is a regular file with non-zero size.
    let mmap = unsafe { Mmap::map(file)? };

    // SECURITY: Verify size hasn't changed since we opened the file.
    // If the file was truncated between open() and mmap(), the mmap
    // region extends beyond the file and accessing those pages would
    // cause SIGBUS. Re-check the actual mapped length.
    let current_size = file.metadata()?.len();
    if current_size != file_size {
        // Drop the mmap before bailing to unmap cleanly
        drop(mmap);
        anyhow::bail!(
            "file size changed during mmap ({} -> {}), possible truncation",
            file_size,
            current_size
        );
    }

    let query_len = mmap.len().min(current_size as usize);
    let residency = backend.query_residency(mmap.as_ptr(), query_len)?;

    Ok((file_size, residency))
}

/// Query the page cache status of a file.
///
/// This mmaps the file read-only, calls mincore() to check which pages
/// are resident, then immediately unmaps.
///
/// If a `FileLayout` is provided (from any format provider), the result
/// includes per-segment cache breakdowns.
pub fn query_cache_status(
    path: &Path,
    backend: &dyn PrefetchBackend,
    layout: Option<&crate::providers::FileLayout>,
) -> anyhow::Result<CacheStatus> {
    let file = validate_file_for_mmap(path)?;
    let file_size = file.metadata()?.len();

    if file_size == 0 {
        return Ok(CacheStatus {
            file_size: 0,
            total_pages: 0,
            cached_pages: 0,
            layer_status: Vec::new(),
        });
    }

    let (file_size, residency) = safe_mmap_query(&file, backend)?;

    let total_pages = residency.len();
    let cached_pages = residency.iter().filter(|&&b| b).count();

    // Per-segment cache status (works with any provider's layout)
    let page_size = backend.page_size() as u64;
    let layer_status = if let Some(layout) = layout {
        layout
            .segments
            .iter()
            .map(|segment| {
                let start_page = (segment.offset / page_size) as usize;
                let end = segment.offset + segment.length;
                let end_page = ((end + page_size - 1) / page_size) as usize;
                let end_page = end_page.min(total_pages);

                let seg_pages = if start_page < end_page {
                    &residency[start_page..end_page]
                } else {
                    &[]
                };

                LayerCacheStatus {
                    layer_name: segment.name.clone(),
                    total_bytes: segment.length,
                    total_pages: seg_pages.len(),
                    cached_pages: seg_pages.iter().filter(|&&b| b).count(),
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    Ok(CacheStatus {
        file_size,
        total_pages,
        cached_pages,
        layer_status,
    })
}
