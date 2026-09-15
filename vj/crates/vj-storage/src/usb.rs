use crate::job_store::JobStore;
use crate::manager::ShowStorageManager;
use crate::{IngestJob, JobState, Result, StorageError};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Whitelisted video extensions per Appliance V3 architecture
pub const SUPPORTED_VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "mov", "mkv", "webm", "avi", "m4v",
];

/// Known OS metadata / hidden directory / file patterns to ignore
pub const OS_METADATA_PATTERNS: &[&str] = &[
    "System Volume Information",
    "$RECYCLE.BIN",
    ".Trashes",
    ".Spotlight-V100",
    ".fseventsd",
    ".TemporaryItems",
    "lost+found",
    "Thumbs.db",
    "desktop.ini",
];

/// Information about a candidate media file discovered in `/RPI-VJ-INBOX/`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateFile {
    pub source_path: PathBuf,
    pub relative_path: PathBuf,
    pub file_name: String,
    pub size_bytes: u64,
    pub extension: String,
}

/// Report produced by scanning removable media
#[derive(Debug, Default, Clone)]
pub struct ScanReport {
    pub inbox_dir: PathBuf,
    pub candidates: Vec<CandidateFile>,
    pub ignored_symlinks: Vec<PathBuf>,
    pub ignored_os_metadata: Vec<PathBuf>,
    pub unsupported_files: Vec<PathBuf>,
    pub total_candidate_bytes: u64,
}

/// Check if a filename or path component represents OS metadata
pub fn is_os_metadata(name: &str) -> bool {
    if name.starts_with("._") || name.starts_with('.') {
        return true;
    }
    for pattern in OS_METADATA_PATTERNS {
        if name.eq_ignore_ascii_case(pattern) {
            return true;
        }
    }
    false
}

/// Check if extension is a supported video format
pub fn is_supported_video_ext(ext: &str) -> bool {
    let lower = ext.to_ascii_lowercase();
    SUPPORTED_VIDEO_EXTENSIONS.contains(&lower.as_str())
}

/// Sanitize filename to prevent path traversal, control chars, or invalid characters
pub fn sanitize_file_name(raw: &str) -> String {
    let filtered: String = raw
        .chars()
        .map(|c| match c {
            '/' | '\\' | '\0' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let trimmed = filtered.trim_matches('.').trim();
    if trimmed.is_empty() {
        "media_file".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Scanner that strictly searches for `/RPI-VJ-INBOX/` and enforces security boundaries
pub struct InboxScanner;

impl InboxScanner {
    /// Locate `/RPI-VJ-INBOX/` at the root of a mounted USB filesystem.
    /// Case-insensitive search across root directories.
    pub fn find_inbox_dir(mount_root: &Path) -> Option<PathBuf> {
        if !mount_root.exists() || !mount_root.is_dir() {
            return None;
        }

        // Exact match
        let exact = mount_root.join("RPI-VJ-INBOX");
        if exact.is_dir() {
            return Some(exact);
        }

        // Case-insensitive search at root level
        if let Ok(entries) = fs::read_dir(mount_root) {
            for entry in entries.flatten() {
                if let Ok(ft) = entry.file_type() {
                    if ft.is_dir() {
                        if let Some(name) = entry.file_name().to_str() {
                            if name.eq_ignore_ascii_case("RPI-VJ-INBOX") {
                                return Some(entry.path());
                            }
                        }
                    }
                }
            }
        }

        None
    }

    /// Scan exclusively within `/RPI-VJ-INBOX/`
    pub fn scan_inbox(mount_root: &Path) -> Result<ScanReport> {
        let inbox_dir = Self::find_inbox_dir(mount_root)
            .ok_or_else(|| StorageError::InboxNotFound(mount_root.to_path_buf()))?;

        let mut report = ScanReport {
            inbox_dir: inbox_dir.clone(),
            ..Default::default()
        };

        Self::scan_directory_recursive(&inbox_dir, &inbox_dir, &mut report)?;
        Ok(report)
    }

    fn scan_directory_recursive(
        inbox_root: &Path,
        current_dir: &Path,
        report: &mut ScanReport,
    ) -> Result<()> {
        let entries = fs::read_dir(current_dir)?;

        for entry_res in entries {
            let entry = entry_res?;
            let path = entry.path();
            let file_name_os = entry.file_name();
            let file_name = file_name_os.to_string_lossy().to_string();

            // 1. Check for symlinks via symlink_metadata (do not follow symlink)
            let meta = match fs::symlink_metadata(&path) {
                Ok(m) => m,
                Err(_) => {
                    continue;
                }
            };

            if meta.file_type().is_symlink() {
                // Reject/ignore symlinks to prevent directory traversal / arbitrary file reading
                report.ignored_symlinks.push(path.clone());
                continue;
            }

            // 2. Ignore hidden files, dotfiles, and OS metadata
            if is_os_metadata(&file_name) {
                report.ignored_os_metadata.push(path.clone());
                continue;
            }

            if meta.is_dir() {
                Self::scan_directory_recursive(inbox_root, &path, report)?;
            } else if meta.is_file() {
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();

                if is_supported_video_ext(&ext) {
                    let relative_path = path
                        .strip_prefix(inbox_root)
                        .unwrap_or(&path)
                        .to_path_buf();
                    let size_bytes = meta.len();

                    report.total_candidate_bytes += size_bytes;
                    report.candidates.push(CandidateFile {
                        source_path: path,
                        relative_path,
                        file_name,
                        size_bytes,
                        extension: ext,
                    });
                } else {
                    report.unsupported_files.push(path);
                }
            }
        }

        Ok(())
    }
}

/// Abstract mounter trait for mounting removable USB storage
pub trait MountProvider: Send + Sync {
    /// Mount device path (e.g. `/dev/sdb1`) to mount point with strict options
    fn mount(&self, device_path: &Path, mount_point: &Path, fstype: Option<&str>) -> Result<()>;
    /// Safely unmount mount point
    fn unmount(&self, mount_point: &Path) -> Result<()>;
}

/// Production system mount provider using standard Linux mount / umount
#[derive(Debug, Default, Clone)]
pub struct SystemMountProvider;

impl MountProvider for SystemMountProvider {
    #[allow(unused_variables)]
    fn mount(&self, device_path: &Path, mount_point: &Path, fstype: Option<&str>) -> Result<()> {
        fs::create_dir_all(mount_point)?;

        #[cfg(unix)]
        {
            use std::process::Command;

            let mut cmd = Command::new("mount");
            // Strict security mount options: nosuid,nodev,noexec,ro
            // For FAT32/exFAT, also set strict ownership and permission masks if possible
            let is_fat_or_exfat = fstype
                .map(|fs| fs.eq_ignore_ascii_case("vfat") || fs.eq_ignore_ascii_case("exfat"))
                .unwrap_or(false);

            let options = if is_fat_or_exfat {
                "nosuid,nodev,noexec,ro,uid=vj,gid=vj,fmask=0137,dmask=0027"
            } else {
                "nosuid,nodev,noexec,ro"
            };

            cmd.arg("-o").arg(options);

            if let Some(fs) = fstype {
                cmd.arg("-t").arg(fs);
            }

            cmd.arg(device_path).arg(mount_point);

            let output = cmd.output()?;
            if !output.status.success() {
                let err_msg = String::from_utf8_lossy(&output.stderr).to_string();
                return Err(StorageError::UsbMountFailed {
                    device: device_path.to_path_buf(),
                    mount_point: mount_point.to_path_buf(),
                    reason: err_msg,
                });
            }

            Ok(())
        }

        #[cfg(not(unix))]
        {
            // On non-unix test systems, verify paths exist
            if !device_path.exists() {
                return Err(StorageError::UsbMountFailed {
                    device: device_path.to_path_buf(),
                    mount_point: mount_point.to_path_buf(),
                    reason: "Device node does not exist".into(),
                });
            }
            Ok(())
        }
    }

    #[allow(unused_variables)]
    fn unmount(&self, mount_point: &Path) -> Result<()> {
        #[cfg(unix)]
        {
            use std::process::Command;

            let mut cmd = Command::new("umount");
            cmd.arg(mount_point);

            let output = cmd.output()?;
            if !output.status.success() {
                // Attempt lazy unmount if busy
                let mut lazy_cmd = Command::new("umount");
                lazy_cmd.arg("-l").arg(mount_point);
                let lazy_out = lazy_cmd.output()?;
                if !lazy_out.status.success() {
                    let err_msg = String::from_utf8_lossy(&output.stderr).to_string();
                    return Err(StorageError::UsbUnmountFailed {
                        mount_point: mount_point.to_path_buf(),
                        reason: err_msg,
                    });
                }
            }

            Ok(())
        }

        #[cfg(not(unix))]
        {
            Ok(())
        }
    }
}

/// Mock mount provider for deterministic unit and integration testing
#[derive(Debug, Default, Clone)]
pub struct MockMountProvider {
    mounted: Arc<std::sync::Mutex<HashSet<PathBuf>>>,
}

impl MockMountProvider {
    pub fn new() -> Self {
        Self {
            mounted: Arc::new(std::sync::Mutex::new(HashSet::new())),
        }
    }

    pub fn is_mounted(&self, mount_point: &Path) -> bool {
        self.mounted.lock().unwrap().contains(mount_point)
    }
}

impl MountProvider for MockMountProvider {
    fn mount(&self, device_path: &Path, mount_point: &Path, _fstype: Option<&str>) -> Result<()> {
        if !device_path.exists() {
            return Err(StorageError::UsbMountFailed {
                device: device_path.to_path_buf(),
                mount_point: mount_point.to_path_buf(),
                reason: "Mock device path does not exist".into(),
            });
        }
        fs::create_dir_all(mount_point)?;
        self.mounted
            .lock()
            .unwrap()
            .insert(mount_point.to_path_buf());
        Ok(())
    }

    fn unmount(&self, mount_point: &Path) -> Result<()> {
        self.mounted
            .lock()
            .unwrap()
            .remove(mount_point);
        Ok(())
    }
}

/// Result of processing a single candidate file
#[derive(Debug)]
pub enum FileIngestOutcome {
    Copied(IngestJob),
    Duplicate {
        job_id: uuid::Uuid,
        sha256: String,
        source_path: PathBuf,
    },
    SkippedAlreadyProcessed(IngestJob),
}

/// USB Media Copy Engine
/// Handles scanning, disk space reserve validation, transactional streamed copy, SHA-256 calculation,
/// deduplication, and safe cleanup/unmounting.
pub struct UsbCopyEngine {
    storage_mgr: ShowStorageManager,
    mount_provider: Arc<dyn MountProvider>,
}

impl UsbCopyEngine {
    pub fn new(storage_mgr: ShowStorageManager, mount_provider: Arc<dyn MountProvider>) -> Self {
        Self {
            storage_mgr,
            mount_provider,
        }
    }

    pub fn storage_manager(&self) -> &ShowStorageManager {
        &self.storage_mgr
    }

    pub fn mount_provider(&self) -> &Arc<dyn MountProvider> {
        &self.mount_provider
    }

    /// Process an entire mounted USB drive:
    /// 1. Scan exclusively for `/RPI-VJ-INBOX/`.
    /// 2. Check aggregate disk space (15% reserve invariant).
    /// 3. For each candidate file: transactional copy with SHA-256 computation to staging.
    /// 4. Deduplicate if identical hash already exists in ledger.
    /// 5. Safely unmount USB drive once queue completes.
    pub fn process_mount(&self, mount_point: &Path) -> Result<Vec<FileIngestOutcome>> {
        let scan_report = InboxScanner::scan_inbox(mount_point)?;
        let mut outcomes = Vec::new();

        // Enforce 15% reserve for the total candidate workload upfront
        if scan_report.total_candidate_bytes > 0 {
            self.storage_mgr
                .check_free_space(scan_report.total_candidate_bytes)?;
        }

        for candidate in &scan_report.candidates {
            let outcome = self.copy_candidate(candidate, || false)?;
            outcomes.push(outcome);
        }

        // Safely unmount after queue completes
        let _ = self.mount_provider.unmount(mount_point);

        Ok(outcomes)
    }

    /// Process a block device by mounting it, processing the inbox, and unmounting it
    pub fn process_device(
        &self,
        device_path: &Path,
        mount_point: &Path,
        fstype: Option<&str>,
    ) -> Result<Vec<FileIngestOutcome>> {
        self.mount_provider
            .mount(device_path, mount_point, fstype)?;

        let outcome_res = self.process_mount(mount_point);

        // Ensure unmount is attempted even if process_mount encounters error
        let _ = self.mount_provider.unmount(mount_point);

        outcome_res
    }

    /// Check if a job with identical source or SHA-256 already exists in JobStore
    pub fn find_existing_duplicate_job(
        job_store: &JobStore,
        source_path: &Path,
        sha256_hash: Option<&str>,
    ) -> Result<Option<IngestJob>> {
        let jobs = job_store.list_jobs()?;
        for job in jobs {
            // Check matching source path if completed or already in progress
            if job.source_path == source_path && (job.state == JobState::Complete || job.state == JobState::Probing || job.state == JobState::Ready) {
                return Ok(Some(job));
            }
            // Check matching SHA256 hash if provided (for any non-failed, non-cancelled job)
            if let Some(hash) = sha256_hash {
                if let Some(existing_hash) = job.details.get("sha256") {
                    if existing_hash == hash && !matches!(job.state, JobState::Failed { .. } | JobState::Cancelled) {
                        return Ok(Some(job));
                    }
                }
            }
        }
        Ok(None)
    }

    /// Copy a single candidate file to staging area (`/srv/rpi-vj/staging/job-<id>/<sanitized_name>`)
    /// with streaming SHA-256 calculation and guard checks.
    /// Supports `simulate_interrupt` closure for mid-copy removal testing.
    pub fn copy_candidate<F>(
        &self,
        candidate: &CandidateFile,
        simulate_interrupt: F,
    ) -> Result<FileIngestOutcome>
    where
        F: Fn() -> bool,
    {
        let job_store = self.storage_mgr.job_store();

        // 1. Re-insertion check: If a job for this exact source path already exists and completed, skip
        if let Some(existing) = Self::find_existing_duplicate_job(job_store, &candidate.source_path, None)? {
            return Ok(FileIngestOutcome::SkippedAlreadyProcessed(existing));
        }

        // 2. Validate 15% reserve space for this file
        self.storage_mgr.check_free_space(candidate.size_bytes)?;

        // 3. Create job in Discovered state & staging directory
        let job = self
            .storage_mgr
            .start_job(candidate.source_path.clone(), candidate.size_bytes)?;

        // 4. Transition to Copying state
        let job = job_store.transition_job(job.id, |j| {
            j.state = JobState::Copying;
            j.progress_percent = 0.0;
            j.details.insert("original_file_name".into(), candidate.file_name.clone());
            j.details.insert(
                "relative_path".into(),
                candidate.relative_path.to_string_lossy().to_string(),
            );
        })?;

        // 5. Open source file (read-only)
        let mut src_file = match File::open(&candidate.source_path) {
            Ok(f) => f,
            Err(e) => {
                let reason = format!("Failed to open source file {}: {}", candidate.source_path.display(), e);
                job_store.transition_job(job.id, |j| {
                    j.state = JobState::Failed {
                        reason: reason.clone(),
                    };
                })?;
                return Err(StorageError::MidCopyRemoval {
                    path: candidate.source_path.clone(),
                    reason,
                });
            }
        };

        // 6. Filename collision safety: place in isolated staging directory
        let sanitized_name = sanitize_file_name(&candidate.file_name);
        let dest_path = job.staging_path.join(&sanitized_name);

        let mut dest_file = match OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&dest_path)
        {
            Ok(f) => f,
            Err(e) => {
                let reason = format!("Failed to create staging file {}: {}", dest_path.display(), e);
                job_store.transition_job(job.id, |j| {
                    j.state = JobState::Failed {
                        reason: reason.clone(),
                    };
                })?;
                return Err(StorageError::Io(e));
            }
        };

        // 7. Stream copy in 64KB chunks while computing SHA-256
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        let mut total_copied: u64 = 0;
        let mut last_progress_report = 0.0f32;

        loop {
            // Mid-copy removal / interrupt simulation check
            if simulate_interrupt() {
                // Clean up partial staging file
                let _ = fs::remove_file(&dest_path);
                let reason = format!(
                    "Drive removed or communication lost mid-copy after {} bytes",
                    total_copied
                );
                job_store.transition_job(job.id, |j| {
                    j.state = JobState::Failed {
                        reason: reason.clone(),
                    };
                })?;
                return Err(StorageError::MidCopyRemoval {
                    path: candidate.source_path.clone(),
                    reason,
                });
            }

            let read_bytes = match src_file.read(&mut buffer) {
                Ok(0) => break, // EOF reached
                Ok(n) => n,
                Err(e) => {
                    // Read error (e.g. device disconnected / unplugged)
                    let _ = fs::remove_file(&dest_path);
                    let reason = format!(
                        "Mid-copy read failure at byte {}: {}",
                        total_copied, e
                    );
                    job_store.transition_job(job.id, |j| {
                        j.state = JobState::Failed {
                            reason: reason.clone(),
                        };
                    })?;
                    return Err(StorageError::MidCopyRemoval {
                        path: candidate.source_path.clone(),
                        reason,
                    });
                }
            };

            if let Err(e) = dest_file.write_all(&buffer[..read_bytes]) {
                let _ = fs::remove_file(&dest_path);
                let reason = format!("Failed writing to staging at byte {}: {}", total_copied, e);
                job_store.transition_job(job.id, |j| {
                    j.state = JobState::Failed {
                        reason: reason.clone(),
                    };
                })?;
                return Err(StorageError::Io(e));
            }

            hasher.update(&buffer[..read_bytes]);
            total_copied += read_bytes as u64;

            // Report progress in increments of >= 10%
            if candidate.size_bytes > 0 {
                let progress = ((total_copied as f64 / candidate.size_bytes as f64) * 100.0) as f32;
                if progress - last_progress_report >= 10.0 || total_copied == candidate.size_bytes {
                    let _ = job_store.transition_job(job.id, |j| {
                        j.progress_percent = progress.min(100.0);
                    });
                    last_progress_report = progress;
                }
            }
        }

        // 8. Short read detection
        if total_copied != candidate.size_bytes {
            let _ = fs::remove_file(&dest_path);
            let reason = format!(
                "Short read detected: expected {} bytes but only received {} bytes",
                candidate.size_bytes, total_copied
            );
            job_store.transition_job(job.id, |j| {
                j.state = JobState::Failed {
                    reason: reason.clone(),
                };
            })?;
            return Err(StorageError::ShortRead {
                path: candidate.source_path.clone(),
                expected: candidate.size_bytes,
                actual: total_copied,
            });
        }

        // 9. fsync durable write on staging file
        dest_file.sync_all()?;
        drop(dest_file);

        let final_hash = format!("{:x}", hasher.finalize());

        // 10. Duplicate content hash detection
        if let Some(dup_job) = Self::find_existing_duplicate_job(job_store, &candidate.source_path, Some(&final_hash))? {
            // Already present in library / prior job: remove duplicate staging file
            let _ = fs::remove_file(&dest_path);
            let _ = fs::remove_dir_all(&job.staging_path);

            let _ = job_store.transition_job(job.id, |j| {
                j.state = JobState::Cancelled;
                j.details.insert("sha256".into(), final_hash.clone());
                j.details.insert("duplicate_of".into(), dup_job.id.to_string());
                j.details.insert("duplicate_reason".into(), "Identical SHA-256 hash match".into());
            });

            return Ok(FileIngestOutcome::Duplicate {
                job_id: dup_job.id,
                sha256: final_hash,
                source_path: candidate.source_path.clone(),
            });
        }

        // 11. Advance job to Probing state with recorded sha256 hash
        let final_job = job_store.transition_job(job.id, |j| {
            j.state = JobState::Probing;
            j.progress_percent = 100.0;
            j.details.insert("sha256".into(), final_hash);
            j.details.insert("staged_file".into(), dest_path.to_string_lossy().to_string());
            j.details.insert("bytes_copied".into(), total_copied.to_string());
        })?;

        Ok(FileIngestOutcome::Copied(final_job))
    }
}
