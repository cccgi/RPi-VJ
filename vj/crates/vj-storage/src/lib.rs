use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Insufficient disk space: required {required_bytes} bytes, free {free_bytes} bytes, total {total_bytes} bytes (must maintain {reserve_percent}% free reserve)")]
    InsufficientSpace {
        required_bytes: u64,
        free_bytes: u64,
        total_bytes: u64,
        reserve_percent: f64,
    },

    #[error("Invalid state transition from {from:?} to {to:?}")]
    InvalidStateTransition {
        from: JobState,
        to: JobState,
    },

    #[error("Job not found: {0}")]
    JobNotFound(Uuid),

    #[error("Generation not found: {0}")]
    GenerationNotFound(String),

    #[error("No previous generation available for rollback")]
    NoPreviousGeneration,

    #[error("Invalid show manifest: {0}")]
    InvalidManifest(String),

    #[error("Storage corrupted: {0}")]
    Corrupted(String),

    #[error("USB mount failed for {device:?} at {mount_point:?}: {reason}")]
    UsbMountFailed {
        device: PathBuf,
        mount_point: PathBuf,
        reason: String,
    },

    #[error("USB unmount failed for {mount_point:?}: {reason}")]
    UsbUnmountFailed {
        mount_point: PathBuf,
        reason: String,
    },

    #[error("RPI-VJ-INBOX directory not found on removable media at {0:?}")]
    InboxNotFound(PathBuf),

    #[error("Mid-copy removal or I/O error during copy of {path:?}: {reason}")]
    MidCopyRemoval {
        path: PathBuf,
        reason: String,
    },

    #[error("Short read on {path:?}: expected {expected} bytes, got {actual} bytes")]
    ShortRead {
        path: PathBuf,
        expected: u64,
        actual: u64,
    },

    #[error("Duplicate media detected for {path:?} with SHA-256 hash {hash}")]
    DuplicateMedia {
        path: PathBuf,
        hash: String,
    },

    #[error("Unsupported media extension '{extension}' for file {path:?}")]
    UnsupportedExtension {
        path: PathBuf,
        extension: String,
    },

    #[error("Security violation: Malicious symlink detected at {path:?} pointing to {target:?}")]
    MaliciousSymlink {
        path: PathBuf,
        target: PathBuf,
    },
}

pub type Result<T> = std::result::Result<T, StorageError>;

/// Job state machine states as specified in A3-08:
/// discovered -> copying -> probing -> needs_decision -> transcoding -> validating -> ready -> activating -> complete
/// plus failed and cancelled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Discovered,
    Copying,
    Probing,
    NeedsDecision,
    Transcoding,
    Validating,
    Ready,
    Activating,
    Complete,
    Failed { reason: String },
    Cancelled,
}

impl JobState {
    pub fn is_terminal(&self) -> bool {
        matches!(self, JobState::Complete | JobState::Failed { .. } | JobState::Cancelled)
    }

    pub fn is_resumable(&self) -> bool {
        // Safe stages that can be resumed or re-entered on boot recovery
        matches!(
            self,
            JobState::Discovered
                | JobState::Copying
                | JobState::Probing
                | JobState::NeedsDecision
                | JobState::Transcoding
                | JobState::Validating
                | JobState::Ready
                | JobState::Activating
        )
    }

    /// Validate state machine transitions according to A3-08
    pub fn can_transition_to(&self, next: &JobState) -> bool {
        // Any non-terminal state can transition to Failed or Cancelled
        if !self.is_terminal() && matches!(next, JobState::Failed { .. } | JobState::Cancelled) {
            return true;
        }

        match (self, next) {
            (JobState::Discovered, JobState::Copying) => true,
            (JobState::Copying, JobState::Probing) => true,
            (JobState::Probing, JobState::NeedsDecision) => true,
            (JobState::Probing, JobState::Transcoding) => true,
            (JobState::Probing, JobState::Validating) => true, // direct validate if no transcode needed
            (JobState::NeedsDecision, JobState::Transcoding) => true,
            (JobState::NeedsDecision, JobState::Validating) => true,
            (JobState::Transcoding, JobState::Validating) => true,
            (JobState::Validating, JobState::Ready) => true,
            (JobState::Ready, JobState::Activating) => true,
            (JobState::Activating, JobState::Complete) => true,
            // Re-attempt / recovery transitions
            (JobState::Discovered, JobState::Discovered) => true,
            (JobState::Copying, JobState::Copying) => true,
            (JobState::Transcoding, JobState::Transcoding) => true,
            (JobState::Validating, JobState::Validating) => true,
            _ => false,
        }
    }
}

/// Metadata and tracking record for an ingest job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestJob {
    pub id: Uuid,
    pub source_path: PathBuf,
    pub state: JobState,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub staging_path: PathBuf,
    pub target_generation: Option<String>,
    pub estimated_bytes: u64,
    pub progress_percent: f32,
    pub details: HashMap<String, String>,
}

impl IngestJob {
    pub fn new(source_path: PathBuf, staging_dir: &Path, estimated_bytes: u64) -> Self {
        let id = Uuid::new_v4();
        let staging_path = staging_dir.join(format!("job-{}", id));
        let now = chrono::Utc::now();

        Self {
            id,
            source_path,
            state: JobState::Discovered,
            created_at: now,
            updated_at: now,
            staging_path,
            target_generation: None,
            estimated_bytes,
            progress_percent: 0.0,
            details: HashMap::new(),
        }
    }
}

pub mod disk;
pub mod job_store;
pub mod manager;
pub mod usb;

pub use disk::DiskSpaceChecker;
pub use job_store::JobStore;
pub use manager::{BootRecoveryReport, GenerationInfo, ShowStorageManager, StorageConfig};
pub use usb::{
    is_os_metadata, is_supported_video_ext, sanitize_file_name, CandidateFile, FileIngestOutcome,
    InboxScanner, MockMountProvider, MountProvider, ScanReport, SystemMountProvider, UsbCopyEngine,
    OS_METADATA_PATTERNS, SUPPORTED_VIDEO_EXTENSIONS,
};
