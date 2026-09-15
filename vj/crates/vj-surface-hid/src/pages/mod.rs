//! Stream Deck UI pages subsystem.
//!
//! Implements deterministic pages:
//! - BOOTING: Boot animation, storage, renderer, media readiness
//! - READY: Standby overview, show selection, quick controls
//! - PERFORMANCE: 4x8 grid, 32 clips across banks, active clip highlight, bank selection (0..7), blackout toggle, BPM/master control
//! - INGEST: USB/LAN ingest status, progress, warnings, accept/cancel, activation
//! - DEGRADED: Fault diagnostic, error codes, recovery actions
//! - SYSTEM: IP/hostname, free space, temp/throttle, renderer restart, guarded shutdown/reboot

pub mod booting;
pub mod degraded;
pub mod ingest;
pub mod performance;
pub mod ready;
pub mod system;

pub use booting::BootingPage;
pub use degraded::DegradedPage;
pub use ingest::IngestPage;
pub use performance::PerformancePage;
pub use ready::ReadyPage;
pub use system::SystemPage;

use std::time::Instant;
use crate::broker_client::{AppState, BrokerClient};
use crate::graphics::Canvas;
use crate::guarded::{GuardedAction, GuardedEngine};

pub const KEY_COUNT: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PageId {
    Booting,
    Ready,
    Performance,
    Ingest,
    Degraded,
    System,
}

impl PageId {
    pub fn title(&self) -> &'static str {
        match self {
            PageId::Booting => "BOOTING",
            PageId::Ready => "READY",
            PageId::Performance => "PERF",
            PageId::Ingest => "INGEST",
            PageId::Degraded => "DEGRADED",
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

/// Fault diagnostic info for DEGRADED page
#[derive(Clone, Debug)]
pub struct FaultInfo {
    pub code: String,
    pub message: String,
    pub subsystem: String,
    pub recovery_hint: String,
    pub renderer_down: bool,
}

impl Default for FaultInfo {
    fn default() -> Self {
        Self {
            code: "OK".to_string(),
            message: "All subsystems operational".to_string(),
            subsystem: "all".to_string(),
            recovery_hint: "None".to_string(),
            renderer_down: false,
        }
    }
}

/// System telemetry data for SYSTEM and general status
#[derive(Clone, Debug)]
pub struct SystemTelemetry {
    pub hostname: String,
    pub ip_address: String,
    pub free_space_bytes: u64,
    pub total_space_bytes: u64,
    pub temperature_c: f32,
    pub throttled: bool,
    pub uptime_secs: u64,
}

impl Default for SystemTelemetry {
    fn default() -> Self {
        Self {
            hostname: "rpi-vj.local".to_string(),
            ip_address: "192.168.1.100".to_string(),
            free_space_bytes: 14_500_000_000,
            total_space_bytes: 29_000_000_000,
            temperature_c: 46.5,
            throttled: false,
            uptime_secs: 120,
        }
    }
}

/// Context supplied to pages for rendering and input handling
pub struct PageContext {
    pub state: AppState,
    pub telemetry: SystemTelemetry,
    pub ingest: IngestStatus,
    pub fault: FaultInfo,
    pub boot_progress: f32,
    pub boot_storage_ok: bool,
    pub boot_renderer_ok: bool,
    pub boot_media_ok: bool,
    pub active_page: PageId,
    pub active_bank: u32,
    pub now: Instant,
}

impl Default for PageContext {
    fn default() -> Self {
        Self {
            state: AppState::default(),
            telemetry: SystemTelemetry::default(),
            ingest: IngestStatus::default(),
            fault: FaultInfo::default(),
            boot_progress: 1.0,
            boot_storage_ok: true,
            boot_renderer_ok: true,
            boot_media_ok: true,
            active_page: PageId::Ready,
            active_bank: 0,
            now: Instant::now(),
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
