//! Engine command plane: commands, events, state, and the reducer.
//!
//! Pure logic. No devices, no GPU, no I/O — so the whole command surface is
//! testable on any machine.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use vj_types::{
    BlendMode, ClipId, DeckId, ErrorCode, LayerId, ParameterPath, ParameterValue, Quantize,
    RegionId, RequestId, ShaderId, SurfaceId, TempoSource, TierBudget, TransitionSpec,
};

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EngineCommand {
    TriggerClip {
        request_id: RequestId,
        deck: DeckId,
        clip: ClipId,
        transition: TransitionSpec,
        quantize: Quantize,
    },
    /// Promote a clip into a hot decoder slot without showing it.
    CueClip {
        deck: DeckId,
        clip: ClipId,
    },
    StopDeck {
        deck: DeckId,
        transition: TransitionSpec,
    },
    SelectShader {
        request_id: RequestId,
        layer: LayerId,
        shader: ShaderId,
    },
    SetParameter {
        target: ParameterPath,
        value: ParameterValue,
        ramp_ms: u32,
    },
    SetLayerOpacity {
        layer: LayerId,
        value: f32,
        ramp_ms: u32,
    },
    SetBlendMode {
        layer: LayerId,
        mode: BlendMode,
    },
    SetBank {
        surface: SurfaceId,
        region: RegionId,
        bank: u32,
    },
    SetQuantize {
        value: Quantize,
    },
    TapTempo {
        at_ns: u64,
    },
    SetTempoAuthority {
        source: TempoSource,
    },
    Blackout {
        enabled: bool,
    },
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EngineEvent {
    CommandAccepted { request_id: RequestId },
    CommandRejected { request_id: RequestId, code: ErrorCode, message: String },
    /// A decoder slot holds a presentable first frame. Not yet visible.
    ClipPrepared { request_id: RequestId, deck: DeckId, clip: ClipId },
    /// The clip is on screen. `actual_time_ns` is a presentation timestamp,
    /// which is what latency gates measure against.
    ClipActivated { request_id: RequestId, deck: DeckId, clip: ClipId, actual_time_ns: u64 },
    /// The trigger was serviced from a cold clip; latency was degraded.
    ColdTrigger { request_id: RequestId, clip: ClipId, extra_ms: u32 },
    ShaderCompileFailed { shader: ShaderId, message: String },
    TempoAuthorityChanged { from: Option<TempoSource>, to: TempoSource },
    DeviceStatusChanged { surface: SurfaceId, connected: bool },
    StateChanged { revision: u64 },
}

/// Work the reducer wants a worker to perform. The reducer itself performs no
/// I/O; it returns effects.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    PrepareClip { request_id: RequestId, deck: DeckId, clip: ClipId },
    ActivateDeck { request_id: RequestId, deck: DeckId, transition: TransitionSpec },
    ScheduleQuantized { request_id: RequestId, quantize: Quantize },
    LoadShader { layer: LayerId, shader: ShaderId },
    PushParameter { target: ParameterPath, value: ParameterValue, ramp_ms: u32 },
    RefreshFeedback,
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeckSlot {
    pub deck: DeckId,
    pub visible_clip: Option<ClipId>,
    pub prepared_clip: Option<ClipId>,
    pub looping: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayerState {
    pub layer: LayerId,
    pub opacity: f32,
    pub blend: BlendMode,
    pub shader: Option<ShaderId>,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EngineState {
    pub revision: u64,
    pub budget: TierBudget,
    pub decks: Vec<DeckSlot>,
    pub layers: Vec<LayerState>,
    pub parameters: HashMap<String, ParameterValue>,
    /// Scroll offset per (surface, region).
    pub banks: HashMap<(String, String), u32>,
    pub quantize: Quantize,
    pub blackout: bool,
}

impl EngineState {
    pub fn new(budget: TierBudget) -> Self {
        let decks = (0..budget.visible_decks)
            .map(|i| DeckSlot {
                deck: DeckId(i),
                visible_clip: None,
                prepared_clip: None,
                looping: true,
            })
            .collect();
        let layers = (0..budget.shader_layers)
            .map(|i| LayerState {
                layer: LayerId(i),
                opacity: if i == 0 { 1.0 } else { 0.0 },
                blend: BlendMode::Alpha,
                shader: None,
                enabled: i == 0,
            })
            .collect();
        Self {
            revision: 0,
            budget,
            decks,
            layers,
            parameters: HashMap::new(),
            banks: HashMap::new(),
            quantize: Quantize::Off,
            blackout: false,
        }
    }

    pub fn deck_mut(&mut self, id: DeckId) -> Option<&mut DeckSlot> {
        self.decks.iter_mut().find(|d| d.deck == id)
    }

    pub fn layer_mut(&mut self, id: LayerId) -> Option<&mut LayerState> {
        self.layers.iter_mut().find(|l| l.layer == id)
    }
}

// ---------------------------------------------------------------------------
// Reducer
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct Outcome {
    pub events: Vec<EngineEvent>,
    pub effects: Vec<Effect>,
}

pub struct Reducer {
    pub state: EngineState,
}

impl Reducer {
    pub fn new(budget: TierBudget) -> Self {
        Self { state: EngineState::new(budget) }
    }

    fn reject(&self, request_id: RequestId, code: ErrorCode, message: &str) -> Outcome {
        Outcome {
            events: vec![EngineEvent::CommandRejected {
                request_id,
                code,
                message: message.to_owned(),
            }],
            effects: vec![],
        }
    }

    fn bump(&mut self, out: &mut Outcome) {
        self.state.revision += 1;
        out.events.push(EngineEvent::StateChanged { revision: self.state.revision });
        out.effects.push(Effect::RefreshFeedback);
    }

    pub fn apply(&mut self, cmd: EngineCommand) -> Outcome {
        let mut out = Outcome::default();

        match cmd {
            EngineCommand::TriggerClip { request_id, deck, clip, transition, quantize } => {
                if self.state.deck_mut(deck).is_none() {
                    return self.reject(request_id, ErrorCode::UnknownDeck, "no such deck");
                }
                out.events.push(EngineEvent::CommandAccepted { request_id: request_id.clone() });
                // Always preroll immediately, even for a quantized launch —
                // a quantized trigger that misses its boundary because decode
                // started late is worse than an unquantized one.
                out.effects.push(Effect::PrepareClip {
                    request_id: request_id.clone(),
                    deck,
                    clip: clip.clone(),
                });
                match quantize {
                    Quantize::Off => out.effects.push(Effect::ActivateDeck {
                        request_id,
                        deck,
                        transition,
                    }),
                    q => out.effects.push(Effect::ScheduleQuantized { request_id, quantize: q }),
                }
                if let Some(slot) = self.state.deck_mut(deck) {
                    slot.prepared_clip = Some(clip);
                }
                self.bump(&mut out);
            }

            EngineCommand::CueClip { deck, clip } => {
                if let Some(slot) = self.state.deck_mut(deck) {
                    slot.prepared_clip = Some(clip.clone());
                    out.effects.push(Effect::PrepareClip {
                        request_id: RequestId::new("cue"),
                        deck,
                        clip,
                    });
                    self.bump(&mut out);
                }
            }

            EngineCommand::StopDeck { deck, .. } => {
                if let Some(slot) = self.state.deck_mut(deck) {
                    slot.visible_clip = None;
                    self.bump(&mut out);
                }
            }

            EngineCommand::SelectShader { request_id, layer, shader } => {
                if self.state.layer_mut(layer).is_none() {
                    return self.reject(request_id, ErrorCode::UnknownLayer, "no such layer");
                }
                out.events.push(EngineEvent::CommandAccepted { request_id });
                if let Some(l) = self.state.layer_mut(layer) {
                    l.shader = Some(shader.clone());
                }
                out.effects.push(Effect::LoadShader { layer, shader });
                self.bump(&mut out);
            }

            EngineCommand::SetParameter { target, value, ramp_ms } => {
                self.state.parameters.insert(target.0.clone(), value.clone());
                out.effects.push(Effect::PushParameter { target, value, ramp_ms });
                self.bump(&mut out);
            }

            EngineCommand::SetLayerOpacity { layer, value, .. } => {
                if let Some(l) = self.state.layer_mut(layer) {
                    l.opacity = value.clamp(0.0, 1.0);
                    self.bump(&mut out);
                }
            }

            EngineCommand::SetBlendMode { layer, mode } => {
                if let Some(l) = self.state.layer_mut(layer) {
                    l.blend = mode;
                    self.bump(&mut out);
                }
            }

            EngineCommand::SetBank { surface, region, bank } => {
                self.state.banks.insert((surface.0, region.0), bank);
                self.bump(&mut out);
            }

            EngineCommand::SetQuantize { value } => {
                self.state.quantize = value;
                self.bump(&mut out);
            }

            EngineCommand::Blackout { enabled } => {
                self.state.blackout = enabled;
                self.bump(&mut out);
            }

            // Tempo commands are forwarded to the arbiter; the reducer only
            // records that state changed so surfaces repaint.
            EngineCommand::TapTempo { .. } | EngineCommand::SetTempoAuthority { .. } => {
                self.bump(&mut out);
            }
        }

        out
    }
}

// ---------------------------------------------------------------------------
// Parameter coalescing
// ---------------------------------------------------------------------------

/// A bounded command queue that collapses repeated `SetParameter` commands on
/// the same path so a fast-moving encoder cannot grow the queue.
#[derive(Debug)]
pub struct CoalescingQueue {
    capacity: usize,
    items: Vec<EngineCommand>,
    pub dropped: u64,
    pub coalesced: u64,
}

impl CoalescingQueue {
    pub fn new(capacity: usize) -> Self {
        Self { capacity, items: Vec::with_capacity(capacity), dropped: 0, coalesced: 0 }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn push(&mut self, cmd: EngineCommand) {
        if let EngineCommand::SetParameter { target, .. } = &cmd {
            if let Some(existing) = self.items.iter_mut().find(|c| {
                matches!(c, EngineCommand::SetParameter { target: t, .. } if t == target)
            }) {
                *existing = cmd;
                self.coalesced += 1;
                return;
            }
        }
        if self.items.len() >= self.capacity {
            // Never block a real-time producer: drop the oldest non-parameter
            // command and count it.
            self.items.remove(0);
            self.dropped += 1;
        }
        self.items.push(cmd);
    }

    pub fn drain(&mut self) -> Vec<EngineCommand> {
        std::mem::take(&mut self.items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reducer() -> Reducer {
        Reducer::new(TierBudget::appliance())
    }

    #[test]
    fn unquantized_trigger_prepares_then_activates() {
        let mut r = reducer();
        let out = r.apply(EngineCommand::TriggerClip {
            request_id: RequestId::new("r1"),
            deck: DeckId(0),
            clip: ClipId::new("tunnel"),
            transition: TransitionSpec::default(),
            quantize: Quantize::Off,
        });
        assert!(matches!(out.events[0], EngineEvent::CommandAccepted { .. }));
        assert!(matches!(out.effects[0], Effect::PrepareClip { .. }));
        assert!(matches!(out.effects[1], Effect::ActivateDeck { .. }));
    }

    #[test]
    fn quantized_trigger_still_prerolls_immediately() {
        let mut r = reducer();
        let out = r.apply(EngineCommand::TriggerClip {
            request_id: RequestId::new("r2"),
            deck: DeckId(0),
            clip: ClipId::new("tunnel"),
            transition: TransitionSpec::default(),
            quantize: Quantize::Bar,
        });
        // Preroll must not wait for the bar boundary.
        assert!(matches!(out.effects[0], Effect::PrepareClip { .. }));
        assert!(matches!(out.effects[1], Effect::ScheduleQuantized { .. }));
    }

    #[test]
    fn trigger_on_missing_deck_is_rejected_not_panicked() {
        let mut r = reducer();
        let out = r.apply(EngineCommand::TriggerClip {
            request_id: RequestId::new("r3"),
            deck: DeckId(99),
            clip: ClipId::new("x"),
            transition: TransitionSpec::default(),
            quantize: Quantize::Off,
        });
        assert!(matches!(
            out.events[0],
            EngineEvent::CommandRejected { code: ErrorCode::UnknownDeck, .. }
        ));
        assert!(out.effects.is_empty());
    }

    #[test]
    fn appliance_budget_creates_two_decks_and_two_layers() {
        let r = reducer();
        assert_eq!(r.state.decks.len(), 2);
        assert_eq!(r.state.layers.len(), 2);
    }

    #[test]
    fn revision_increments_on_every_accepted_mutation() {
        let mut r = reducer();
        let before = r.state.revision;
        r.apply(EngineCommand::Blackout { enabled: true });
        assert_eq!(r.state.revision, before + 1);
    }

    #[test]
    fn parameter_commands_coalesce_instead_of_growing_the_queue() {
        let mut q = CoalescingQueue::new(8);
        for i in 0..100 {
            q.push(EngineCommand::SetParameter {
                target: ParameterPath::new("fx.a.param.0"),
                value: ParameterValue::Float(i as f32 / 100.0),
                ramp_ms: 0,
            });
        }
        assert_eq!(q.len(), 1, "a sweeping encoder must not grow the queue");
        assert_eq!(q.coalesced, 99);
        assert_eq!(q.dropped, 0);
        let drained = q.drain();
        assert_eq!(
            drained[0],
            EngineCommand::SetParameter {
                target: ParameterPath::new("fx.a.param.0"),
                value: ParameterValue::Float(0.99),
                ramp_ms: 0,
            },
            "the newest value must win"
        );
    }

    #[test]
    fn distinct_parameter_paths_do_not_coalesce_together() {
        let mut q = CoalescingQueue::new(8);
        q.push(EngineCommand::SetParameter {
            target: ParameterPath::new("fx.a.param.0"),
            value: ParameterValue::Float(0.1),
            ramp_ms: 0,
        });
        q.push(EngineCommand::SetParameter {
            target: ParameterPath::new("fx.a.param.1"),
            value: ParameterValue::Float(0.2),
            ramp_ms: 0,
        });
        assert_eq!(q.len(), 2);
    }

    #[test]
    fn saturated_queue_drops_oldest_and_counts_it() {
        let mut q = CoalescingQueue::new(4);
        for i in 0..10 {
            q.push(EngineCommand::CueClip { deck: DeckId(0), clip: ClipId::new(format!("c{i}")) });
        }
        assert_eq!(q.len(), 4);
        assert_eq!(q.dropped, 6);
    }
}
