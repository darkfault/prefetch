//! Core prefetch engine with platform-specific page cache warming.
//!
//! This crate provides the `PrefetchEngine` which orchestrates intelligent
//! pre-loading of files into the OS page cache, with pluggable format
//! providers for structure-aware prefetching.
//!
//! Built-in support for GGUF (LLM models) and user-defined manifest files.
//! Any file format can be supported by implementing the `FileProvider` trait.

pub mod platform;
pub mod prefetch;
pub mod providers;
pub mod cache_status;

pub use cache_status::CacheStatus;
pub use prefetch::engine::PrefetchEngine;
pub use prefetch::strategy::{MemoryBudget, PrefetchStrategy};
pub use providers::{FileLayout, FileProvider, ProviderRegistry, Segment};
