//! USB HID transport and native Stream Deck XL controller for RPi-VJ Appliance V3.
//!
//! Operates the Stream Deck XL without Chromium, WebHID, browser profiles, or pairing.
//! Directly interfaces via Linux hidraw (using udev rules granting the `vj` group access)
//! or mock/simulation transports for automated testing.
//!
//! Architectural requirements implemented:
//! - Edge-triggered 32-key input with exactly one press and one release per physical action
//! - Sequential 0..31 logical indexing matching top-to-bottom, left-to-right physical layout
//! - Bounded input and output queues with key-level icon coalescing
//! - Decoupled I/O threads so stalled icon writes never block button presses
//! - Automatic reconnection with cached page replay restoring all 32 icons within 5 seconds
//! - Bounded raw-report diagnostics, off by default
//! - Controller state and lifecycle event publishing to broker state file and persistent boot ledger
//! - Deterministic pages: BOOTING, READY, PERFORMANCE, INGEST, DEGRADED, SYSTEM
//! - Guarded actions with 3-second long hold and 2-key confirmation
//! - Direct non-blocking broker IPC client (/run/rpi-vj/broker.sock) with automatic state replay

pub mod broker_client;
pub mod controller;
pub mod device;
pub mod graphics;
pub mod guarded;
pub mod pages;
pub mod publisher;
pub mod queue;
pub mod transport;
pub mod ui_engine;

pub use broker_client::{AppState, BrokerClient, BrokerCommand, BrokerTransport, MockBrokerTransport};
#[cfg(unix)]
pub use broker_client::UnixSocketTransport;
pub use controller::{ControllerConfig, StreamDeckController};
pub use device::{
    diff_buttons, frame_brightness_report, frame_key_image_report, frame_reset_report,
    key_to_row_col, row_col_to_key, IconCache, StreamDeckXL, ELGATO_VENDOR_ID,
    STREAM_DECK_XL_COLS, STREAM_DECK_XL_HEADER_SIZE, STREAM_DECK_XL_ICON_SIZE,
    STREAM_DECK_XL_KEY_COUNT, STREAM_DECK_XL_PAYLOAD_SIZE, STREAM_DECK_XL_PRODUCT_IDS,
    STREAM_DECK_XL_REPORT_SIZE, STREAM_DECK_XL_ROWS,
};
pub use graphics::{Canvas, Rgb, ICON_HEIGHT, ICON_PIXELS, ICON_WIDTH};
pub use guarded::{GuardedAction, GuardedEngine, GuardedEngineState};
pub use pages::{
    BootingPage, DegradedPage, FaultInfo, IngestPage, IngestStatus, Page, PageAction, PageContext,
    PageId, PerformancePage, ReadyPage, SystemPage, SystemTelemetry, KEY_COUNT,
};
pub use publisher::{BootLedgerEntry, ControllerState, StatePublisher};
pub use queue::{BoundedInputQueue, CoalescingOutputQueue};
pub use transport::{MockTransport, Transport};
pub use ui_engine::UIEngine;

use serde::{Deserialize, Serialize};

/// Stable identifier for a connected surface.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SurfaceId(pub String);

impl SurfaceId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SurfaceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::fmt::Debug for SurfaceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SurfaceId({})", self.0)
    }
}

impl From<&str> for SurfaceId {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

/// Control event emitted by the Stream Deck controller.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ControlEvent {
    Press {
        key: u8,
        logical_key: u8,
        at_ns: u64,
    },
    Release {
        key: u8,
        logical_key: u8,
        at_ns: u64,
    },
    Connected {
        surface: SurfaceId,
    },
    Disconnected {
        surface: SurfaceId,
    },
}

/// Command sent to the Stream Deck device.
#[derive(Clone, Debug, PartialEq)]
pub enum DeckCommand {
    SetIcon {
        key: u8,
        jpeg: Vec<u8>,
    },
    ClearKey {
        key: u8,
    },
    SetBrightness {
        pct: u8,
    },
    Reset,
    ReplayPage,
}

#[derive(Debug, thiserror::Error)]
pub enum StreamDeckError {
    #[error("device not connected")]
    NotConnected,
    #[error("transport I/O error: {0}")]
    Io(String),
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("timed out: {0}")]
    Timeout(String),
    #[error("queue saturated")]
    QueueFull,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_id_round_trips() {
        let id = SurfaceId::new("sdxl-primary");
        assert_eq!(id.as_str(), "sdxl-primary");
        assert_eq!(format!("{id}"), "sdxl-primary");
    }

    #[test]
    fn control_event_serialization() {
        let press = ControlEvent::Press {
            key: 7,
            logical_key: 7,
            at_ns: 123456789,
        };
        let json = serde_json::to_string(&press).unwrap();
        assert!(json.contains("\"event\":\"press\""));
        assert!(json.contains("\"key\":7"));
    }
}
