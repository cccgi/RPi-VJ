//! Admin API: versioned JSON over a unix socket (Pi) or a localhost
//! WebSocket (desktop).
//!
//! Low-rate only — configuration, show load/save, MIDI learn, diagnostics.
//! Per-frame telemetry does not go over JSON.

use serde::{Deserialize, Serialize};
use vj_core::{EngineCommand, EngineEvent};

pub mod remote;
pub use remote::*;

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Request {
    Hello { protocol: u32 },
    Command { command: EngineCommand },
    GetState,
    LoadShow { path: String },
    SaveShow { path: String },
    BeginLearn { region: String },
    EndLearn,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Response {
    Hello { protocol: u32, tier: String },
    Event { event: EngineEvent },
    Error { message: String },
    Ok,
}

/// Reject an incompatible major protocol version with a clear diagnostic
/// rather than half-speaking it.
pub fn check_protocol(theirs: u32) -> Result<(), String> {
    if theirs == PROTOCOL_VERSION {
        Ok(())
    } else {
        Err(format!("protocol {theirs} unsupported; this build speaks {PROTOCOL_VERSION}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_round_trip_as_tagged_json() {
        let r = Request::LoadShow { path: "shows/warehouse.toml".into() };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"kind\":\"load_show\""));
        assert_eq!(serde_json::from_str::<Request>(&json).unwrap(), r);
    }

    #[test]
    fn unknown_protocol_is_refused() {
        assert!(check_protocol(PROTOCOL_VERSION).is_ok());
        assert!(check_protocol(99).is_err());
    }
}
