//! Required observable tests for RPi-VJ Appliance V3 task A3-06.
//!
//! Observable verification criteria:
//! 1. Run without Chromium (pure native Rust driver, zero browser/WebHID).
//! 2. All 32 keys produce the correct logical index exactly once.
//! 3. Icons match their keys.
//! 4. Ten unplug/replug cycles restore the page within five seconds.
//! 5. An intentionally stalled icon write does not block presses.

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use vj_surface_hid::{
    controller::{ControllerConfig, StreamDeckController},
    device::{STREAM_DECK_XL_KEY_COUNT, STREAM_DECK_XL_REPORT_SIZE},
    transport::MockTransport,
    ControlEvent, SurfaceId,
};

fn create_test_controller() -> (StreamDeckController, Arc<MockTransport>, tempfile_holder::TempDir) {
    let tmp = tempfile_holder::TempDir::new("sdxl-obs-test");
    let broker_path = tmp.path().join("controller-state.json");
    let ledger_path = tmp.path().join("boot-ledger.jsonl");

    let mock = Arc::new(MockTransport::new());
    let config = ControllerConfig {
        surface_id: SurfaceId::new("sdxl-observable"),
        read_timeout: Duration::from_millis(10),
        write_timeout: Duration::from_millis(500),
        reconnect_poll_interval: Duration::from_millis(20),
        broker_state_path: broker_path,
        boot_ledger_path: ledger_path,
        ..Default::default()
    };

    let controller = StreamDeckController::new(config, mock.clone());
    // Wait for connection initialization
    thread::sleep(Duration::from_millis(50));
    (controller, mock, tmp)
}

/// 1 & 2: All 32 keys produce the correct logical index exactly once on press and release.
#[test]
fn test_observable_all_32_keys_produce_correct_logical_index_exactly_once() {
    let (controller, mock, _tmp) = create_test_controller();
    assert!(controller.is_connected());

    // Drain initial connection events
    let _ = controller.drain_events();

    for key in 0..STREAM_DECK_XL_KEY_COUNT as u8 {
        // Press key
        mock.inject_key_action(key, true);
        thread::sleep(Duration::from_millis(25));

        let events = controller.drain_events();
        assert_eq!(
            events.len(),
            1,
            "Key {key} press must emit exactly one event"
        );
        match &events[0] {
            ControlEvent::Press {
                key: k,
                logical_key: lk,
                ..
            } => {
                assert_eq!(*k, key, "Hardware key index must match {key}");
                assert_eq!(*lk, key, "Logical key index must match {key}");
            }
            other => panic!("Expected Press event for key {key}, got {:?}", other),
        }

        // Held key across multiple polls: zero additional events
        for _ in 0..5 {
            thread::sleep(Duration::from_millis(10));
            assert!(
                controller.drain_events().is_empty(),
                "Held key {key} must not fire repeat triggers"
            );
        }

        // Release key
        mock.inject_key_action(key, false);
        thread::sleep(Duration::from_millis(25));

        let rel_events = controller.drain_events();
        assert_eq!(
            rel_events.len(),
            1,
            "Key {key} release must emit exactly one event"
        );
        match &rel_events[0] {
            ControlEvent::Release {
                key: k,
                logical_key: lk,
                ..
            } => {
                assert_eq!(*k, key, "Release hardware key index must match {key}");
                assert_eq!(*lk, key, "Release logical key index must match {key}");
            }
            other => panic!("Expected Release event for key {key}, got {:?}", other),
        }
    }

    assert_eq!(controller.total_presses(), 32);
    assert_eq!(controller.total_releases(), 32);
    controller.stop();
}

/// 3: Icons match their keys.
#[test]
fn test_observable_icons_match_their_keys() {
    let (controller, mock, _tmp) = create_test_controller();
    assert!(controller.is_connected());

    mock.clear_written_reports();

    // Send 32 distinct icons, each with distinct identifiable bytes for key k
    for k in 0..STREAM_DECK_XL_KEY_COUNT as u8 {
        let jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0, k, 0xAB, 0xCD, 0xEF];
        controller.set_icon(k, jpeg);
    }

    // Wait for output worker to transmit all 32 icons
    let start = Instant::now();
    loop {
        let written = mock.get_written_reports();
        if written.len() >= 32 || start.elapsed() > Duration::from_secs(3) {
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }

    let written = mock.get_written_reports();
    assert!(
        written.len() >= 32,
        "Expected at least 32 icon reports written, got {}",
        written.len()
    );

    // Verify each key's report matches its key index and payload
    let mut verified_keys = [false; STREAM_DECK_XL_KEY_COUNT];
    for report in &written {
        assert_eq!(report.len(), STREAM_DECK_XL_REPORT_SIZE);
        assert_eq!(report[0], 0x02, "Report ID must be 0x02");
        assert_eq!(report[1], 0x07, "Command must be 0x07 (Set Image)");

        let key = report[2] as usize;
        assert!(key < STREAM_DECK_XL_KEY_COUNT);
        // Byte 8..8+len contains payload slice: byte 12 (8+4) should be key as u8
        assert_eq!(
            report[12], key as u8,
            "Icon payload content must match key {key}"
        );
        verified_keys[key] = true;
    }

    for (k, &verified) in verified_keys.iter().enumerate() {
        assert!(verified, "Key {k} must have had its icon transmitted");
    }

    controller.stop();
}

/// 4: Ten unplug/replug cycles restore the page within five seconds.
#[test]
fn test_observable_ten_unplug_replug_cycles_restore_page_within_five_seconds() {
    let (controller, mock, _tmp) = create_test_controller();
    assert!(controller.is_connected());

    // Populate full 32-key page into cache
    let page: [Option<Vec<u8>>; STREAM_DECK_XL_KEY_COUNT] = std::array::from_fn(|i| {
        Some(vec![0xFF, 0xD8, 0xFF, 0xDB, i as u8, 0x11, 0x22, 0x33])
    });
    controller.set_page(1, &page);

    // Wait for initial upload to complete
    let start_init = Instant::now();
    while mock.get_written_reports().len() < 32 && start_init.elapsed() < Duration::from_secs(2) {
        thread::sleep(Duration::from_millis(20));
    }
    assert!(mock.get_written_reports().len() >= 32, "Initial page upload failed");

    let benchmark_start = Instant::now();

    for cycle in 1..=10 {
        // 1. Simulate physical USB unplug
        mock.simulate_unplug();

        // Wait for controller to register disconnect
        let dc_start = Instant::now();
        while controller.is_connected() && dc_start.elapsed() < Duration::from_millis(300) {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(!controller.is_connected(), "Cycle {cycle}: controller must register disconnect");

        mock.clear_written_reports();

        // 2. Simulate physical USB replug
        mock.simulate_replug();

        // Wait for controller to reconnect and replay cached page (all 32 keys)
        let rc_start = Instant::now();
        while (!controller.is_connected() || mock.get_written_reports().len() < 32)
            && rc_start.elapsed() < Duration::from_millis(400)
        {
            thread::sleep(Duration::from_millis(5));
        }

        assert!(
            controller.is_connected(),
            "Cycle {cycle}: controller must reconnect"
        );
        let written = mock.get_written_reports();
        assert!(
            written.len() >= 32,
            "Cycle {cycle}: all 32 cached icons must be replayed upon reconnect (got {})",
            written.len()
        );
    }

    let total_elapsed = benchmark_start.elapsed();
    println!(
        "Ten unplug/replug cycles completed in {:?} (target: < 5.0 seconds)",
        total_elapsed
    );
    assert!(
        total_elapsed < Duration::from_secs(5),
        "10 unplug/replug cycles must restore page within 5.0 seconds, took {:?}",
        total_elapsed
    );

    controller.stop();
}

/// 5: An intentionally stalled icon write does not block presses.
#[test]
fn test_observable_stalled_icon_write_does_not_block_presses() {
    let (controller, mock, _tmp) = create_test_controller();
    assert!(controller.is_connected());
    let _ = controller.drain_events();

    // Configure an intentional write stall of 800ms on the transport
    mock.set_write_stall(Some(Duration::from_millis(800)));

    // Queue an icon write for key 0 — this causes the output thread to block inside write() for 800ms
    controller.set_icon(0, vec![0xFF, 0xD8, 0xAA, 0xBB, 0xCC]);

    // Give output thread a tiny slice to pick up the item and enter the stalled write call
    thread::sleep(Duration::from_millis(20));

    // While the output thread is blocked in write(), immediately inject button presses
    let press_start = Instant::now();
    let mut states = [false; STREAM_DECK_XL_KEY_COUNT];
    states[7] = true;
    states[14] = true;
    mock.inject_button_states(&states);

    // Wait for the button events to arrive in the input queue
    let mut received = Vec::new();
    let poll_deadline = Instant::now() + Duration::from_millis(200);

    while Instant::now() < poll_deadline && received.len() < 2 {
        if let Some(event) = controller.poll_event() {
            received.push(event);
        } else {
            thread::sleep(Duration::from_millis(2));
        }
    }

    let press_latency = press_start.elapsed();
    println!(
        "Button presses received in {:?} while icon write was stalled for 800ms",
        press_latency
    );

    // Must be received rapidly (< 80ms), completely unaffected by the 800ms stalled icon write!
    assert!(
        press_latency < Duration::from_millis(80),
        "Button press latency ({:?}) must not be blocked by stalled icon write",
        press_latency
    );

    assert_eq!(received.len(), 2);
    assert!(matches!(
        &received[0],
        ControlEvent::Press { key: 7, logical_key: 7, .. }
    ));
    assert!(matches!(
        &received[1],
        ControlEvent::Press { key: 14, logical_key: 14, .. }
    ));

    mock.set_write_stall(None);
    controller.stop();
}

/// Helper module for managing temporary test directories without external crate dependencies.
mod tempfile_holder {
    use std::path::{Path, PathBuf};

    pub struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        pub fn new(prefix: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "{}-{}-{}",
                prefix,
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let _ = std::fs::create_dir_all(&path);
            Self { path }
        }

        pub fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}
