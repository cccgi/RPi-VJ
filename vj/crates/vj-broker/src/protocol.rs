use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AckStatus {
    Accepted,
    Applied,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Message {
    #[serde(rename = "command")]
    Command {
        v: u32,
        id: String,
        name: String,
        #[serde(default)]
        args: serde_json::Value,
    },
    #[serde(rename = "ack")]
    Ack {
        v: u32,
        id: String,
        status: AckStatus,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    #[serde(rename = "state")]
    State {
        v: u32,
        state: serde_json::Value,
    },
    #[serde(rename = "event")]
    Event {
        v: u32,
        name: String,
        data: serde_json::Value,
    },
    #[serde(rename = "heartbeat")]
    Heartbeat {
        v: u32,
        source: String,
        timestamp_ms: u64,
        #[serde(default)]
        frame_count: u64,
    },
    #[serde(rename = "subscribe")]
    Subscribe {
        v: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
    },
    #[serde(rename = "query")]
    Query {
        v: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
    },
}

impl Message {
    pub fn command(id: impl Into<String>, name: impl Into<String>, args: serde_json::Value) -> Self {
        Self::Command {
            v: PROTOCOL_VERSION,
            id: id.into(),
            name: name.into(),
            args,
        }
    }

    pub fn ack_accepted(id: impl Into<String>) -> Self {
        Self::Ack {
            v: PROTOCOL_VERSION,
            id: id.into(),
            status: AckStatus::Accepted,
            detail: None,
        }
    }

    pub fn ack_applied(id: impl Into<String>) -> Self {
        Self::Ack {
            v: PROTOCOL_VERSION,
            id: id.into(),
            status: AckStatus::Applied,
            detail: None,
        }
    }

    pub fn ack_failed(id: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::Ack {
            v: PROTOCOL_VERSION,
            id: id.into(),
            status: AckStatus::Failed,
            detail: Some(detail.into()),
        }
    }

    pub fn state(state_json: serde_json::Value) -> Self {
        Self::State {
            v: PROTOCOL_VERSION,
            state: state_json,
        }
    }

    pub fn heartbeat(source: impl Into<String>, timestamp_ms: u64, frame_count: u64) -> Self {
        Self::Heartbeat {
            v: PROTOCOL_VERSION,
            source: source.into(),
            timestamp_ms,
            frame_count,
        }
    }
}

/// Bounded LRU cache for command deduplication IDs.
#[derive(Debug, Clone)]
pub struct DedupCache {
    capacity: usize,
    entries: VecDeque<String>,
    set: HashSet<String>,
}

impl DedupCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: VecDeque::with_capacity(capacity),
            set: HashSet::with_capacity(capacity),
        }
    }

    /// Check if an ID has been seen. If not seen, record it and return false. If seen, return true.
    pub fn check_and_insert(&mut self, id: &str) -> bool {
        if self.set.contains(id) {
            return true;
        }

        if self.entries.len() >= self.capacity {
            if let Some(oldest) = self.entries.pop_front() {
                self.set.remove(&oldest);
            }
        }

        let id_string = id.to_string();
        self.set.insert(id_string.clone());
        self.entries.push_back(id_string);
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_command_serialization() {
        let cmd = Message::command("test-uuid-1", "clip.trigger", serde_json::json!({"key": 7}));
        let json_str = serde_json::to_string(&cmd).unwrap();
        assert!(json_str.contains("\"v\":1"));
        assert!(json_str.contains("\"type\":\"command\""));
        assert!(json_str.contains("\"name\":\"clip.trigger\""));
        assert!(json_str.contains("\"key\":7"));

        let deserialized: Message = serde_json::from_str(&json_str).unwrap();
        assert_eq!(cmd, deserialized);
    }

    #[test]
    fn test_message_ack_serialization() {
        let ack = Message::ack_accepted("test-uuid-2");
        let json_str = serde_json::to_string(&ack).unwrap();
        assert_eq!(json_str, r#"{"type":"ack","v":1,"id":"test-uuid-2","status":"accepted"}"#);

        let deserialized: Message = serde_json::from_str(&json_str).unwrap();
        assert_eq!(ack, deserialized);
    }

    #[test]
    fn test_dedup_cache() {
        let mut cache = DedupCache::new(3);
        assert!(!cache.check_and_insert("id1"));
        assert!(cache.check_and_insert("id1")); // Duplicate!

        assert!(!cache.check_and_insert("id2"));
        assert!(!cache.check_and_insert("id3"));

        // Capacity reached: inserting id4 should evict id1
        assert!(!cache.check_and_insert("id4"));
        assert!(!cache.check_and_insert("id1")); // id1 was evicted, so now accepted again
    }
}
