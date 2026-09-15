//! State publishing to broker and persistent boot ledger.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Controller state snapshot published to ephemeral broker storage (/run/rpi-vj/controller-state.json).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ControllerState {
    pub subsystem: String,
    pub connected: bool,
    pub surface_id: String,
    pub product: String,
    pub vid: String,
    pub pid: String,
    pub active_page: u32,
    pub cached_icons: usize,
    pub pending_writes: usize,
    pub total_presses: u64,
    pub total_releases: u64,
    pub reconnect_count: u64,
    pub last_event: Option<String>,
    pub updated_at_ns: u64,
}

/// An entry appended to the persistent boot ledger (/var/lib/rpi-vj/boot-ledger.jsonl).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BootLedgerEntry {
    pub timestamp_ns: u64,
    pub subsystem: String,
    pub event: String,
    pub details: serde_json::Value,
}

pub struct StatePublisher {
    pub broker_state_path: PathBuf,
    pub boot_ledger_path: PathBuf,
}

impl StatePublisher {
    pub fn new(broker_state_path: impl Into<PathBuf>, boot_ledger_path: impl Into<PathBuf>) -> Self {
        Self {
            broker_state_path: broker_state_path.into(),
            boot_ledger_path: boot_ledger_path.into(),
        }
    }

    pub fn default_paths() -> Self {
        Self::new(
            "/run/rpi-vj/controller-state.json",
            "/var/lib/rpi-vj/boot-ledger.jsonl",
        )
    }

    /// Atomically write controller state to broker health file.
    pub fn publish_state(&self, state: &ControllerState) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(state)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        if let Some(parent) = self.broker_state_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let tmp_path = self.broker_state_path.with_extension("tmp");
        {
            let mut file = File::create(&tmp_path)?;
            file.write_all(json.as_bytes())?;
            file.sync_all()?;
        }
        std::fs::rename(&tmp_path, &self.broker_state_path)?;
        Ok(())
    }

    /// Append an immutable event record to the persistent boot ledger with fsync.
    pub fn record_boot_ledger(&self, event: &str, details: serde_json::Value) -> std::io::Result<()> {
        let now_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let entry = BootLedgerEntry {
            timestamp_ns: now_ns,
            subsystem: "controller".into(),
            event: event.into(),
            details,
        };

        let line = serde_json::to_string(&entry)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        if let Some(parent) = self.boot_ledger_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.boot_ledger_path)?;

        writeln!(file, "{}", line)?;
        file.sync_data()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_publish_and_boot_ledger() {
        let dir = std::env::temp_dir().join(format!("vj-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);

        let broker_path = dir.join("controller-state.json");
        let ledger_path = dir.join("boot-ledger.jsonl");

        let publisher = StatePublisher::new(&broker_path, &ledger_path);

        let state = ControllerState {
            subsystem: "controller".into(),
            connected: true,
            surface_id: "sdxl-1".into(),
            product: "Stream Deck XL".into(),
            vid: "0x0fd9".into(),
            pid: "0x006c".into(),
            active_page: 1,
            cached_icons: 32,
            pending_writes: 0,
            total_presses: 10,
            total_releases: 10,
            reconnect_count: 1,
            last_event: Some("press:7".into()),
            updated_at_ns: 1000,
        };

        publisher.publish_state(&state).expect("publish state");
        assert!(broker_path.exists());

        let read_back: ControllerState =
            serde_json::from_str(&std::fs::read_to_string(&broker_path).unwrap()).unwrap();
        assert_eq!(read_back, state);

        publisher
            .record_boot_ledger(
                "connected",
                serde_json::json!({ "device": "Stream Deck XL", "vid": "0x0fd9", "pid": "0x006c" }),
            )
            .expect("record ledger");

        assert!(ledger_path.exists());
        let ledger_contents = std::fs::read_to_string(&ledger_path).unwrap();
        assert!(ledger_contents.contains("\"event\":\"connected\""));
        assert!(ledger_contents.contains("\"subsystem\":\"controller\""));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
