use std::os::fd::RawFd;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;

/// Platform-specific backend for page cache operations.
///
/// Implementations use OS-specific syscalls to advise the kernel
/// about memory access patterns and query page cache residency.
pub trait PrefetchBackend: Send + Sync {
    /// Advise the kernel that the byte range [offset, offset+len) of the
    /// file will be needed soon. This triggers asynchronous readahead
    /// into the page cache without blocking.
    fn advise_willneed(&self, fd: RawFd, offset: u64, len: u64) -> anyhow::Result<()>;

    /// Query which pages of the given memory-mapped region are currently
    /// resident in physical memory (page cache).
    ///
    /// Returns a vector of booleans, one per page, where `true` means
    /// the page is in the cache.
    fn query_residency(&self, addr: *const u8, len: usize) -> anyhow::Result<Vec<bool>>;

    /// Get the system page size in bytes.
    fn page_size(&self) -> usize;

    /// Set the current thread to use low IO priority so prefetching
    /// doesn't interfere with interactive workloads.
    fn set_low_io_priority(&self) -> anyhow::Result<()>;
}

/// Create the platform-appropriate prefetch backend.
pub fn create_backend() -> Box<dyn PrefetchBackend> {
    #[cfg(target_os = "linux")]
    {
        Box::new(linux::LinuxBackend::new())
    }
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacOSBackend::new())
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        compile_error!("prefetch only supports Linux and macOS")
    }
}
