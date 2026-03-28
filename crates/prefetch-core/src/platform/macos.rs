use std::os::fd::RawFd;

use super::PrefetchBackend;

/// macOS F_RDADVISE command for fcntl.
const F_RDADVISE: libc::c_int = 44;

/// macOS IO policy constants.
const IOPOL_TYPE_DISK: libc::c_int = 1;
const IOPOL_SCOPE_THREAD: libc::c_int = 2;
const IOPOL_UTILITY: libc::c_int = 2; // less aggressive than THROTTLE

#[repr(C)]
struct RadvisoryT {
    ra_offset: libc::off_t,
    ra_count: libc::c_int,
}

extern "C" {
    fn setiopolicy_np(
        iotype: libc::c_int,
        scope: libc::c_int,
        policy: libc::c_int,
    ) -> libc::c_int;
}

pub struct MacOSBackend {
    page_size: usize,
}

impl MacOSBackend {
    pub fn new() -> Self {
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as usize;
        Self { page_size }
    }
}

impl PrefetchBackend for MacOSBackend {
    fn advise_willneed(&self, fd: RawFd, offset: u64, len: u64) -> anyhow::Result<()> {
        // macOS doesn't have posix_fadvise. Use fcntl(F_RDADVISE) which
        // tells the kernel to begin reading the specified range into cache.
        //
        // F_RDADVISE takes a radvisory_t struct.
        // For large ranges, we issue multiple advisories in chunks since
        // ra_count is c_int (max ~2GB).
        let chunk_size: u64 = 1 << 30; // 1 GB chunks
        let mut remaining = len;
        let mut current_offset = offset;

        while remaining > 0 {
            let this_chunk = remaining.min(chunk_size) as libc::c_int;
            let advisory = RadvisoryT {
                ra_offset: current_offset as libc::off_t,
                ra_count: this_chunk,
            };

            let ret = unsafe {
                libc::fcntl(fd, F_RDADVISE, &advisory as *const RadvisoryT)
            };

            if ret == -1 {
                // F_RDADVISE can fail on some filesystems; fall back to
                // mmap + madvise approach
                return self.advise_willneed_mmap(fd, offset, len);
            }

            current_offset += this_chunk as u64;
            remaining -= this_chunk as u64;
        }

        Ok(())
    }

    fn query_residency(&self, addr: *const u8, len: usize) -> anyhow::Result<Vec<bool>> {
        let n_pages = (len + self.page_size - 1) / self.page_size;
        // macOS mincore uses `char *` (signed) for the vec parameter
        let mut vec: Vec<libc::c_char> = vec![0; n_pages];

        let ret = unsafe {
            libc::mincore(
                addr as *mut libc::c_void,
                len,
                vec.as_mut_ptr(),
            )
        };
        if ret != 0 {
            anyhow::bail!("mincore failed: {}", std::io::Error::last_os_error());
        }

        Ok(vec.iter().map(|&b| b & 1 != 0).collect())
    }

    fn page_size(&self) -> usize {
        self.page_size
    }

    fn set_low_io_priority(&self) -> anyhow::Result<()> {
        // Set IO throttle policy for this thread
        let ret = unsafe { setiopolicy_np(IOPOL_TYPE_DISK, IOPOL_SCOPE_THREAD, IOPOL_UTILITY) };
        if ret != 0 {
            tracing::warn!(
                "setiopolicy_np(UTILITY) failed: {} — prefetching will use normal IO priority",
                std::io::Error::last_os_error()
            );
        }

        // Also set nice level
        unsafe { libc::nice(19) };

        Ok(())
    }
}

impl MacOSBackend {
    /// Fallback: mmap the file range and use madvise(MADV_WILLNEED).
    fn advise_willneed_mmap(&self, fd: RawFd, offset: u64, len: u64) -> anyhow::Result<()> {
        // Align offset down to page boundary
        let page_offset = offset % self.page_size as u64;
        let aligned_offset = offset - page_offset;
        let aligned_len = len + page_offset;

        let addr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                aligned_len as usize,
                libc::PROT_READ,
                libc::MAP_PRIVATE,
                fd,
                aligned_offset as libc::off_t,
            )
        };

        if addr == libc::MAP_FAILED {
            anyhow::bail!("mmap failed for advise fallback: {}", std::io::Error::last_os_error());
        }

        let ret = unsafe {
            libc::madvise(addr, aligned_len as usize, libc::MADV_WILLNEED)
        };

        // Always unmap, regardless of madvise result
        unsafe { libc::munmap(addr, aligned_len as usize) };

        if ret != 0 {
            anyhow::bail!("madvise(WILLNEED) failed: {}", std::io::Error::last_os_error());
        }

        Ok(())
    }
}
