//! CPU and I/O priority management and job execution controller.
//! Supports cooperative pausing / cancellation when entering PERFORMANCE mode.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Priority level for media analysis and transcode processes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriorityLevel {
    /// Ingest background tasks (low CPU/IO priority: nice 10-19, ionice idle/best-effort low)
    BackgroundIngest,
    /// Standard process priority
    Normal,
}

/// Apply low CPU priority (nice) and low I/O priority (ionice) to the current OS process/thread.
pub fn apply_low_priority() {
    #[cfg(unix)]
    unsafe {
        // Set nice level to +10 (lower scheduling priority)
        libc::nice(10);
    }
}

/// A cooperative cancellation/pause token that ingest jobs can observe.
#[derive(Debug, Clone)]
pub struct PauseToken {
    is_paused: Arc<AtomicBool>,
    is_cancelled: Arc<AtomicBool>,
}

impl Default for PauseToken {
    fn default() -> Self {
        Self::new()
    }
}

impl PauseToken {
    pub fn new() -> Self {
        Self {
            is_paused: Arc::new(AtomicBool::new(false)),
            is_cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Mark the token as paused (e.g. PERFORMANCE mode activated)
    pub fn pause(&self) {
        self.is_paused.store(true, Ordering::SeqCst);
    }

    /// Resume processing (e.g. PERFORMANCE mode deactivated)
    pub fn resume(&self) {
        self.is_paused.store(false, Ordering::SeqCst);
    }

    /// Check if processing is paused
    pub fn is_paused(&self) -> bool {
        self.is_paused.load(Ordering::SeqCst)
    }

    /// Cancel the job immediately
    pub fn cancel(&self) {
        self.is_cancelled.store(true, Ordering::SeqCst);
    }

    /// Check if the job was cancelled
    pub fn is_cancelled(&self) -> bool {
        self.is_cancelled.load(Ordering::SeqCst)
    }

    /// Wait or sleep while paused, returning Err if cancelled
    pub fn wait_if_paused(&self) -> Result<(), String> {
        while self.is_paused() {
            if self.is_cancelled() {
                return Err("Job was cancelled while paused".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        if self.is_cancelled() {
            return Err("Job was cancelled".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pause_token_lifecycle() {
        let token = PauseToken::new();
        assert!(!token.is_paused());
        assert!(!token.is_cancelled());

        token.pause();
        assert!(token.is_paused());

        token.resume();
        assert!(!token.is_paused());
        assert!(token.wait_if_paused().is_ok());

        token.cancel();
        assert!(token.is_cancelled());
        assert!(token.wait_if_paused().is_err());
    }
}
