use std::os::fd::RawFd;

use super::PrefetchBackend;

pub struct LinuxBackend {
    page_size: usize,
}

impl LinuxBackend {
    pub fn new() -> Self {
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as usize;
        Self { page_size }
    }
}

impl PrefetchBackend for LinuxBackend {
    fn advise_willneed(&self, fd: RawFd, offset: u64, len: u64) -> anyhow::Result<()> {
        // posix_fadvise works on file descriptors without needing mmap.
        // It tells the kernel to start reading the specified range into
        // the page cache asynchronously.
        let ret = unsafe {
            libc::posix_fadvise(
                fd,
                offset as libc::off_t,
                len as libc::off_t,
                libc::POSIX_FADV_WILLNEED,
            )
        };
        if ret != 0 {
            anyhow::bail!("posix_fadvise(WILLNEED) failed: {}", std::io::Error::from_raw_os_error(ret));
        }
        Ok(())
    }

    fn query_residency(&self, addr: *const u8, len: usize) -> anyhow::Result<Vec<bool>> {
        let n_pages = (len + self.page_size - 1) / self.page_size;
        let mut vec: Vec<u8> = vec![0u8; n_pages];

        let ret = unsafe {
            libc::mincore(
                addr as *mut libc::c_void,
                len,
                vec.as_mut_ptr() as *mut libc::c_uchar,
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
        // ioprio_set syscall: set IO scheduling class to IDLE
        // IOPRIO_WHO_PROCESS = 1, pid 0 = current process
        // IOPRIO_CLASS_IDLE = 3, data = 0
        // ioprio = (class << 13) | data
        let ioprio: libc::c_int = (3 << 13) | 0;
        let ret = unsafe { libc::syscall(libc::SYS_ioprio_set, 1i32, 0i32, ioprio) };
        if ret != 0 {
            tracing::warn!(
                "ioprio_set(IDLE) failed: {} — prefetching will use normal IO priority",
                std::io::Error::last_os_error()
            );
        }

        // Also set nice level to lowest priority
        unsafe { libc::nice(19) };

        Ok(())
    }
}
