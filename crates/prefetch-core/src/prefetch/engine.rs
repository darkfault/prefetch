use std::os::fd::AsRawFd;
use std::path::Path;

use crate::cache_status::{self, CacheStatus};
use crate::platform::{self, PrefetchBackend};
use crate::prefetch::progress::{PrefetchProgress, ProgressTracker};
use crate::prefetch::strategy::{MemoryBudget, PrefetchStrategy};
use crate::providers::{FileLayout, ProviderRegistry};

/// Result of a completed prefetch operation.
#[derive(Debug)]
pub struct PrefetchResult {
    pub progress: PrefetchProgress,
    pub cache_status: CacheStatus,
    pub budget_limited: bool,
}

/// The main prefetch engine that orchestrates cache warming.
///
/// Works with any file format. When a `ProviderRegistry` is set,
/// it auto-detects the file format and prefetches segments in
/// priority order. Falls back to sequential for unknown formats.
pub struct PrefetchEngine {
    backend: Box<dyn PrefetchBackend>,
    budget: MemoryBudget,
    chunk_size: u64,
    registry: ProviderRegistry,
}

impl PrefetchEngine {
    pub fn new() -> Self {
        Self {
            backend: platform::create_backend(),
            budget: MemoryBudget::default(),
            chunk_size: 64 * 1024 * 1024,
            registry: ProviderRegistry::new(),
        }
    }

    pub fn with_config(budget: MemoryBudget, chunk_size_mb: u64) -> Self {
        Self {
            backend: platform::create_backend(),
            budget,
            chunk_size: chunk_size_mb * 1024 * 1024,
            registry: ProviderRegistry::new(),
        }
    }

    /// Register a file format provider for format-aware prefetching.
    pub fn register_provider(&mut self, provider: Box<dyn crate::providers::FileProvider>) {
        self.registry.register(provider);
    }

    pub fn set_low_priority(&self) -> anyhow::Result<()> {
        self.backend.set_low_io_priority()
    }

    /// Prefetch a file into the page cache.
    ///
    /// Tries registered providers to detect the file format and prefetch
    /// segments in priority order. Falls back to sequential for unknown formats.
    pub fn prefetch_file(
        &self,
        path: &Path,
        strategy: &PrefetchStrategy,
        mut on_progress: impl FnMut(&PrefetchProgress),
    ) -> anyhow::Result<PrefetchResult> {
        // SECURITY: Validate the file before operating on it.
        let metadata = std::fs::symlink_metadata(path)?;
        if !metadata.is_file() && !metadata.file_type().is_symlink() {
            anyhow::bail!("refusing to prefetch non-regular file: {}", path.display());
        }
        if metadata.file_type().is_symlink() {
            let resolved = std::fs::canonicalize(path)?;
            if !std::fs::metadata(&resolved)?.is_file() {
                anyhow::bail!("symlink {} resolves to non-regular file", path.display());
            }
        }

        let file = std::fs::File::open(path)?;
        let file_size = file.metadata()?.len();
        let fd = file.as_raw_fd();

        tracing::info!(
            path = %path.display(),
            size_mb = file_size / (1024 * 1024),
            strategy = %strategy,
            "starting prefetch"
        );

        let result = match strategy {
            PrefetchStrategy::Sequential => {
                self.prefetch_sequential(path, fd, file_size, &mut on_progress)?
            }
            PrefetchStrategy::InferenceOrder | PrefetchStrategy::FirstNLayers(_) => {
                // Try providers to get structured layout
                match self.registry.analyze(path) {
                    Some(layout) => {
                        tracing::info!(format = %layout.format_name, "detected file format");
                        self.prefetch_structured(path, fd, &layout, strategy, &mut on_progress)?
                    }
                    None => {
                        tracing::info!("no provider matched, using sequential prefetch");
                        self.prefetch_sequential(path, fd, file_size, &mut on_progress)?
                    }
                }
            }
        };

        tracing::info!(
            cached_percent = format!("{:.1}%", result.cache_status.cached_percent()),
            elapsed_ms = result.progress.elapsed.as_millis(),
            throughput_mbps = format!("{:.1}", result.progress.throughput_mbps()),
            "prefetch complete"
        );

        Ok(result)
    }

    /// Backward-compatible alias for `prefetch_file`.
    pub fn prefetch_model(
        &self,
        path: &Path,
        strategy: &PrefetchStrategy,
        on_progress: impl FnMut(&PrefetchProgress),
    ) -> anyhow::Result<PrefetchResult> {
        self.prefetch_file(path, strategy, on_progress)
    }

    /// Query cache status, using providers for segment-level detail.
    pub fn cache_status(&self, path: &Path) -> anyhow::Result<CacheStatus> {
        let layout = self.registry.analyze(path);
        cache_status::query_cache_status(path, self.backend.as_ref(), layout.as_ref())
    }

    /// Analyze a file's structure using registered providers.
    pub fn analyze(&self, path: &Path) -> Option<FileLayout> {
        self.registry.analyze(path)
    }

    fn prefetch_sequential(
        &self,
        path: &Path,
        fd: std::os::fd::RawFd,
        file_size: u64,
        on_progress: &mut impl FnMut(&PrefetchProgress),
    ) -> anyhow::Result<PrefetchResult> {
        let mut tracker = ProgressTracker::new(file_size, 1);
        tracker.set_current_layer("sequential".to_string());
        let mut budget_limited = false;

        let mut offset = 0u64;
        while offset < file_size {
            if !self.budget.should_continue(offset) {
                budget_limited = true;
                break;
            }
            let chunk = self.chunk_size.min(file_size - offset);
            self.backend.advise_willneed(fd, offset, chunk)?;
            offset += chunk;
            tracker.add_bytes(chunk);
            on_progress(&tracker.snapshot());
        }
        tracker.complete_layer();

        let cache_status = cache_status::query_cache_status(path, self.backend.as_ref(), None)?;
        Ok(PrefetchResult { progress: tracker.snapshot(), cache_status, budget_limited })
    }

    /// Structure-aware prefetch using a FileLayout from any provider.
    fn prefetch_structured(
        &self,
        path: &Path,
        fd: std::os::fd::RawFd,
        layout: &FileLayout,
        strategy: &PrefetchStrategy,
        on_progress: &mut impl FnMut(&PrefetchProgress),
    ) -> anyhow::Result<PrefetchResult> {
        let segments: Vec<_> = match strategy {
            PrefetchStrategy::InferenceOrder => layout.ordered_segments(),
            PrefetchStrategy::FirstNLayers(n) => {
                layout.ordered_segments()
                    .into_iter()
                    .take(*n as usize + 1) // +1 for embedding/header
                    .collect()
            }
            PrefetchStrategy::Sequential => unreachable!(),
        };

        let total_bytes: u64 = segments.iter().map(|s| s.length).sum();
        let mut tracker = ProgressTracker::new(total_bytes, segments.len());
        let mut budget_limited = false;

        for segment in &segments {
            tracker.set_current_layer(segment.name.clone());

            if !self.budget.should_continue(tracker.snapshot().bytes_advised) {
                budget_limited = true;
                break;
            }

            let mut offset = segment.offset;
            let end = segment.offset + segment.length;
            while offset < end {
                let chunk = self.chunk_size.min(end - offset);
                self.backend.advise_willneed(fd, offset, chunk)?;
                offset += chunk;
                tracker.add_bytes(chunk);
                on_progress(&tracker.snapshot());
            }

            tracker.complete_layer();
            on_progress(&tracker.snapshot());
        }

        let cache_status = cache_status::query_cache_status(path, self.backend.as_ref(), Some(layout))?;
        Ok(PrefetchResult { progress: tracker.snapshot(), cache_status, budget_limited })
    }
}
