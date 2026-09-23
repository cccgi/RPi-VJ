//! End-to-end integration test for Task A3-07: Stream Deck State UI & Command Integration.
//!
//! Verifies observable (post page-consolidation: PERFORMANCE/SYSTEM/INGEST only):
//! - Boots directly onto PERFORMANCE (no separate BOOTING/READY landing pages)
//! - Switch banks via the bank-select modal (Banks 0..7)
//! - Select / trigger a clip (with active clip highlight)
//! - Restart renderer simulation and confirm restored state replay
//! - View real telemetry (hostname, temperature) on SYSTEM page
//! - Perform guarded clean shutdown using Stream Deck controls

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use vj_surface_hid::{
    broker_client::BrokerClient,
    controller::{ControllerConfig, StreamDeckController},
    pages::PageId,
    transport::MockTransport,
    ui_engine::UIEngine,
    SurfaceId,
};

#[test]
fn test_observable_a3_07_complete_streamdeck_operator_flow() {
    let mock_transport = Arc::new(MockTransport::new());
    let config = ControllerConfig {
        surface_id: SurfaceId::new("sdxl-a3-07-flow"),
        broker_state_path: std::env::temp_dir().join("test-state-a307.json"),
        boot_ledger_path: std::env::temp_dir().join("test-ledger-a307.jsonl"),
        ..Default::default()
    };

    let controller = Arc::new(StreamDeckController::new(config, mock_transport.clone()));
    let (broker, mock_broker) = BrokerClient::with_mock();
    let broker_arc = Arc::new(broker);

    let mut ui = UIEngine::new(controller.clone(), broker_arc.clone());

    // 1. PERFORMANCE: boots directly here -- no BOOTING/READY landing pages
    assert_eq!(ui.active_page_id(), PageId::Performance);

    // Step forward to Bank 3 with instant BANK NEXT (key 25), 3 presses from bank 0
    for _ in 0..3 {
        ui.on_key_press(25);
        ui.on_key_release(25);
    }
    assert_eq!(ui.context().state.active_bank, 3);

    thread::sleep(Duration::from_millis(50));
    let lines = mock_broker.drain_outgoing();
    assert!(lines.iter().any(|l| l.contains("\"bank.select\"") && l.contains("\"index\":3")));

    // Select Clip 9
    ui.on_key_press(9);
    ui.on_key_release(9);
    assert_eq!(ui.context().state.active_clip, Some(9));

    thread::sleep(Duration::from_millis(50));
    let lines = mock_broker.drain_outgoing();
    assert!(lines.iter().any(|l| l.contains("\"clip.trigger\"") && l.contains("\"key\":9")));

    // Toggle blackout (Key 31, the fixed panic key)
    ui.on_key_press(31);
    ui.on_key_release(31);
    assert!(ui.context().state.blackout);

    // 4. RENDERER RESTART & STATE REPLAY:
    // Renderer restarts -> state replay restores active bank (3), active clip (9), blackout (true)
    mock_broker.drain_outgoing();
    broker_arc.replay_state();

    thread::sleep(Duration::from_millis(50));
    let replay_lines = mock_broker.drain_outgoing();
    assert!(replay_lines.iter().any(|l| l.contains("\"bank.select\"") && l.contains("\"index\":3")));
    assert!(replay_lines.iter().any(|l| l.contains("\"clip.trigger\"") && l.contains("\"key\":9")));
    assert!(replay_lines.iter().any(|l| l.contains("\"blackout.set\"") && l.contains("\"value\":true")));

    // 5. SYSTEM PAGE: View real telemetry (hostname, CPU temp)
    ui.set_page(PageId::System);
    assert_eq!(ui.active_page_id(), PageId::System);
    assert_eq!(ui.context().telemetry.hostname, "rpi-vj.local");
    assert!(ui.context().telemetry.temperature_c > 0.0);

    // 6. GUARDED SHUTDOWN:
    // Press shutdown key (Key 27, post-remap position) -> arms 2-key confirmation mode
    ui.on_key_press(27);
    assert!(ui.guarded.is_active());

    // Confirm shutdown by pressing confirm key (Key 0)
    mock_broker.drain_outgoing();
    ui.on_key_press(0);
    ui.on_key_release(0);

    thread::sleep(Duration::from_millis(50));
    let shutdown_lines = mock_broker.drain_outgoing();
    assert!(
        shutdown_lines.iter().any(|l| l.contains("\"system.shutdown\"")),
        "Guarded shutdown must submit system.shutdown command"
    );

    controller.stop();
    broker_arc.stop();
}

#[test]
fn test_observable_a3_11_streamdeck_usb_ingest_complete_workflow() {
    let mock_transport = Arc::new(MockTransport::new());
    let config = ControllerConfig {
        surface_id: SurfaceId::new("sdxl-a3-11-flow"),
        broker_state_path: std::env::temp_dir().join("test-state-a311.json"),
        boot_ledger_path: std::env::temp_dir().join("test-ledger-a311.jsonl"),
        ..Default::default()
    };

    let controller = Arc::new(StreamDeckController::new(config, mock_transport.clone()));
    let (broker, mock_broker) = BrokerClient::with_mock();
    let broker_arc = Arc::new(broker);

    let mut ui = UIEngine::new(controller.clone(), broker_arc.clone());

    // 1. Switch to Ingest Page
    ui.set_page(PageId::Ingest);
    assert_eq!(ui.active_page_id(), PageId::Ingest);

    // 2. Populate Ingest Context with Drive and Queue Status (5 clips discovered)
    {
        let ctx = ui.context_mut();
        ctx.ingest.usb_inserted = true;
        ctx.ingest.usb_status = vj_surface_hid::pages::UsbDriveStatus::Ready;
        ctx.ingest.file_count = 5;
        ctx.ingest.current_file_index = 1;
        ctx.ingest.current_file_name = Some("clip1_quad.mp4".to_string());
        ctx.ingest.step = "Analyzing".to_string();
        ctx.ingest.progress = 0.50; // 50% through pipeline
        ctx.ingest.recommendation = vj_surface_hid::pages::MediaRecommendation::QuadMirrorDetected;
    }

    ui.render_full_page();

    // Verify key 0 shows USB DRIVE READY
    assert_eq!(ui.context().ingest.usb_status, vj_surface_hid::pages::UsbDriveStatus::Ready);
    assert_eq!(ui.context().ingest.file_count, 5);

    // 3. Interactive Decision Testing:
    // Case A: Accept Recommendation (Key 16)
    mock_broker.drain_outgoing();
    ui.on_key_press(16);
    ui.on_key_release(16);
    assert!(ui.context().ingest.proposed_crop); // QuadMirror recommendation sets crop=true
    thread::sleep(Duration::from_millis(30));
    let lines = mock_broker.drain_outgoing();
    assert!(lines.iter().any(|l| l.contains("\"ingest.accept_recommendation\"") && l.contains("\"crop\":true")));

    // Case B: Keep Full Frame Override (Key 17) -> Disables crop
    ui.on_key_press(17);
    ui.on_key_release(17);
    assert!(!ui.context().ingest.proposed_crop);
    thread::sleep(Duration::from_millis(30));
    let lines = mock_broker.drain_outgoing();
    assert!(lines.iter().any(|l| l.contains("\"ingest.decision_override\"") && l.contains("\"crop\":false")));

    // Case C: Crop Quadrant Force (Key 18) -> Forces crop
    ui.on_key_press(18);
    ui.on_key_release(18);
    assert!(ui.context().ingest.proposed_crop);
    thread::sleep(Duration::from_millis(30));
    let lines = mock_broker.drain_outgoing();
    assert!(lines.iter().any(|l| l.contains("\"ingest.decision_override\"") && l.contains("\"force_crop\":true")));

    // Case D: Repair Seam Force (Key 19) -> Forces blend seam
    ui.on_key_press(19);
    ui.on_key_release(19);
    assert!(ui.context().ingest.proposed_seam);
    thread::sleep(Duration::from_millis(30));
    let lines = mock_broker.drain_outgoing();
    assert!(lines.iter().any(|l| l.contains("\"ingest.decision_override\"") && l.contains("\"repair_seam\":true")));

    // Case E: Skip File (Key 20)
    ui.on_key_press(20);
    ui.on_key_release(20);
    thread::sleep(Duration::from_millis(30));
    let lines = mock_broker.drain_outgoing();
    assert!(lines.iter().any(|l| l.contains("\"ingest.skip_file\"")));

    // 4. Performance Mode / Priority Testing:
    // Switch to PERFORMANCE Page -> Transcode jobs must automatically pause
    ui.on_key_press(31); // Exit to System (post-consolidation exit target)
    ui.on_key_release(31);
    assert_eq!(ui.active_page_id(), PageId::System);

    mock_broker.drain_outgoing();
    ui.set_page(PageId::Performance);
    assert_eq!(ui.active_page_id(), PageId::Performance);
    assert!(ui.context().ingest.transcode_paused_for_perf);

    thread::sleep(Duration::from_millis(30));
    let perf_lines = mock_broker.drain_outgoing();
    assert!(perf_lines.iter().any(|l| l.contains("\"ingest.pause_transcode\"")));

    // Leaving Performance back to Ingest resumes transcode
    ui.set_page(PageId::Ingest);
    assert_eq!(ui.active_page_id(), PageId::Ingest);
    assert!(!ui.context().ingest.transcode_paused_for_perf);

    thread::sleep(Duration::from_millis(30));
    let resume_lines = mock_broker.drain_outgoing();
    assert!(resume_lines.iter().any(|l| l.contains("\"ingest.resume_transcode\"")));

    // 5. Guarded Safe Eject (Key 28 with 3s hold)
    mock_broker.drain_outgoing();
    let hold_start = std::time::Instant::now();
    ui.on_key_press_at(28, hold_start);
    assert!(ui.guarded.is_active());

    // Advance context time past 3.0s and tick UI engine
    ui.tick_at(hold_start + Duration::from_millis(3100));
    thread::sleep(Duration::from_millis(30));
    let eject_lines = mock_broker.drain_outgoing();
    assert!(eject_lines.iter().any(|l| l.contains("\"usb.eject\"")));
    assert_eq!(ui.context().ingest.usb_status, vj_surface_hid::pages::UsbDriveStatus::Ejected);
    assert!(!ui.context().ingest.usb_inserted);

    // 6. Guarded Activate Show Generation (Key 27 with 3s hold)
    mock_broker.drain_outgoing();
    let act_start = std::time::Instant::now();
    ui.on_key_press_at(27, act_start);
    assert!(ui.guarded.is_active());

    ui.tick_at(act_start + Duration::from_millis(3100));
    thread::sleep(Duration::from_millis(30));
    let activate_lines = mock_broker.drain_outgoing();
    assert!(activate_lines.iter().any(|l| l.contains("\"show.activate\"")));

    controller.stop();
    broker_arc.stop();
}

