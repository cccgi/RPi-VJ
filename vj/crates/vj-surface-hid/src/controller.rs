//! Native Stream Deck XL controller service engine.
//!
//! Orchestrates edge-triggered input decoding, decoupled non-blocking icon writes,
//! automatic reconnect with cached page replay, bounded diagnostics, and state publishing.

use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::{
    device::{
        frame_brightness_report, frame_key_image_report, frame_reset_report,
        STREAM_DECK_XL_KEY_COUNT,
    },
    publisher::{ControllerState, StatePublisher},
    queue::{BoundedInputQueue, CoalescingOutputQueue},
    transport::Transport,
    ControlEvent, StreamDeckError, StreamDeckXL, SurfaceId,
};

#[derive(Clone, Debug)]
pub struct ControllerConfig {
    pub surface_id: SurfaceId,
    pub raw_diagnostics: bool,
    pub max_raw_dumps: usize,
    pub input_queue_capacity: usize,
    pub write_timeout: Duration,
    pub read_timeout: Duration,
    pub reconnect_poll_interval: Duration,
    pub broker_state_path: PathBuf,
    pub boot_ledger_path: PathBuf,
    pub default_brightness: u8,
}

impl Default for ControllerConfig {
    fn default() -> Self {
        Self {
            surface_id: SurfaceId::new("streamdeck-xl-primary"),
            raw_diagnostics: false,
            max_raw_dumps: 16,
            input_queue_capacity: 256,
            write_timeout: Duration::from_millis(500),
            read_timeout: Duration::from_millis(10),
            reconnect_poll_interval: Duration::from_millis(50),
            broker_state_path: PathBuf::from("/run/rpi-vj/controller-state.json"),
            boot_ledger_path: PathBuf::from("/var/lib/rpi-vj/boot-ledger.jsonl"),
            default_brightness: 100,
        }
    }
}

pub struct StreamDeckController {
    config: ControllerConfig,
    device: Arc<Mutex<StreamDeckXL>>,
    transport: Arc<dyn Transport>,
    input_queue: Arc<BoundedInputQueue>,
    output_queue: Arc<CoalescingOutputQueue>,
    publisher: Arc<StatePublisher>,
    raw_diagnostics: Arc<AtomicBool>,
    raw_dumps_remaining: Arc<AtomicUsize>,
    running: Arc<AtomicBool>,
    is_connected: Arc<AtomicBool>,

    // Observability stats
    total_presses: Arc<AtomicU64>,
    total_releases: Arc<AtomicU64>,
    reconnect_count: Arc<AtomicU64>,
    active_page: Arc<AtomicU32>,

    _input_thread: Option<JoinHandle<()>>,
    _output_thread: Option<JoinHandle<()>>,
}

impl StreamDeckController {
    pub fn new(config: ControllerConfig, transport: Arc<dyn Transport>) -> Self {
        let surface_id = config.surface_id.clone();
        let device = Arc::new(Mutex::new(StreamDeckXL::new(surface_id)));
        let input_queue = Arc::new(BoundedInputQueue::new(config.input_queue_capacity));
        let output_queue = Arc::new(CoalescingOutputQueue::new());
        let publisher = Arc::new(StatePublisher::new(
            &config.broker_state_path,
            &config.boot_ledger_path,
        ));
        let raw_diagnostics = Arc::new(AtomicBool::new(config.raw_diagnostics));
        let raw_dumps_remaining = Arc::new(AtomicUsize::new(config.max_raw_dumps));
        let running = Arc::new(AtomicBool::new(true));
        let is_connected = Arc::new(AtomicBool::new(false));

        let total_presses = Arc::new(AtomicU64::new(0));
        let total_releases = Arc::new(AtomicU64::new(0));
        let reconnect_count = Arc::new(AtomicU64::new(0));
        let active_page = Arc::new(AtomicU32::new(1));

        let mut controller = Self {
            config,
            device,
            transport,
            input_queue,
            output_queue,
            publisher,
            raw_diagnostics,
            raw_dumps_remaining,
            running,
            is_connected,
            total_presses,
            total_releases,
            reconnect_count,
            active_page,
            _input_thread: None,
            _output_thread: None,
        };

        controller.start_threads();
        controller
    }

    fn now_ns() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
    }

    fn start_threads(&mut self) {
        let running = self.running.clone();
        let is_connected = self.is_connected.clone();
        let device = self.device.clone();
        let transport = self.transport.clone();
        let input_queue = self.input_queue.clone();
        let output_queue = self.output_queue.clone();
        let publisher = self.publisher.clone();
        let raw_diag = self.raw_diagnostics.clone();
        let raw_dumps = self.raw_dumps_remaining.clone();
        let total_presses = self.total_presses.clone();
        let total_releases = self.total_releases.clone();
        let reconnect_count = self.reconnect_count.clone();
        let active_page = self.active_page.clone();
        let surface_id = self.config.surface_id.clone();
        let read_timeout = self.config.read_timeout;
        let reconnect_interval = self.config.reconnect_poll_interval;
        let default_brightness = self.config.default_brightness;

        // --- 1. Input loop thread ---
        let input_thread = thread::Builder::new()
            .name("sdxl-input".into())
            .spawn(move || {
                let mut buf = vec![0u8; 1024];
                let mut was_connected = false;

                while running.load(Ordering::SeqCst) {
                    let connected_now = transport.is_connected();

                    if !connected_now {
                        if was_connected {
                            was_connected = false;
                            is_connected.store(false, Ordering::SeqCst);

                            // Disconnect edge: release any held buttons
                            let release_edges = {
                                let mut dev = device.lock().unwrap();
                                dev.reset_button_state()
                            };
                            let now = Self::now_ns();
                            for (key, _) in release_edges {
                                total_releases.fetch_add(1, Ordering::Relaxed);
                                input_queue.push(ControlEvent::Release {
                                    key: key as u8,
                                    logical_key: key as u8,
                                    at_ns: now,
                                });
                            }

                            input_queue.push(ControlEvent::Disconnected {
                                surface: surface_id.clone(),
                            });

                            let _ = publisher.record_boot_ledger(
                                "disconnected",
                                serde_json::json!({
                                    "surface": surface_id.as_str(),
                                    "action": "streamdeck_unplugged"
                                }),
                            );

                            Self::update_broker_state(
                                &publisher,
                                &surface_id,
                                false,
                                active_page.load(Ordering::Relaxed),
                                0,
                                output_queue.len(),
                                total_presses.load(Ordering::Relaxed),
                                total_releases.load(Ordering::Relaxed),
                                reconnect_count.load(Ordering::Relaxed),
                                Some("disconnected".into()),
                            );
                        }

                        thread::sleep(reconnect_interval);
                        continue;
                    }

                    // Connected transition
                    if !was_connected {
                        was_connected = true;
                        is_connected.store(true, Ordering::SeqCst);
                        reconnect_count.fetch_add(1, Ordering::SeqCst);

                        // Initialize hardware: reset and brightness
                        let reset_cmd = frame_reset_report();
                        let _ = transport.send_feature(&reset_cmd, Duration::from_millis(200));
                        let bright_cmd = frame_brightness_report(default_brightness);
                        let _ = transport.send_feature(&bright_cmd, Duration::from_millis(200));

                        input_queue.push(ControlEvent::Connected {
                            surface: surface_id.clone(),
                        });

                        let _ = publisher.record_boot_ledger(
                            "connected",
                            serde_json::json!({
                                "surface": surface_id.as_str(),
                                "action": "streamdeck_connected",
                                "reconnect_count": reconnect_count.load(Ordering::Relaxed)
                            }),
                        );

                        // Replay cached page upon reconnect: mark all cached populated icons dirty
                        let dirty_keys = {
                            let mut dev = device.lock().unwrap();
                            dev.icon_cache_mut().mark_all_dirty()
                        };

                        for key in dirty_keys {
                            let dev = device.lock().unwrap();
                            if let Some(jpeg) = dev.icon_cache().get_icon(key) {
                                output_queue.push(key as u8, jpeg.to_vec());
                            }
                        }

                        let cached_count = {
                            let dev = device.lock().unwrap();
                            dev.icon_cache().populated_count()
                        };

                        Self::update_broker_state(
                            &publisher,
                            &surface_id,
                            true,
                            active_page.load(Ordering::Relaxed),
                            cached_count,
                            output_queue.len(),
                            total_presses.load(Ordering::Relaxed),
                            total_releases.load(Ordering::Relaxed),
                            reconnect_count.load(Ordering::Relaxed),
                            Some("reconnected".into()),
                        );
                    }

                    // Read input reports from device without holding any mutex!
                    let read_res = transport.read(&mut buf, read_timeout);

                    match read_res {
                        Ok(0) => {
                            // Timeout / no data ready; loop
                        }
                        Ok(n) => {
                            // Bounded diagnostic report dump
                            if raw_diag.load(Ordering::Relaxed) {
                                let dumps = raw_dumps.load(Ordering::Relaxed);
                                if dumps > 0 {
                                    raw_dumps.fetch_sub(1, Ordering::Relaxed);
                                    let hot: Vec<String> = buf[..n]
                                        .iter()
                                        .enumerate()
                                        .filter(|(_, &v)| v != 0)
                                        .map(|(i, &v)| format!("{i}:{v}"))
                                        .collect();
                                    eprintln!(
                                        "[STREAMDECK RAW DIAG] len={} nonzeros=[{}]",
                                        n,
                                        hot.join(" ")
                                    );
                                }
                            }

                            // Parse report and emit edge transitions
                            let transitions = {
                                let mut dev = device.lock().unwrap();
                                dev.parse_input_report(&buf[..n])
                            };

                            if let Some(edges) = transitions {
                                let now = Self::now_ns();
                                for (key, pressed) in edges {
                                    let logical_key = key as u8;
                                    if pressed {
                                        total_presses.fetch_add(1, Ordering::Relaxed);
                                        input_queue.push(ControlEvent::Press {
                                            key: key as u8,
                                            logical_key,
                                            at_ns: now,
                                        });
                                    } else {
                                        total_releases.fetch_add(1, Ordering::Relaxed);
                                        input_queue.push(ControlEvent::Release {
                                            key: key as u8,
                                            logical_key,
                                            at_ns: now,
                                        });
                                    }
                                }
                            }
                        }
                        Err(StreamDeckError::NotConnected) => {
                            if was_connected {
                                was_connected = false;
                                is_connected.store(false, Ordering::SeqCst);
                                let release_edges = {
                                    let mut dev = device.lock().unwrap();
                                    dev.reset_button_state()
                                };
                                let now = Self::now_ns();
                                for (key, _) in release_edges {
                                    total_releases.fetch_add(1, Ordering::Relaxed);
                                    input_queue.push(ControlEvent::Release {
                                        key: key as u8,
                                        logical_key: key as u8,
                                        at_ns: now,
                                    });
                                }
                                input_queue.push(ControlEvent::Disconnected {
                                    surface: surface_id.clone(),
                                });
                            }
                        }
                        Err(_) => {
                            // Transient read error; continue
                        }
                    }
                }
            })
            .expect("spawn input thread");

        // --- 2. Output loop thread (decoupled for non-blocking icon writes) ---
        let running_out = self.running.clone();
        let is_connected_out = self.is_connected.clone();
        let device_out = self.device.clone();
        let transport_out = self.transport.clone();
        let output_queue_out = self.output_queue.clone();
        let write_timeout = self.config.write_timeout;

        let output_thread = thread::Builder::new()
            .name("sdxl-output".into())
            .spawn(move || {
                while running_out.load(Ordering::SeqCst) {
                    if !is_connected_out.load(Ordering::SeqCst) {
                        thread::sleep(Duration::from_millis(20));
                        continue;
                    }

                    if let Some((key, jpeg_data)) = output_queue_out.pop() {
                        // Frame into 1024-byte packets
                        let packets = frame_key_image_report(key, &jpeg_data);
                        let mut success = true;

                        for packet in packets {
                            if !running_out.load(Ordering::SeqCst)
                                || !is_connected_out.load(Ordering::SeqCst)
                            {
                                success = false;
                                break;
                            }
                            // Write packet with timeout
                            let write_res = transport_out.write(&packet, write_timeout);

                            if let Err(err) = write_res {
                                eprintln!(
                                    "[STREAMDECK] icon write error on key {}: {}",
                                    key, err
                                );
                                success = false;
                                break;
                            }
                        }

                        // Update device icon cache on successful upload
                        if success {
                            let mut dev = device_out.lock().unwrap();
                            dev.set_key_icon(key as usize, Some(jpeg_data));
                        }
                    } else {
                        thread::sleep(Duration::from_millis(5));
                    }
                }
            })
            .expect("spawn output thread");

        self._input_thread = Some(input_thread);
        self._output_thread = Some(output_thread);
    }

    fn update_broker_state(
        publisher: &StatePublisher,
        surface_id: &SurfaceId,
        connected: bool,
        page: u32,
        cached_icons: usize,
        pending_writes: usize,
        presses: u64,
        releases: u64,
        reconnects: u64,
        last_event: Option<String>,
    ) {
        let state = ControllerState {
            subsystem: "controller".into(),
            connected,
            surface_id: surface_id.to_string(),
            product: "Stream Deck XL".into(),
            vid: "0x0fd9".into(),
            pid: "0x006c".into(),
            active_page: page,
            cached_icons,
            pending_writes,
            total_presses: presses,
            total_releases: releases,
            reconnect_count: reconnects,
            last_event,
            updated_at_ns: Self::now_ns(),
        };
        let _ = publisher.publish_state(&state);
    }

    /// Update an icon for key (0..31).
    pub fn set_icon(&self, key: u8, jpeg_data: Vec<u8>) {
        if (key as usize) >= STREAM_DECK_XL_KEY_COUNT {
            return;
        }
        self.output_queue.push(key, jpeg_data);
    }

    /// Load or restore an entire 32-key page.
    pub fn set_page(&self, page_number: u32, icons: &[Option<Vec<u8>>; STREAM_DECK_XL_KEY_COUNT]) {
        self.active_page.store(page_number, Ordering::Relaxed);
        for (i, icon) in icons.iter().enumerate() {
            if let Some(jpeg) = icon {
                self.set_icon(i as u8, jpeg.clone());
            }
        }
    }

    /// Replay all currently cached page icons.
    pub fn replay_cached_page(&self) {
        let dev = self.device.lock().unwrap();
        for i in 0..STREAM_DECK_XL_KEY_COUNT {
            if let Some(jpeg) = dev.icon_cache().get_icon(i) {
                self.output_queue.push(i as u8, jpeg.to_vec());
            }
        }
    }

    /// Set panel brightness percentage (0..100%).
    pub fn set_brightness(&self, pct: u8) -> Result<(), StreamDeckError> {
        let report = frame_brightness_report(pct);
        self.transport.send_feature(&report, Duration::from_millis(200))
    }

    /// Reset panel state and clear all keys.
    pub fn reset(&self) -> Result<(), StreamDeckError> {
        self.output_queue.clear();
        {
            let mut dev = self.device.lock().unwrap();
            dev.icon_cache_mut().clear();
        }
        let report = frame_reset_report();
        self.transport.send_feature(&report, Duration::from_millis(200))
    }

    /// Poll for an edge-triggered control event.
    pub fn poll_event(&self) -> Option<ControlEvent> {
        self.input_queue.pop()
    }

    /// Drain all pending events.
    pub fn drain_events(&self) -> Vec<ControlEvent> {
        self.input_queue.drain()
    }

    /// Enable or disable raw report diagnostic logging.
    pub fn set_raw_diagnostics(&self, enabled: bool) {
        self.raw_diagnostics.store(enabled, Ordering::SeqCst);
        if enabled {
            self.raw_dumps_remaining.store(self.config.max_raw_dumps, Ordering::SeqCst);
        }
    }

    pub fn is_connected(&self) -> bool {
        self.is_connected.load(Ordering::SeqCst)
    }

    pub fn total_presses(&self) -> u64 {
        self.total_presses.load(Ordering::Relaxed)
    }

    pub fn total_releases(&self) -> u64 {
        self.total_releases.load(Ordering::Relaxed)
    }

    pub fn reconnect_count(&self) -> u64 {
        self.reconnect_count.load(Ordering::Relaxed)
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

impl Drop for StreamDeckController {
    fn drop(&mut self) {
        self.stop();
        if let Some(t) = self._input_thread.take() {
            let _ = t.join();
        }
        if let Some(t) = self._output_thread.take() {
            let _ = t.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::MockTransport;

    #[test]
    fn test_controller_initialization_and_events() {
        let mock = Arc::new(MockTransport::new());
        let config = ControllerConfig {
            surface_id: SurfaceId::new("test-sdxl"),
            broker_state_path: std::env::temp_dir().join("test-state.json"),
            boot_ledger_path: std::env::temp_dir().join("test-ledger.jsonl"),
            ..Default::default()
        };

        let controller = StreamDeckController::new(config, mock.clone());

        // Allow connection initialization to register
        thread::sleep(Duration::from_millis(50));
        assert!(controller.is_connected());

        // Drain connected event
        let events = controller.drain_events();
        assert!(events.iter().any(|e| matches!(e, ControlEvent::Connected { .. })));

        // Inject key 5 press
        mock.inject_key_action(5, true);
        thread::sleep(Duration::from_millis(50));

        let events = controller.drain_events();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0],
            ControlEvent::Press {
                key: 5,
                logical_key: 5,
                at_ns: events[0].at_ns().unwrap(),
            }
        );

        // Holding key 5: no duplicate event
        thread::sleep(Duration::from_millis(50));
        assert!(controller.drain_events().is_empty());

        // Release key 5
        mock.inject_key_action(5, false);
        thread::sleep(Duration::from_millis(50));

        let events = controller.drain_events();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0],
            ControlEvent::Release {
                key: 5,
                logical_key: 5,
                at_ns: events[0].at_ns().unwrap(),
            }
        );

        controller.stop();
    }

    trait ControlEventExt {
        fn at_ns(&self) -> Option<u64>;
    }

    impl ControlEventExt for ControlEvent {
        fn at_ns(&self) -> Option<u64> {
            match self {
                ControlEvent::Press { at_ns, .. } => Some(*at_ns),
                ControlEvent::Release { at_ns, .. } => Some(*at_ns),
                _ => None,
            }
        }
    }
}
