use crate::auth::PortalAuthManager;
use crate::protocol::{DedupCache, Message, PROTOCOL_VERSION};
use crate::state::StateStore;
use crate::supervisor::Supervisor;
use crate::upload::UploadManager;
use std::io::{BufRead, BufReader, Write};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use vj_storage::ShowStorageManager;

pub struct BrokerContext {
    pub state_store: Mutex<StateStore>,
    pub supervisor: Mutex<Supervisor>,
    pub dedup_cache: Mutex<DedupCache>,
    pub subscribers: Mutex<Vec<Box<dyn Write + Send>>>,
    pub storage_manager: Option<Arc<ShowStorageManager>>,
    pub auth_manager: Option<Arc<Mutex<PortalAuthManager>>>,
    pub upload_manager: Mutex<UploadManager>,
    pub hostname: String,
    pub started_instant: Instant,
}

impl BrokerContext {
    pub fn new(state_store: StateStore, supervisor: Supervisor) -> Self {
        Self {
            state_store: Mutex::new(state_store),
            supervisor: Mutex::new(supervisor),
            dedup_cache: Mutex::new(DedupCache::new(1024)),
            subscribers: Mutex::new(Vec::new()),
            storage_manager: None,
            auth_manager: None,
            upload_manager: Mutex::new(UploadManager::new()),
            hostname: "rpi-vj.local".to_string(),
            started_instant: Instant::now(),
        }
    }

    pub fn with_portal(
        state_store: StateStore,
        supervisor: Supervisor,
        storage_manager: Option<Arc<ShowStorageManager>>,
        auth_manager: Option<Arc<Mutex<PortalAuthManager>>>,
        hostname: Option<String>,
    ) -> Self {
        Self {
            state_store: Mutex::new(state_store),
            supervisor: Mutex::new(supervisor),
            dedup_cache: Mutex::new(DedupCache::new(1024)),
            subscribers: Mutex::new(Vec::new()),
            storage_manager,
            auth_manager,
            upload_manager: Mutex::new(UploadManager::new()),
            hostname: hostname.unwrap_or_else(|| "rpi-vj.local".to_string()),
            started_instant: Instant::now(),
        }
    }

    pub fn handle_message(&self, raw_line: &str) -> Option<Message> {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            return None;
        }

        let parsed: Result<Message, _> = serde_json::from_str(trimmed);
        match parsed {
            Ok(Message::Command {
                v: _,
                id,
                name,
                args,
            }) => {
                // 1. Dedup check
                let mut cache = self.dedup_cache.lock().unwrap();
                if cache.check_and_insert(&id) {
                    // Duplicate! Return accepted ack immediately
                    return Some(Message::ack_accepted(id));
                }
                drop(cache);

                // 2. Apply command to state store
                let mut store = self.state_store.lock().unwrap();
                let (ok, err_msg) = store.apply_command(&name, &args);
                let current_state_json =
                    serde_json::to_value(store.get_state()).unwrap_or_default();
                drop(store);

                // 3. Broadcast state update if mutation succeeded
                if ok {
                    self.broadcast_state(&current_state_json);
                    Some(Message::ack_accepted(id))
                } else {
                    Some(Message::ack_failed(
                        id,
                        err_msg.unwrap_or_else(|| "Command rejected".to_string()),
                    ))
                }
            }
            Ok(Message::Heartbeat {
                v: _,
                source: _,
                timestamp_ms: _,
                frame_count: _,
            }) => {
                let mut sup = self.supervisor.lock().unwrap();
                sup.record_heartbeat(Instant::now());
                None
            }
            Ok(Message::Query { v: _, id: _ }) => {
                let store = self.state_store.lock().unwrap();
                let state_json = serde_json::to_value(store.get_state()).unwrap_or_default();
                Some(Message::State {
                    v: PROTOCOL_VERSION,
                    state: state_json,
                })
            }
            Ok(Message::Subscribe { v: _, id: _ }) => {
                let store = self.state_store.lock().unwrap();
                let state_json = serde_json::to_value(store.get_state()).unwrap_or_default();
                Some(Message::State {
                    v: PROTOCOL_VERSION,
                    state: state_json,
                })
            }
            Ok(_other) => {
                // Return query of state for unknown queries or echo
                None
            }
            Err(e) => Some(Message::ack_failed(
                "error",
                format!("Invalid JSON message: {}", e),
            )),
        }
    }

    pub fn broadcast_state(&self, state_json: &serde_json::Value) {
        let msg = Message::State {
            v: PROTOCOL_VERSION,
            state: state_json.clone(),
        };
        if let Ok(json_str) = serde_json::to_string(&msg) {
            let mut line = json_str;
            line.push('\n');

            let mut subs = self.subscribers.lock().unwrap();
            subs.retain_mut(|writer| {
                writer.write_all(line.as_bytes()).is_ok() && writer.flush().is_ok()
            });
        }
    }
}

pub fn handle_client_stream<R: std::io::Read, W: Write + Send + 'static>(
    reader: R,
    mut writer: W,
    ctx: Arc<BrokerContext>,
) {
    let buf_reader = BufReader::new(reader);

    for line_result in buf_reader.lines() {
        match line_result {
            Ok(line) => {
                if let Some(resp) = ctx.handle_message(&line) {
                    if let Ok(resp_json) = serde_json::to_string(&resp) {
                        let _ = writeln!(writer, "{}", resp_json);
                        let _ = writer.flush();
                    }
                }
            }
            Err(_) => break,
        }
    }
}
