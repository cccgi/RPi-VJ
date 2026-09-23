//! Operator Live Remote & Telemetry streaming over WebSockets / HTTP.

use serde::{Deserialize, Serialize};
use vj_core::EngineCommand;
use vj_types::{ClipId, DeckId, LayerId, ShaderId};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeckSummary {
    pub deck: DeckId,
    pub visible_clip: Option<ClipId>,
    pub prepared_clip: Option<ClipId>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayerSummary {
    pub layer: LayerId,
    pub opacity: f32,
    pub blend: String,
    pub shader: Option<ShaderId>,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioTelemetry {
    pub volume: f32,
    pub bass: f32,
    pub mid: f32,
    pub treble: f32,
    pub onset: f32,
    pub beat_trigger: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TempoTelemetry {
    pub bpm: f32,
    pub source: String,
    pub beat_phase: f32,
    pub bar_phase: f32,
    pub confidence: f32,
}

/// A lightweight frame snapshot broadcast to operator devices (iPad / phone) at 15–30 Hz.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TelemetrySnapshot {
    pub sequence: u64,
    pub revision: u64,
    pub active_bank: u32,
    pub decks: Vec<DeckSummary>,
    pub layers: Vec<LayerSummary>,
    pub audio: AudioTelemetry,
    pub tempo: TempoTelemetry,
    pub fps: f32,
}

impl TelemetrySnapshot {
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json_str)
    }
}

/// Parse an incoming remote command JSON payload from an operator tablet/phone.
pub fn parse_remote_command(json_str: &str) -> Result<EngineCommand, serde_json::Error> {
    serde_json::from_str(json_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vj_types::Quantize;

    #[test]
    fn telemetry_snapshot_serializes_cleanly() {
        let snapshot = TelemetrySnapshot {
            sequence: 120,
            revision: 4,
            active_bank: 0,
            decks: vec![DeckSummary {
                deck: DeckId(0),
                visible_clip: Some(ClipId::new("tunnel_loop")),
                prepared_clip: None,
            }],
            layers: vec![LayerSummary {
                layer: LayerId(0),
                opacity: 1.0,
                blend: "alpha".into(),
                shader: None,
                enabled: true,
            }],
            audio: AudioTelemetry {
                volume: 0.85,
                bass: 0.92,
                mid: 0.45,
                treble: 0.30,
                onset: 1.0,
                beat_trigger: 1,
            },
            tempo: TempoTelemetry {
                bpm: 128.0,
                source: "link".into(),
                beat_phase: 0.25,
                bar_phase: 0.0625,
                confidence: 1.0,
            },
            fps: 59.94,
        };

        let json = snapshot.to_json().expect("failed to serialize telemetry");
        assert!(json.contains("\"bpm\":128.0"));
        assert!(json.contains("\"fps\":59.94"));

        let deserialized = TelemetrySnapshot::from_json(&json).expect("failed to deserialize");
        assert_eq!(deserialized.sequence, 120);
        assert_eq!(deserialized.audio.beat_trigger, 1);
    }

    #[test]
    fn parses_remote_trigger_command() {
        let json = r#"{
            "type": "trigger_clip",
            "request_id": "remote-btn-1",
            "deck": 0,
            "clip": "synthwave_drop",
            "transition": { "kind": "crossfade", "duration_ms": 500 },
            "quantize": "bar"
        }"#;

        let cmd = parse_remote_command(json).expect("failed to parse trigger command");
        match cmd {
            EngineCommand::TriggerClip { request_id, deck, clip, transition, quantize } => {
                assert_eq!(request_id.as_str(), "remote-btn-1");
                assert_eq!(deck, DeckId(0));
                assert_eq!(clip, ClipId::new("synthwave_drop"));
                assert_eq!(transition.duration_ms, 500);
                assert_eq!(quantize, Quantize::Bar);
            }
            _ => panic!("unexpected command variant"),
        }
    }
}
