//! rpi-vj-controller: Native Stream Deck XL service for RPi-VJ Appliance V3.
//!
//! Replaces Chromium/WebHID with native Linux hidraw access.
//! Emits edge-triggered button events, handles asynchronous JPEG icon writes,
//! caches current show page, recovers across hotplugs, drives deterministic
//! state pages (PERFORMANCE, INGEST, SYSTEM), connects to broker IPC
//! socket, and handles state replay across renderer restarts.

use std::env;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use vj_surface_hid::{
    broker_client::{BrokerClient, BrokerTransport, MockBrokerTransport},
    controller::{ControllerConfig, StreamDeckController},
    pages::show_manifest::{default_show_json_path, default_web_root, ShowManifest},
    transport::{HidrawTransport, MockTransport, Transport},
    ui_engine::UIEngine,
    ControlEvent, PageId, SurfaceId,
};

#[cfg(unix)]
use vj_surface_hid::broker_client::UnixSocketTransport;

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

    let socket_path = env::var("RPI_VJ_BROKER_SOCKET")
        .unwrap_or_else(|_| "/run/rpi-vj/broker.sock".to_string());

    eprintln!("[rpi-vj-controller] Starting Native Stream Deck XL Controller");
    eprintln!("[rpi-vj-controller] Mode: {}", if is_mock { "MOCK" } else { "HIDRAW" });
    eprintln!("[rpi-vj-controller] Diagnostics: {}", if config.raw_diagnostics { "ENABLED" } else { "OFF" });
    eprintln!("[rpi-vj-controller] Broker Socket: {}", socket_path);
    eprintln!("[rpi-vj-controller] State File: {:?}", config.broker_state_path);
    eprintln!("[rpi-vj-controller] Boot Ledger: {:?}", config.boot_ledger_path);

    // 1. Initialize Stream Deck Transport
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

    let controller = Arc::new(StreamDeckController::new(config.clone(), transport));

    // 2. Initialize Broker IPC Client
    let broker_transport: Arc<dyn BrokerTransport> = {
        #[cfg(unix)]
        {
            if is_mock {
                Arc::new(MockBrokerTransport::new())
            } else {
                Arc::new(UnixSocketTransport::new(&socket_path))
            }
        }
        #[cfg(not(unix))]
        {
            Arc::new(MockBrokerTransport::new())
        }
    };

    let broker = Arc::new(BrokerClient::new(broker_transport));

    // 3. Initialize UI Engine with deterministic pages
    // Boots straight into PERFORMANCE (skipping READY) so clip triggering
    // needs zero navigation, matching the pre-V3 flat-32-key layout's
    // instant-play feel. READY (bank overview, health, ingest, system) is
    // still one tap away via key 31 ("EXIT / DISARM / READY >").
    let mut ui = UIEngine::new(controller.clone(), broker.clone());

    // Real pad names/thumbnails from the deployed show.json, best-effort.
    // Missing/malformed file or a slow/corrupt thumbnail never blocks
    // startup -- ShowManifest::load() is a soft-fail path by design.
    let manifest_load_start = Instant::now();
    match ShowManifest::load(&default_show_json_path(), &default_web_root()) {
        Some(manifest) => {
            eprintln!(
                "[rpi-vj-controller] Loaded show.json pad names/thumbnails in {:?}",
                manifest_load_start.elapsed()
            );
            ui.context_mut().show_manifest = Some(Arc::new(manifest));
        }
        None => {
            eprintln!("[rpi-vj-controller] No show.json pad data available; using numeric labels");
        }
    }

    ui.set_page(PageId::Performance);

    let term_signal = Arc::new(AtomicBool::new(false));
    let term_clone = term_signal.clone();
    ctrlc_handler(term_clone);

    eprintln!("[rpi-vj-controller] Controller & UI Engine running. Listening for events...");

    let mut last_telemetry_refresh = Instant::now();
    let mut last_tick = Instant::now();

    while !term_signal.load(Ordering::SeqCst) {
        // Poll edge-triggered HID button events
        while let Some(event) = controller.poll_event() {
            match event {
                ControlEvent::Press { key, logical_key, at_ns } => {
                    if config.raw_diagnostics {
                        eprintln!("[STREAMDECK KEY PRESS] key={} logical={} at_ns={}", key, logical_key, at_ns);
                    }
                    ui.on_key_press(key);
                }
                ControlEvent::Release { key, logical_key, at_ns } => {
                    if config.raw_diagnostics {
                        eprintln!("[STREAMDECK KEY RELEASE] key={} logical={} at_ns={}", key, logical_key, at_ns);
                    }
                    ui.on_key_release(key);
                }
                ControlEvent::Connected { surface } => {
                    eprintln!("[STREAMDECK CONNECTED] surface={}", surface);
                    // Replay active page icons upon reconnect. Timed
                    // (always, not just under --diag) since this is exactly
                    // the number needed to judge whether clip thumbnails
                    // are worth their render cost -- a full 32-key page
                    // render is the single most expensive thing this
                    // process does.
                    let render_start = Instant::now();
                    ui.render_full_page();
                    eprintln!(
                        "[rpi-vj-controller] Full page render (32 keys): {:?}",
                        render_start.elapsed()
                    );
                    // Replay active bank and clip state to broker
                    broker.replay_state();
                }
                ControlEvent::Disconnected { surface } => {
                    eprintln!("[STREAMDECK DISCONNECTED] surface={}", surface);
                }
            }
        }

        // Periodic UI Tick (holds, animations, state updates)
        if last_tick.elapsed() >= Duration::from_millis(50) {
            last_tick = Instant::now();
            ui.tick();
        }

        // Telemetry refresh every 2 seconds
        if last_telemetry_refresh.elapsed() >= Duration::from_secs(2) {
            last_telemetry_refresh = Instant::now();
            ui.refresh_telemetry();
        }

        thread::sleep(Duration::from_millis(5));
    }

    eprintln!("[rpi-vj-controller] Shutting down controller cleanly...");
    broker.stop();
    controller.stop();
}

fn ctrlc_handler(flag: Arc<AtomicBool>) {
    let _ = flag;
}
