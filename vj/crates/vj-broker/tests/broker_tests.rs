use std::sync::Arc;
use std::time::{Duration, Instant};
use vj_broker::protocol::{AckStatus, Message, PROTOCOL_VERSION};
use vj_broker::socket::BrokerContext;
use vj_broker::state::StateStore;
use vj_broker::supervisor::{EscalationLevel, Supervisor, SupervisorAction};

#[test]
fn test_offline_command_acceptance() {
    let t0 = Instant::now();
    let store = StateStore::new(None);
    let sup = Supervisor::new(None, None, t0, 1700000000);
    let ctx = Arc::new(BrokerContext::new(store, sup));

    // 1. Submit clip trigger command while renderer is not active
    let cmd = Message::command("cmd-clip-1", "clip.trigger", serde_json::json!({"key": 5, "bank": 2}));
    let raw = serde_json::to_string(&cmd).unwrap();
    let resp = ctx.handle_message(&raw);

    // 2. Acceptance must be immediate (status: "accepted")
    assert_eq!(
        resp,
        Some(Message::Ack {
            v: PROTOCOL_VERSION,
            id: "cmd-clip-1".to_string(),
            status: AckStatus::Accepted,
            detail: None,
        })
    );

    // 3. Query state when renderer connects
    let query_resp = ctx.handle_message(r#"{"v":1,"type":"query"}"#);
    match query_resp {
        Some(Message::State { v: 1, state }) => {
            assert_eq!(state.get("active_clip").and_then(|v| v.as_u64()), Some(5));
            assert_eq!(state.get("active_bank").and_then(|v| v.as_u64()), Some(2));
        }
        other => panic!("Unexpected query response: {:?}", other),
    }
}

#[test]
fn test_command_deduplication() {
    let t0 = Instant::now();
    let store = StateStore::new(None);
    let sup = Supervisor::new(None, None, t0, 1700000000);
    let ctx = Arc::new(BrokerContext::new(store, sup));

    let cmd = Message::command("duplicate-uuid", "tempo.bpm", serde_json::json!({"bpm": 135.0}));
    let raw = serde_json::to_string(&cmd).unwrap();

    let resp1 = ctx.handle_message(&raw);
    let resp2 = ctx.handle_message(&raw);

    assert_eq!(resp1, resp2);
    assert_eq!(
        resp1,
        Some(Message::Ack {
            v: PROTOCOL_VERSION,
            id: "duplicate-uuid".to_string(),
            status: AckStatus::Accepted,
            detail: None,
        })
    );

    // Version counter should only increment once
    let state = ctx.state_store.lock().unwrap().get_state().clone();
    assert_eq!(state.version, 2);
    assert_eq!(state.tempo_bpm, 135.0);
}

#[test]
fn test_monotonic_supervisor_wall_clock_immunity() {
    let t0 = Instant::now();
    let mut sup = Supervisor::new(None, None, t0, 1700000000);

    // Initial state
    assert_eq!(sup.get_level(), EscalationLevel::Normal);

    // Simulate huge jump in wall-clock time (e.g. NTP sync jumps 10,000s into future)
    // but monotonic time only advanced 1 second
    let monotonic_t1 = t0 + Duration::from_secs(1);
    let epoch_jumped = 1700010000; // Jumped 10,000s!

    let act = sup.check_health(monotonic_t1, epoch_jumped);
    // Because monotonic duration is 1s (< 3s warning, < 6s restart), no false recovery occurs!
    assert_eq!(act, SupervisorAction::None);
    assert_eq!(sup.get_level(), EscalationLevel::Normal);
}
