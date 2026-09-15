//! End-to-end integration test for Task A3-07: Stream Deck State UI & Command Integration.
//!
//! Verifies observable:
//! - Complete boot-status inspection (BOOTING page)
//! - Switch banks (Banks 0..7)
//! - Select / trigger a clip (with active clip highlight)
//! - Restart renderer simulation and confirm restored state replay
//! - View current IP, hostname, free space, and temperature (SYSTEM page)
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

    // 1. BOOTING: Boot status inspection
    ui.set_page(PageId::Booting);
    assert_eq!(ui.active_page_id(), PageId::Booting);

    // Verify boot page keys are populated
    let boot_canvas = ui.context().boot_storage_ok && ui.context().boot_renderer_ok;
    assert!(boot_canvas);

    // Press Ready entry key (key 31) on Booting page
    ui.on_key_press(31);
    ui.on_key_release(31);
    assert_eq!(ui.active_page_id(), PageId::Ready);

    // 2. READY: Switch banks (e.g. Bank 3)
    ui.on_key_press(3);
    ui.on_key_release(3);
    assert_eq!(ui.context().state.active_bank, 3);

    thread::sleep(Duration::from_millis(50));
    let lines = mock_broker.drain_outgoing();
    assert!(lines.iter().any(|l| l.contains("\"bank.select\"") && l.contains("\"index\":3")));

    // 3. PERFORMANCE: Arm performance mode (Key 16)
    ui.on_key_press(16);
    ui.on_key_release(16);
    assert_eq!(ui.active_page_id(), PageId::Performance);

    // Select Clip 9
    ui.on_key_press(9);
    ui.on_key_release(9);
    assert_eq!(ui.context().state.active_clip, Some(9));

    thread::sleep(Duration::from_millis(50));
    let lines = mock_broker.drain_outgoing();
    assert!(lines.iter().any(|l| l.contains("\"clip.trigger\"") && l.contains("\"key\":9")));

    // Toggle blackout (Key 28)
    ui.on_key_press(28);
    ui.on_key_release(28);
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

    // 5. SYSTEM PAGE: View IP / Hostname / Free Space / Temp
    ui.set_page(PageId::System);
    assert_eq!(ui.active_page_id(), PageId::System);
    assert_eq!(ui.context().telemetry.hostname, "rpi-vj.local");
    assert!(!ui.context().telemetry.ip_address.is_empty());
    assert!(ui.context().telemetry.free_space_bytes > 0);

    // 6. GUARDED SHUTDOWN:
    // Press shutdown key (Key 26) -> arms 2-key confirmation mode
    ui.on_key_press(26);
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
