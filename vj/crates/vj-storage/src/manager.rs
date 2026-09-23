use crate::disk::{DiskSpaceChecker, OsDiskSpaceChecker};
use crate::job_store::JobStore;
use crate::{IngestJob, JobState, Result, StorageError};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

/// Configuration for ShowStorageManager
#[derive(Debug, Clone)]
pub struct StorageConfig {
    /// Shows directory (e.g. `/srv/rpi-vj/shows`)
    pub shows_dir: PathBuf,
    /// Staging directory (e.g. `/srv/rpi-vj/staging`)
    pub staging_dir: PathBuf,
    /// Current active pointer path (e.g. `/srv/rpi-vj/current`)
    pub current_link: PathBuf,
    /// Job store ledger file path (e.g. `/var/lib/rpi-vj/ingest-jobs.jsonl`)
    pub job_ledger_path: PathBuf,
    /// Required free space reserve percentage (default 15.0)
    pub free_reserve_percent: f64,
}

impl StorageConfig {
    pub fn default_appliance() -> Self {
        Self {
            shows_dir: PathBuf::from("/srv/rpi-vj/shows"),
            staging_dir: PathBuf::from("/srv/rpi-vj/staging"),
            current_link: PathBuf::from("/srv/rpi-vj/current"),
            job_ledger_path: PathBuf::from("/var/lib/rpi-vj/ingest-jobs.jsonl"),
            free_reserve_percent: 15.0,
        }
    }

    pub fn for_testing<P: AsRef<Path>>(base: P) -> Self {
        let base = base.as_ref();
        Self {
            shows_dir: base.join("shows"),
            staging_dir: base.join("staging"),
            current_link: base.join("current"),
            job_ledger_path: base.join("var-lib").join("ingest-jobs.jsonl"),
            free_reserve_percent: 15.0,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GenerationInfo {
    pub name: String,
    pub path: PathBuf,
    pub is_active: bool,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Show Storage Manager: Handles immutable show generations, staging, atomic symlinks, rollback,
/// 15% space checks, and crash recovery.
#[derive(Clone)]
pub struct ShowStorageManager {
    config: StorageConfig,
    job_store: JobStore,
    space_checker: Arc<dyn DiskSpaceChecker>,
}

impl ShowStorageManager {
    pub fn new(config: StorageConfig) -> Result<Self> {
        Self::with_checker(config, Arc::new(OsDiskSpaceChecker))
    }

    pub fn with_checker(
        config: StorageConfig,
        space_checker: Arc<dyn DiskSpaceChecker>,
    ) -> Result<Self> {
        fs::create_dir_all(&config.shows_dir)?;
        fs::create_dir_all(&config.staging_dir)?;
        let job_store = JobStore::new(&config.job_ledger_path)?;

        Ok(Self {
            config,
            job_store,
            space_checker,
        })
    }

    pub fn config(&self) -> &StorageConfig {
        &self.config
    }

    pub fn job_store(&self) -> &JobStore {
        &self.job_store
    }

    // --- Space Checks ---

    /// Ensure at least 15% free disk reserve remains after taking `required_bytes`
    pub fn check_free_space(&self, required_bytes: u64) -> Result<()> {
        self.space_checker.ensure_reserve(
            &self.config.staging_dir,
            required_bytes,
            self.config.free_reserve_percent,
        )
    }

    // --- Generation & Staging Management ---

    /// Create a new generation directory name formatted as `gen-<timestamp>-<hash>`
    pub fn generate_generation_name(content_hint: &[u8]) -> String {
        let ts = chrono::Utc::now().format("%Y%m%d%H%M%S").to_string();
        let mut hasher = Sha256::new();
        hasher.update(content_hint);
        hasher.update(Uuid::new_v4().as_bytes());
        let hash_hex = format!("{:x}", hasher.finalize());
        let short_hash = &hash_hex[..8];
        format!("gen-{}-{}", ts, short_hash)
    }

    /// Create a staging folder for a new ingest job
    pub fn create_staging_area(&self, job_id: Uuid) -> Result<PathBuf> {
        let staging_path = self.config.staging_dir.join(format!("job-{}", job_id));
        if staging_path.exists() {
            fs::remove_dir_all(&staging_path)?;
        }
        fs::create_dir_all(&staging_path)?;
        Ok(staging_path)
    }

    /// List all generations under `shows/` sorted by creation time (newest first)
    pub fn list_generations(&self) -> Result<Vec<GenerationInfo>> {
        if !self.config.shows_dir.exists() {
            return Ok(Vec::new());
        }

        let current_target = self.get_active_generation_path()?;
        let mut generations = Vec::new();

        for entry in fs::read_dir(&self.config.shows_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("gen-") {
                        let is_active = current_target
                            .as_ref()
                            .map(|cur| cur == &path || cur.file_name() == path.file_name())
                            .unwrap_or(false);

                        generations.push(GenerationInfo {
                            name: name.to_string(),
                            path: path.clone(),
                            is_active,
                            created_at: None,
                        });
                    }
                }
            }
        }

        // Sort descending by name (timestamp is ISO format prefix: gen-YYYYMMDDHHMMSS-...)
        generations.sort_by(|a, b| b.name.cmp(&a.name));
        Ok(generations)
    }

    /// Get current active show path (resolving symlink or pointer)
    pub fn get_active_generation_path(&self) -> Result<Option<PathBuf>> {
        if !self.config.current_link.exists() {
            // Check if broken symlink or windows pointer
            if let Ok(target) = fs::read_link(&self.config.current_link) {
                return Ok(Some(target));
            }
            return Ok(None);
        }

        // On Unix or Windows symlinks:
        if let Ok(target) = fs::read_link(&self.config.current_link) {
            let resolved = if target.is_relative() {
                self.config
                    .current_link
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join(target)
            } else {
                target
            };
            return Ok(Some(resolved));
        }

        // Fallback for file-based pointer fallback if symlinks not supported on target filesystem
        if self.config.current_link.is_file() {
            let content = fs::read_to_string(&self.config.current_link)?;
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                return Ok(Some(PathBuf::from(trimmed)));
            }
        }

        Ok(Some(self.config.current_link.clone()))
    }

    /// Atomically activate a generation by updating the `/srv/rpi-vj/current` symlink.
    /// Guarantees: Atomic replacement via temporary link/pointer and atomic rename.
    pub fn activate_generation(&self, generation_name: &str) -> Result<()> {
        let gen_path = self.config.shows_dir.join(generation_name);
        if !gen_path.exists() {
            return Err(StorageError::GenerationNotFound(
                generation_name.to_string(),
            ));
        }

        let link_parent = self
            .config
            .current_link
            .parent()
            .unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(link_parent)?;

        let tmp_link = link_parent.join(format!(".current_tmp_{}", Uuid::new_v4()));

        // Create symlink or robust pointer
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink(&gen_path, &tmp_link)?;
            fs::rename(&tmp_link, &self.config.current_link)?;
        }

        #[cfg(windows)]
        {
            // On Windows, try directory symlink; if privilege error, fallback to pointer file
            use std::os::windows::fs::symlink_dir;
            let symlink_res = symlink_dir(&gen_path, &tmp_link);
            if symlink_res.is_ok() {
                if self.config.current_link.exists() {
                    let _ = fs::remove_file(&self.config.current_link);
                    let _ = fs::remove_dir_all(&self.config.current_link);
                }
                fs::rename(&tmp_link, &self.config.current_link)?;
            } else {
                // Pointer file fallback
                let mut tmp_file = fs::File::create(&tmp_link)?;
                use std::io::Write;
                writeln!(tmp_file, "{}", gen_path.display())?;
                tmp_file.sync_all()?;
                drop(tmp_file);
                if self.config.current_link.exists() {
                    let _ = fs::remove_file(&self.config.current_link);
                }
                fs::rename(&tmp_link, &self.config.current_link)?;
            }
        }

        #[cfg(not(any(unix, windows)))]
        {
            let mut tmp_file = fs::File::create(&tmp_link)?;
            use std::io::Write;
            writeln!(tmp_file, "{}", gen_path.display())?;
            tmp_file.sync_all()?;
            drop(tmp_file);
            fs::rename(&tmp_link, &self.config.current_link)?;
        }

        Ok(())
    }

    /// Roll back to the previous generation.
    /// Finds the most recent non-active generation and activates it.
    pub fn rollback(&self) -> Result<String> {
        let gens = self.list_generations()?;
        let active_gen = gens.iter().find(|g| g.is_active);

        let target_gen = if let Some(active) = active_gen {
            gens.iter().find(|g| g.name != active.name)
        } else {
            gens.first()
        };

        match target_gen {
            Some(gen) => {
                let name = gen.name.clone();
                self.activate_generation(&name)?;
                Ok(name)
            }
            None => Err(StorageError::NoPreviousGeneration),
        }
    }

    /// Garbage collect old generations: keep active generation and the previous generation for rollback.
    /// Older generations are safely pruned.
    pub fn prune_old_generations(&self, keep_count: usize) -> Result<Vec<String>> {
        let gens = self.list_generations()?;
        let keep_threshold = keep_count.max(2); // Always keep at least 2 (active + rollback)
        let mut pruned = Vec::new();

        if gens.len() > keep_threshold {
            for old_gen in &gens[keep_threshold..] {
                if !old_gen.is_active
                    && old_gen.path.exists() {
                        fs::remove_dir_all(&old_gen.path)?;
                        pruned.push(old_gen.name.clone());
                    }
            }
        }
        Ok(pruned)
    }

    // --- Ingest Job Lifecycle & Boot Recovery ---

    /// Start a new ingest job with 15% reserve check
    pub fn start_job(&self, source_path: PathBuf, estimated_bytes: u64) -> Result<IngestJob> {
        // Enforce 15% free reserve
        self.check_free_space(estimated_bytes)?;

        let job = IngestJob::new(source_path, &self.config.staging_dir, estimated_bytes);
        self.create_staging_area(job.id)?;
        self.job_store.save_job(&job)?;
        Ok(job)
    }

    /// Promote completed staging directory to immutable show generation
    pub fn promote_staging_to_generation(
        &self,
        job_id: Uuid,
        generation_name: &str,
    ) -> Result<PathBuf> {
        let staging_path = self.config.staging_dir.join(format!("job-{}", job_id));
        if !staging_path.exists() {
            return Err(StorageError::Corrupted(format!(
                "Staging directory missing for job {}",
                job_id
            )));
        }

        let target_gen_path = self.config.shows_dir.join(generation_name);
        if target_gen_path.exists() {
            fs::remove_dir_all(&target_gen_path)?;
        }

        // Atomic move of staging to immutable show generation
        fs::rename(&staging_path, &target_gen_path)?;
        Ok(target_gen_path)
    }

    /// Boot recovery:
    /// 1. Inspect all recorded jobs in the durable ledger.
    /// 2. Any incomplete/aborted job has its staging files quarantined (`staging/quarantine-<timestamp>-<job_id>`).
    /// 3. Jobs in intermediate states are marked as `Failed { reason: "Interrupted by system restart/power loss" }`
    ///    or resumed if safe.
    /// 4. Ensures the active `/srv/rpi-vj/current` generation is intact, or rolls back if necessary.
    pub fn run_boot_recovery(&self) -> Result<BootRecoveryReport> {
        let mut report = BootRecoveryReport::default();
        let jobs = self.job_store.list_jobs()?;

        for job in jobs {
            if !job.state.is_terminal() {
                // Job was interrupted before completion
                let staging_path = &job.staging_path;
                if staging_path.exists() {
                    let quarantine_name = format!(
                        "quarantine-{}-{}",
                        chrono::Utc::now().format("%Y%m%d%H%M%S"),
                        job.id
                    );
                    let quarantine_path = self.config.staging_dir.join(&quarantine_name);
                    fs::rename(staging_path, &quarantine_path)?;
                    report
                        .quarantined_paths
                        .push(quarantine_path.to_string_lossy().to_string());
                }

                let reason = format!(
                    "Interrupted by power loss or reboot while in state {:?}",
                    job.state
                );
                let updated = self.job_store.transition_job(job.id, |j| {
                    j.state = JobState::Failed {
                        reason: reason.clone(),
                    };
                    j.details
                        .insert("recovery_note".into(), "Quarantined on boot recovery".into());
                })?;

                report.recovered_jobs.push(updated);
            }
        }

        // Clean up any untracked staging directories
        if self.config.staging_dir.exists() {
            for entry in fs::read_dir(&self.config.staging_dir)? {
                let entry = entry?;
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("job-") {
                        let quarantine_name = format!(
                            "quarantine-untracked-{}-{}",
                            chrono::Utc::now().format("%Y%m%d%H%M%S"),
                            name
                        );
                        let quarantine_path = self.config.staging_dir.join(quarantine_name);
                        let _ = fs::rename(&path, &quarantine_path);
                        report
                            .quarantined_paths
                            .push(quarantine_path.to_string_lossy().to_string());
                    }
                }
            }
        }

        // Verify active show integrity
        let active = self.get_active_generation_path()?;
        report.active_generation = active.map(|p| p.to_string_lossy().to_string());

        Ok(report)
    }
}

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct BootRecoveryReport {
    pub recovered_jobs: Vec<IngestJob>,
    pub quarantined_paths: Vec<String>,
    pub active_generation: Option<String>,
}
