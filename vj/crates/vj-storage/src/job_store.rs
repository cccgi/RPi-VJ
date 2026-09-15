use crate::{IngestJob, Result, StorageError};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Durable, crash-resilient append-only JSONL job ledger.
/// Guarantees that every transition is flushed & fsynced to disk before returning.
#[derive(Debug, Clone)]
pub struct JobStore {
    ledger_path: PathBuf,
    lock: Arc<Mutex<()>>,
}

impl JobStore {
    pub fn new<P: AsRef<Path>>(ledger_path: P) -> Result<Self> {
        let path = ledger_path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(Self {
            ledger_path: path,
            lock: Arc::new(Mutex::new(())),
        })
    }

    /// Read all jobs by replaying the append-only ledger in order.
    /// Each subsequent record for the same UUID overwrites prior state.
    pub fn list_jobs(&self) -> Result<Vec<IngestJob>> {
        let _guard = self.lock.lock().unwrap();
        if !self.ledger_path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&self.ledger_path)?;
        let reader = BufReader::new(file);
        let mut job_map = std::collections::BTreeMap::<Uuid, IngestJob>::new();

        for line_res in reader.lines() {
            let line = line_res?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(job) = serde_json::from_str::<IngestJob>(trimmed) {
                job_map.insert(job.id, job);
            }
        }

        Ok(job_map.into_values().collect())
    }

    pub fn get_job(&self, id: Uuid) -> Result<Option<IngestJob>> {
        let jobs = self.list_jobs()?;
        Ok(jobs.into_iter().find(|j| j.id == id))
    }

    /// Persist a job record with fsync durability
    pub fn save_job(&self, job: &IngestJob) -> Result<()> {
        let _guard = self.lock.lock().unwrap();
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.ledger_path)?;

        let line = serde_json::to_string(job)?;
        writeln!(file, "{}", line)?;
        file.sync_all()?;
        Ok(())
    }

    /// Transition a job to a new state, enforcing transition validity, durability, and idempotency
    pub fn transition_job<F>(&self, id: Uuid, update_fn: F) -> Result<IngestJob>
    where
        F: FnOnce(&mut IngestJob),
    {
        let _guard = self.lock.lock().unwrap();

        // Replay all jobs to get latest state
        let mut jobs = if self.ledger_path.exists() {
            let file = File::open(&self.ledger_path)?;
            let reader = BufReader::new(file);
            let mut map = std::collections::BTreeMap::<Uuid, IngestJob>::new();
            for line_res in reader.lines() {
                let line = line_res?;
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if let Ok(j) = serde_json::from_str::<IngestJob>(trimmed) {
                    map.insert(j.id, j);
                }
            }
            map
        } else {
            std::collections::BTreeMap::new()
        };

        let mut current_job = jobs.remove(&id).ok_or(StorageError::JobNotFound(id))?;
        let old_state = current_job.state.clone();

        // Apply mutation
        update_fn(&mut current_job);
        current_job.updated_at = chrono::Utc::now();

        // Check state transition validity if state changed
        if old_state != current_job.state && !old_state.can_transition_to(&current_job.state) {
            return Err(StorageError::InvalidStateTransition {
                from: old_state,
                to: current_job.state,
            });
        }

        // Durable append & sync
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.ledger_path)?;
        let line = serde_json::to_string(&current_job)?;
        writeln!(file, "{}", line)?;
        file.sync_all()?;

        Ok(current_job)
    }

    /// Perform compact rewrite of ledger when needed
    pub fn compact(&self) -> Result<()> {
        let _guard = self.lock.lock().unwrap();
        if !self.ledger_path.exists() {
            return Ok(());
        }

        let jobs = {
            let file = File::open(&self.ledger_path)?;
            let reader = BufReader::new(file);
            let mut map = std::collections::BTreeMap::<Uuid, IngestJob>::new();
            for line_res in reader.lines() {
                let line = line_res?;
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if let Ok(j) = serde_json::from_str::<IngestJob>(trimmed) {
                    map.insert(j.id, j);
                }
            }
            map
        };

        let tmp_path = self.ledger_path.with_extension("tmp");
        {
            let mut tmp_file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&tmp_path)?;
            for job in jobs.values() {
                let line = serde_json::to_string(job)?;
                writeln!(tmp_file, "{}", line)?;
            }
            tmp_file.sync_all()?;
        }

        std::fs::rename(&tmp_path, &self.ledger_path)?;
        Ok(())
    }
}
