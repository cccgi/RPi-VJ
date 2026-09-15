//! rpi-vj-controller: Native Stream Deck XL service for RPi-VJ Appliance V3.
//!
//! Replaces Chromium/WebHID with native Linux hidraw access.
//! Emits edge-triggered button events, handles asynchronous JPEG icon writes,
//! caches current show page, recovers across hotplugs, and publishes state to
//! broker and persistent boot ledger.

use std::env;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use vj_surface_hid::{
    controller::{ControllerConfig, StreamDeckController},
    transport::{HidrawTransport, MockTransport, Transport},
    ControlEvent, SurfaceId,
};

fn main() {
    let args: Vec<String> = env::args().collect();
    let is_mock = args.iter().any(|a| a == "--mock" || a == "-m");
    let is_diag = args.iter().any(|a| a == "--diag" || a == "-d");

    let mut config = ControllerConfig {
        surface_id: SurfaceId::new("streamdeck-xl-appliance"),
        raw_diagnostics: is_diag,
        ..Default::default()
    };

    if let Ok(state_path) = env::var("RPI_VJ_BROKER_STATE_PATH") {
        config.broker_state_path = PathBuf::from(state_path);
    }
    if let Ok(ledger_path) = env::var("RPI_VJ_BOOT_LEDGER_PATH") {
        config.boot_ledger_path = PathBuf::from(ledger_path);
    }

    eprintln!("[rpi-vj-controller] Starting Native Stream Deck XL Controller");
    eprintln!("[rpi-vj-controller] Mode: {}", if is_mock { "MOCK" } else { "HIDRAW" });
    eprintln!("[rpi-vj-controller] Diagnostics: {}", if config.raw_diagnostics { "ENABLED" } else { "OFF" });
    eprintln!("[rpi-vj-controller] State File: {:?}", config.broker_state_path);
    eprintln!("[rpi-vj-controller] Boot Ledger: {:?}", config.boot_ledger_path);

    let transport: Arc<dyn Transport> = if is_mock {
        Arc::new(MockTransport::new())
    } else {
        match HidrawTransport::open_default() {
            Ok(t) => Arc::new(t),
            Err(e) => {
                eprintln!("[rpi-vj-controller] Device not currently attached (waiting for udev hotplug): {}", e);
                Arc::new(HidrawTransport::new())
            }
        }
    };

    let controller = Arc::new(StreamDeckController::new(config, transport));

    let term_signal = Arc::new(AtomicBool::new(false));
    let term_clone = term_signal.clone();
    ctrlc_handler(term_clone);

    eprintln!("[rpi-vj-controller] Controller running. Listening for events...");

    while !term_signal.load(Ordering::SeqCst) {
        if let Some(event) = controller.poll_event() {
            match event {
                ControlEvent::Press { key, logical_key, at_ns } => {
                    eprintln!("[STREAMDECK KEY PRESS] key={} logical={} at_ns={}", key, logical_key, at_ns);
                }
                ControlEvent::Release { key, logical_key, at_ns } => {
                    eprintln!("[STREAMDECK KEY RELEASE] key={} logical={} at_ns={}", key, logical_key, at_ns);
                }
                ControlEvent::Connected { surface } => {
                    eprintln!("[STREAMDECK CONNECTED] surface={}", surface);
                }
                ControlEvent::Disconnected { surface } => {
                    eprintln!("[STREAMDECK DISCONNECTED] surface={}", surface);
                }
            }
        } else {
            thread::sleep(Duration::from_millis(5));
        }
    }

    eprintln!("[rpi-vj-controller] Shutting down controller cleanly...");
    controller.stop();
}

fn ctrlc_handler(flag: Arc<AtomicBool>) {
    // Simple fallback loop or ctrl-c handler if available
    let _ = flag;
}
