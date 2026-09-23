//! Stream Deck UI pages subsystem.
//!
//! Implements 3 deterministic pages (consolidated from an earlier 6-page
//! design after an audit found BOOTING unreachable, READY's bank overview
//! redundant with PERFORMANCE's own bank-select modal, DEGRADED's real
//! actions overlapping SYSTEM's, and several telemetry fields permanently
//! fake — see HANDOFF.md for the full record):
//! - PERFORMANCE: boot page. 24 clip pads + bank select, stop, blackout,
//!   BPM, and master opacity control. Single-tap, no navigation needed.
//! - SYSTEM: real telemetry only (hostname, CPU temp), renderer restart,
//!   state restore, guarded reboot/shutdown, ingest entry.
//! - INGEST: USB/LAN ingest status, progress, warnings, accept/cancel, activation

pub mod ingest;
pub mod performance;
pub mod show_manifest;
pub mod system;

pub use ingest::IngestPage;
pub use performance::PerformancePage;
pub use show_manifest::{PadInfo, ShowManifest};
pub use system::SystemPage;

use std::sync::Arc;
use std::time::Instant;
use crate::broker_client::{AppState, BrokerClient};
use crate::graphics::Canvas;
use crate::guarded::{GuardedAction, GuardedEngine};

pub const KEY_COUNT: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PageId {
    Performance,
    Ingest,
    System,
}

impl PageId {
    pub fn title(&self) -> &'static str {
        match self {
            PageId::Performance => "PERF",
            PageId::Ingest => "INGEST",
            PageId::System => "SYSTEM",
        }
    }
}

/// USB Drive status
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsbDriveStatus {
    NoMedia,
    Plugged,
    Scanning,
    Ready,
    Ejected,
}

impl UsbDriveStatus {
    pub fn label(&self) -> &'static str {
        match self {
            UsbDriveStatus::NoMedia => "NO MEDIA",
            UsbDriveStatus::Plugged => "PLUGGED",
            UsbDriveStatus::Scanning => "SCANNING",
            UsbDriveStatus::Ready => "READY",
            UsbDriveStatus::Ejected => "EJECTED",
        }
    }
}

/// Media analysis recommendation from probe/analysis stage
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaRecommendation {
    None,
    Ok,
    QuadMirrorDetected,
    SeamGapDetected,
    QuadAndSeamDetected,
    CorruptOrUnsupported,
}

impl MediaRecommendation {
    pub fn label(&self) -> &'static str {
        match self {
            MediaRecommendation::None => "NONE",
            MediaRecommendation::Ok => "REC: OK",
            MediaRecommendation::QuadMirrorDetected => "QUAD_MIRROR",
            MediaRecommendation::SeamGapDetected => "SEAM_GAP",
            MediaRecommendation::QuadAndSeamDetected => "QUAD+SEAM",
            MediaRecommendation::CorruptOrUnsupported => "BAD FORMAT",
        }
    }
}

/// Ingest subsystem status
#[derive(Clone, Debug)]
pub struct IngestStatus {
    pub usb_inserted: bool,
    pub usb_status: UsbDriveStatus,
    pub file_count: usize,
    pub current_file_index: usize,
    pub job_name: Option<String>,
    pub current_file_name: Option<String>,
    pub step: String,
    pub progress: f32,
    pub warning: Option<String>,
    pub error_code: Option<String>,
    pub recommendation: MediaRecommendation,
    pub proposed_crop: bool,
    pub proposed_seam: bool,
    pub completed_generation: Option<String>,
    pub transcode_paused_for_perf: bool,
}

impl Default for IngestStatus {
    fn default() -> Self {
        Self {
            usb_inserted: false,
            usb_status: UsbDriveStatus::NoMedia,
            file_count: 0,
            current_file_index: 0,
            job_name: None,
            current_file_name: None,
            step: "Idle".to_string(),
            progress: 0.0,
            warning: None,
            error_code: None,
            recommendation: MediaRecommendation::None,
            proposed_crop: false,
            proposed_seam: false,
            completed_generation: None,
            transcode_paused_for_perf: false,
        }
    }
}

/// System telemetry data for SYSTEM display. Only fields actually kept
/// current by `UIEngine::refresh_telemetry()` belong here — a prior
/// version also carried ip_address/free_space_bytes/total_space_bytes/
/// throttled/uptime_secs, none of which were ever updated after their
/// `Default`, so they permanently displayed fake values. Removed rather
/// than left to silently lie on stage.
#[derive(Clone, Debug)]
pub struct SystemTelemetry {
    pub hostname: String,
    pub temperature_c: f32,
}

impl Default for SystemTelemetry {
    fn default() -> Self {
        Self {
            hostname: "rpi-vj.local".to_string(),
            temperature_c: 46.5,
        }
    }
}

/// Context supplied to pages for rendering and input handling
pub struct PageContext {
    pub state: AppState,
    pub telemetry: SystemTelemetry,
    pub ingest: IngestStatus,
    pub active_page: PageId,
    pub active_bank: u32,
    pub now: Instant,
    /// Real pad names/thumbnails from the deployed show.json, if it loaded
    /// successfully at startup. `None` means "couldn't load it" (missing
    /// file, bad JSON, etc.) -- pages fall back to numeric labels, not an
    /// error condition.
    pub show_manifest: Option<Arc<ShowManifest>>,
}

impl Default for PageContext {
    fn default() -> Self {
        Self {
            state: AppState::default(),
            telemetry: SystemTelemetry::default(),
            ingest: IngestStatus::default(),
            active_page: PageId::Performance,
            active_bank: 0,
            now: Instant::now(),
            show_manifest: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PageAction {
    SwitchPage(PageId),
    TriggerGuarded(GuardedAction),
    None,
}

/// Trait implemented by all Stream Deck UI pages
pub trait Page: Send + Sync {
    fn page_id(&self) -> PageId;
    fn render_key(&self, key: u8, ctx: &PageContext, guarded: &GuardedEngine) -> Canvas;
    fn handle_key_press(&mut self, key: u8, ctx: &mut PageContext, broker: &BrokerClient, guarded: &mut GuardedEngine) -> PageAction;
    fn handle_key_release(&mut self, key: u8, ctx: &mut PageContext, broker: &BrokerClient, guarded: &mut GuardedEngine) -> PageAction;
}
