use crate::{Result, StorageError};
use std::path::Path;

/// Trait to allow mocking disk space in unit tests and using OS stats in production
pub trait DiskSpaceChecker: Send + Sync {
    /// Return (free_bytes, total_bytes) for the filesystem containing `path`
    fn query_space(&self, path: &Path) -> Result<(u64, u64)>;

    /// Validate that performing a job with `required_bytes` respects the reserve percentage (default 15%)
    fn ensure_reserve(&self, path: &Path, required_bytes: u64, reserve_percent: f64) -> Result<()> {
        let (free_bytes, total_bytes) = self.query_space(path)?;
        let reserve_bytes = (total_bytes as f64 * (reserve_percent / 100.0)) as u64;

        if free_bytes < reserve_bytes || free_bytes.saturating_sub(required_bytes) < reserve_bytes {
            return Err(StorageError::InsufficientSpace {
                required_bytes,
                free_bytes,
                total_bytes,
                reserve_percent,
            });
        }
        Ok(())
    }
}

/// Production disk space checker using OS APIs
#[derive(Debug, Default, Clone)]
pub struct OsDiskSpaceChecker;

impl DiskSpaceChecker for OsDiskSpaceChecker {
    fn query_space(&self, path: &Path) -> Result<(u64, u64)> {
        #[cfg(unix)]
        {
            use std::ffi::CString;
            use std::os::unix::ffi::OsStrExt;

            let path_c = CString::new(path.as_os_str().as_bytes())
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
            let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
            if unsafe { libc::statvfs(path_c.as_ptr(), &mut stat) } != 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            let total_bytes = stat.f_blocks as u64 * stat.f_frsize as u64;
            let free_bytes = stat.f_bavail as u64 * stat.f_frsize as u64;
            Ok((free_bytes, total_bytes))
        }

        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;

            // Resolve to absolute path or directory
            let p = if path.exists() {
                path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
            } else {
                path.to_path_buf()
            };
            let mut wide: Vec<u16> = p.as_os_str().encode_wide().collect();
            wide.push(0);

            let mut free_bytes_available: u64 = 0;
            let mut total_number_of_bytes: u64 = 0;
            let mut total_number_of_free_bytes: u64 = 0;

            extern "system" {
                fn GetDiskFreeSpaceExW(
                    lpDirectoryName: *const u16,
                    lpFreeBytesAvailableToCaller: *mut u64,
                    lpTotalNumberOfBytes: *mut u64,
                    lpTotalNumberOfFreeBytes: *mut u64,
                ) -> i32;
            }

            let ret = unsafe {
                GetDiskFreeSpaceExW(
                    wide.as_ptr(),
                    &mut free_bytes_available,
                    &mut total_number_of_bytes,
                    &mut total_number_of_free_bytes,
                )
            };

            if ret == 0 {
                // If path does not exist yet, fallback to parent directory or root
                if let Some(parent) = path.parent() {
                    return self.query_space(parent);
                }
                return Err(std::io::Error::last_os_error().into());
            }

            Ok((free_bytes_available, total_number_of_bytes))
        }

        #[cfg(not(any(unix, windows)))]
        {
            // Fallback for unsupported platforms
            Ok((100 * 1024 * 1024 * 1024, 200 * 1024 * 1024 * 1024))
        }
    }
}

/// Mock disk space checker for unit tests
#[derive(Debug, Clone)]
pub struct MockDiskSpaceChecker {
    pub free_bytes: u64,
    pub total_bytes: u64,
}

impl MockDiskSpaceChecker {
    pub fn new(free_bytes: u64, total_bytes: u64) -> Self {
        Self {
            free_bytes,
            total_bytes,
        }
    }
}

impl DiskSpaceChecker for MockDiskSpaceChecker {
    fn query_space(&self, _path: &Path) -> Result<(u64, u64)> {
        Ok((self.free_bytes, self.total_bytes))
    }
}
