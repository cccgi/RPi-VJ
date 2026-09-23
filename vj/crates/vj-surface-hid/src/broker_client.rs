//! Broker IPC client connecting to `/run/rpi-vj/broker.sock`.
//!
//! Submits commands immediately over non-blocking queues without holding
//! global input locks or blocking on icon writes.
//! Handles background reconnects, state subscription, and state replay across renderer restarts.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::io::{BufRead, BufReader, Write};
#[cfg(unix)]
use std::path::PathBuf;

pub const DEFAULT_BROKER_SOCKET_PATH: &str = "/run/rpi-vj/broker.sock";
pub const PROTOCOL_VERSION: u32 = 1;

/// Show state mirrored from broker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppState {
    pub version: u64,
    pub show_name: String,
    pub active_bank: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_clip: Option<u32>,
    pub blackout: bool,
    pub master_opacity: f32,
    pub tempo_bpm: f32,
    #[serde(default)]
    pub properties: serde_json::Value,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            version: 1,
            show_name: "default".to_string(),
            active_bank: 0,
            active_clip: None,
            blackout: false,
            master_opacity: 1.0,
            tempo_bpm: 120.0,
            properties: serde_json::json!({}),
        }
    }
}

/// Outgoing command sent to broker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrokerCommand {
    pub v: u32,
    pub id: String,
    #[serde(rename = "type")]
    pub msg_type: String,
    pub name: String,
    #[serde(default)]
    pub args: serde_json::Value,
}

impl BrokerCommand {
    pub fn new(id: impl Into<String>, name: impl Into<String>, args: serde_json::Value) -> Self {
        Self {
            v: PROTOCOL_VERSION,
            id: id.into(),
            msg_type: "command".to_string(),
            name: name.into(),
            args,
        }
    }
}

/// Abstract transport trait for broker IPC.
pub trait BrokerTransport: Send + Sync + 'static {
    fn send_line(&self, line: &str) -> Result<(), std::io::Error>;
    fn read_line(&self, timeout: Duration) -> Result<Option<String>, std::io::Error>;
    fn is_connected(&self) -> bool;
    fn reconnect(&self) -> bool;
}

#[cfg(unix)]
pub struct UnixSocketTransport {
    path: PathBuf,
    stream: Mutex<Option<std::os::unix::net::UnixStream>>,
    reader: Mutex<Option<BufReader<std::os::unix::net::UnixStream>>>,
}

#[cfg(unix)]
impl UnixSocketTransport {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let p = path.into();
        let mut transport = Self {
            path: p,
            stream: Mutex::new(None),
            reader: Mutex::new(None),
        };
        transport.reconnect();
        transport
    }
}

#[cfg(unix)]
impl BrokerTransport for UnixSocketTransport {
    fn send_line(&self, line: &str) -> Result<(), std::io::Error> {
        let mut lock = self.stream.lock().unwrap();
        if let Some(stream) = lock.as_mut() {
            stream.write_all(line.as_bytes())?;
            stream.write_all(b"\n")?;
            stream.flush()?;
            Ok(())
        } else {
            Err(std::io::Error::new(std::io::ErrorKind::NotConnected, "socket not connected"))
        }
    }

    fn read_line(&self, timeout: Duration) -> Result<Option<String>, std::io::Error> {
        let mut r_lock = self.reader.lock().unwrap();
        if let Some(reader) = r_lock.as_mut() {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    // EOF
                    drop(r_lock);
                    *self.stream.lock().unwrap() = None;
                    *self.reader.lock().unwrap() = None;
                    Ok(None)
                }
                Ok(_) => Ok(Some(line.trim_end().to_string())),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {
                    Ok(None)
                }
                Err(e) => Err(e),
            }
        } else {
            thread::sleep(timeout);
            Ok(None)
        }
    }

    fn is_connected(&self) -> bool {
        self.stream.lock().unwrap().is_some()
    }

    fn reconnect(&self) -> bool {
        if let Ok(stream) = std::os::unix::net::UnixStream::connect(&self.path) {
            let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
            let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
            if let Ok(clone_read) = stream.try_clone() {
                *self.reader.lock().unwrap() = Some(BufReader::new(clone_read));
            }
            *self.stream.lock().unwrap() = Some(stream);
            true
        } else {
            false
        }
    }
}

/// In-memory mock broker transport for unit and integration testing.
pub struct MockBrokerTransport {
    connected: Arc<AtomicBool>,
    outgoing_lines: Arc<Mutex<VecDeque<String>>>,
    incoming_lines: Arc<Mutex<VecDeque<String>>>,
}

impl MockBrokerTransport {
    pub fn new() -> Self {
        Self {
            connected: Arc::new(AtomicBool::new(true)),
            outgoing_lines: Arc::new(Mutex::new(VecDeque::new())),
            incoming_lines: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub fn push_incoming(&self, line: impl Into<String>) {
        self.incoming_lines.lock().unwrap().push_back(line.into());
    }

    pub fn pop_outgoing(&self) -> Option<String> {
        self.outgoing_lines.lock().unwrap().pop_front()
    }

    pub fn drain_outgoing(&self) -> Vec<String> {
        self.outgoing_lines.lock().unwrap().drain(..).collect()
    }

    pub fn set_connected(&self, conn: bool) {
        self.connected.store(conn, Ordering::SeqCst);
    }
}

impl Default for MockBrokerTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl BrokerTransport for MockBrokerTransport {
    fn send_line(&self, line: &str) -> Result<(), std::io::Error> {
        if !self.connected.load(Ordering::SeqCst) {
            return Err(std::io::Error::new(std::io::ErrorKind::NotConnected, "mock disconnected"));
        }
        self.outgoing_lines.lock().unwrap().push_back(line.to_string());
        Ok(())
    }

    fn read_line(&self, timeout: Duration) -> Result<Option<String>, std::io::Error> {
        if !self.connected.load(Ordering::SeqCst) {
            thread::sleep(timeout);
            return Ok(None);
        }
        if let Some(line) = self.incoming_lines.lock().unwrap().pop_front() {
            Ok(Some(line))
        } else {
            thread::sleep(timeout);
            Ok(None)
        }
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    fn reconnect(&self) -> bool {
        self.connected.store(true, Ordering::SeqCst);
        true
    }
}

pub struct BrokerClient {
    transport: Arc<dyn BrokerTransport>,
    state: Arc<Mutex<AppState>>,
    outgoing_queue: Arc<Mutex<VecDeque<BrokerCommand>>>,
    command_counter: Arc<AtomicU64>,
    running: Arc<AtomicBool>,
    connected: Arc<AtomicBool>,
    _worker_thread: Option<JoinHandle<()>>,
}

impl BrokerClient {
    pub fn new(transport: Arc<dyn BrokerTransport>) -> Self {
        let state = Arc::new(Mutex::new(AppState::default()));
        let outgoing_queue = Arc::new(Mutex::new(VecDeque::with_capacity(128)));
        let command_counter = Arc::new(AtomicU64::new(1));
        let running = Arc::new(AtomicBool::new(true));
        let connected = Arc::new(AtomicBool::new(transport.is_connected()));

        let mut client = Self {
            transport,
            state,
            outgoing_queue,
            command_counter,
            running,
            connected,
            _worker_thread: None,
        };

        client.start_worker();
        client
    }

    pub fn with_mock() -> (Self, Arc<MockBrokerTransport>) {
        let mock = Arc::new(MockBrokerTransport::new());
        let client = Self::new(mock.clone());
        (client, mock)
    }

    fn start_worker(&mut self) {
        let transport = self.transport.clone();
        let state = self.state.clone();
        let queue = self.outgoing_queue.clone();
        let running = self.running.clone();
        let connected = self.connected.clone();

        let handle = thread::Builder::new()
            .name("broker-ipc".into())
            .spawn(move || {
                let mut last_reconnect = Instant::now();

                while running.load(Ordering::SeqCst) {
                    let is_conn = transport.is_connected();
                    connected.store(is_conn, Ordering::SeqCst);

                    if !is_conn {
                        if last_reconnect.elapsed() >= Duration::from_millis(500) {
                            last_reconnect = Instant::now();
                            if transport.reconnect() {
                                connected.store(true, Ordering::SeqCst);
                                // Subscribe immediately
                                let sub_cmd = serde_json::json!({
                                    "v": PROTOCOL_VERSION,
                                    "type": "subscribe"
                                });
                                let _ = transport.send_line(&sub_cmd.to_string());
                            }
                        }
                        thread::sleep(Duration::from_millis(20));
                        continue;
                    }

                    // 1. Drain and send all pending outgoing commands immediately
                    loop {
                        let cmd_to_send = {
                            let mut q = queue.lock().unwrap();
                            q.pop_front()
                        };

                        match cmd_to_send {
                            Some(cmd) => {
                                if let Ok(json) = serde_json::to_string(&cmd) {
                                    if let Err(e) = transport.send_line(&json) {
                                        eprintln!("[BROKER IPC] send failed: {}", e);
                                        connected.store(false, Ordering::SeqCst);
                                        break;
                                    }
                                }
                            }
                            None => break,
                        }
                    }

                    // 2. Read incoming messages
                    match transport.read_line(Duration::from_millis(10)) {
                        Ok(Some(line)) => {
                            Self::process_incoming_line(&line, &state);
                        }
                        Ok(None) => {}
                        Err(e) => {
                            eprintln!("[BROKER IPC] read error: {}", e);
                            connected.store(false, Ordering::SeqCst);
                        }
                    }

                    thread::sleep(Duration::from_millis(2));
                }
            })
            .expect("spawn broker client worker");

        self._worker_thread = Some(handle);
    }

    fn process_incoming_line(line: &str, state: &Arc<Mutex<AppState>>) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return;
        }

        if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if let Some(msg_type) = val.get("type").and_then(|v| v.as_str()) {
                if msg_type == "state" {
                    if let Some(state_val) = val.get("state") {
                        if let Ok(loaded) = serde_json::from_value::<AppState>(state_val.clone()) {
                            let mut st = state.lock().unwrap();
                            *st = loaded;
                        }
                    }
                }
            }
        }
    }

    fn next_command_id(&self) -> String {
        let seq = self.command_counter.fetch_add(1, Ordering::Relaxed);
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        format!("cmd-{}-{}", now_ms, seq)
    }

    /// Submit a command immediately to the non-blocking queue.
    pub fn send_command(&self, name: impl Into<String>, args: serde_json::Value) -> String {
        let id = self.next_command_id();
        let cmd = BrokerCommand::new(id.clone(), name, args);
        {
            let mut q = self.outgoing_queue.lock().unwrap();
            q.push_back(cmd);
        }
        id
    }

    // Convenience command helpers:
    pub fn trigger_clip(&self, key: u32, bank: Option<u32>) -> String {
        let mut args = serde_json::json!({ "key": key });
        if let Some(b) = bank {
            args["bank"] = serde_json::json!(b);
        }
        // Update optimistic local state immediately for instant responsive feedback
        {
            let mut st = self.state.lock().unwrap();
            st.active_clip = Some(key);
            if let Some(b) = bank {
                st.active_bank = b;
            }
        }
        self.send_command("clip.trigger", args)
    }

    pub fn stop_clip(&self) -> String {
        {
            let mut st = self.state.lock().unwrap();
            st.active_clip = None;
        }
        self.send_command("clip.stop", serde_json::json!({}))
    }

    pub fn select_bank(&self, bank: u32) -> String {
        {
            let mut st = self.state.lock().unwrap();
            st.active_bank = bank;
        }
        self.send_command("bank.select", serde_json::json!({ "index": bank }))
    }

    pub fn toggle_blackout(&self) -> String {
        {
            let mut st = self.state.lock().unwrap();
            st.blackout = !st.blackout;
        }
        self.send_command("blackout.toggle", serde_json::json!({}))
    }

    pub fn set_blackout(&self, value: bool) -> String {
        {
            let mut st = self.state.lock().unwrap();
            st.blackout = value;
        }
        self.send_command("blackout.set", serde_json::json!({ "value": value }))
    }

    /// Forward the raw key index (0-31) of a physical press, regardless of
    /// what it means on the current page. This does not touch local
    /// optimistic state -- it's a parallel signal purely for the gig web
    /// engine's bridge to detect and dispatch every press (including
    /// repeats of the same key) through its own authoritative per-page
    /// `handleKeyPress`/`handleSystemKey` logic, which already implements
    /// the full documented button mapping (see docs/original/STREAMDECK.md)
    /// -- the broker/native side deliberately does not duplicate it.
    pub fn notify_key_press(&self, key: u32) -> String {
        self.send_command("key.press", serde_json::json!({ "key": key }))
    }

    pub fn set_tempo_bpm(&self, bpm: f32) -> String {
        {
            let mut st = self.state.lock().unwrap();
            st.tempo_bpm = bpm.clamp(20.0, 300.0);
        }
        self.send_command("tempo.bpm", serde_json::json!({ "bpm": bpm }))
    }

    pub fn set_master_opacity(&self, opacity: f32) -> String {
        {
            let mut st = self.state.lock().unwrap();
            st.master_opacity = opacity.clamp(0.0, 1.0);
        }
        self.send_command("master.opacity", serde_json::json!({ "value": opacity }))
    }

    pub fn load_show(&self, name: &str) -> String {
        self.send_command("show.load", serde_json::json!({ "name": name }))
    }

    pub fn restart_renderer(&self) -> String {
        self.send_command("renderer.restart", serde_json::json!({}))
    }

    /// Replay active bank, clip, and blackout state on renderer reconnect or restart.
    pub fn replay_state(&self) {
        let (bank, clip, blackout, tempo, opacity) = {
            let st = self.state.lock().unwrap();
            (st.active_bank, st.active_clip, st.blackout, st.tempo_bpm, st.master_opacity)
        };

        self.select_bank(bank);
        if let Some(clip_idx) = clip {
            self.trigger_clip(clip_idx, Some(bank));
        }
        self.set_blackout(blackout);
        self.set_tempo_bpm(tempo);
        self.set_master_opacity(opacity);
    }

    pub fn state(&self) -> AppState {
        self.state.lock().unwrap().clone()
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

impl Drop for BrokerClient {
    fn drop(&mut self) {
        self.stop();
        if let Some(handle) = self._worker_thread.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_broker_client_non_blocking_send_and_state_sync() {
        let (client, mock) = BrokerClient::with_mock();
        assert!(client.is_connected());

        // Send clip trigger
        let id = client.trigger_clip(7, Some(2));
        assert!(id.starts_with("cmd-"));
        assert_eq!(client.state().active_clip, Some(7));
        assert_eq!(client.state().active_bank, 2);

        // Wait for worker to send line
        thread::sleep(Duration::from_millis(50));
        let lines = mock.drain_outgoing();
        assert!(!lines.is_empty());
        assert!(lines[0].contains("\"name\":\"clip.trigger\""));
        assert!(lines[0].contains("\"key\":7"));
        assert!(lines[0].contains("\"bank\":2"));

        // Simulate incoming state update from broker
        let state_msg = serde_json::json!({
            "v": 1,
            "type": "state",
            "state": {
                "version": 99,
                "show_name": "festival-2026",
                "active_bank": 3,
                "active_clip": 12,
                "blackout": true,
                "master_opacity": 0.8,
                "tempo_bpm": 135.0,
                "properties": {}
            }
        });
        mock.push_incoming(state_msg.to_string());

        thread::sleep(Duration::from_millis(50));
        let curr = client.state();
        assert_eq!(curr.version, 99);
        assert_eq!(curr.show_name, "festival-2026");
        assert_eq!(curr.active_bank, 3);
        assert_eq!(curr.active_clip, Some(12));
        assert!(curr.blackout);
        assert_eq!(curr.tempo_bpm, 135.0);

        client.stop();
    }

    #[test]
    fn test_state_replay() {
        let (client, mock) = BrokerClient::with_mock();
        // Setup state
        {
            let mut st = client.state.lock().unwrap();
            st.active_bank = 4;
            st.active_clip = Some(19);
            st.blackout = true;
            st.tempo_bpm = 140.0;
        }

        mock.drain_outgoing();
        client.replay_state();

        thread::sleep(Duration::from_millis(50));
        let lines = mock.drain_outgoing();
        assert!(lines.len() >= 4);
        assert!(lines.iter().any(|l| l.contains("\"bank.select\"") && l.contains("\"index\":4")));
        assert!(lines.iter().any(|l| l.contains("\"clip.trigger\"") && l.contains("\"key\":19")));
        assert!(lines.iter().any(|l| l.contains("\"blackout.set\"") && l.contains("\"value\":true")));
        assert!(lines.iter().any(|l| l.contains("\"tempo.bpm\"") && l.contains("\"bpm\":140.0")));

        client.stop();
    }
}
