//! Master engine runner: binds audio, video, render, surfaces, and tempo into a unified runtime loop.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use vj_audio::{AudioAnalyzer, AudioAnalyzerConfig, AudioFeatures};
use vj_core::{CoalescingQueue, Effect, EngineCommand, EngineEvent, Outcome, Reducer};
use vj_surface::{ControlEvent, ControllerAdapter, FeedbackCell, FeedbackColor, FeedbackFrame, FeedbackPriority, VirtualSurface};
use vj_tempo::{AuthorityPolicy, LaunchDecision, LaunchWindows, TempoArbiter, TempoReport};
use vj_types::{ClipId, ControlId, DeckId, LayerId, Quantize, ShowManifest, Tier, TierBudget, TransitionSpec};

pub struct EngineRunner {
    pub show: ShowManifest,
    pub tier: Tier,
    pub budget: TierBudget,
    pub reducer: Reducer,
    pub command_queue: CoalescingQueue,
    pub surfaces: VirtualSurface,
    pub adapters: Vec<Box<dyn ControllerAdapter>>,
    pub tempo: TempoArbiter,
    pub analyzer: AudioAnalyzer,
    pub latest_audio: AudioFeatures,
    pub running: Arc<AtomicBool>,
    pub frame_count: u64,
    pub active_bank: u32,
}

impl EngineRunner {
    pub fn new(show: ShowManifest, tier: Tier) -> Self {
        let budget = match tier {
            Tier::Appliance => TierBudget::appliance(),
            Tier::Portable => TierBudget::portable(),
            Tier::Workstation => TierBudget::workstation(),
        };

        let reducer = Reducer::new(budget.clone());
        let command_queue = CoalescingQueue::new(64);
        let surfaces = VirtualSurface::new();
        let tempo = TempoArbiter::new(AuthorityPolicy::default());
        let analyzer = AudioAnalyzer::new(AudioAnalyzerConfig::default());

        Self {
            show,
            tier,
            budget,
            reducer,
            command_queue,
            surfaces,
            adapters: Vec::new(),
            tempo,
            analyzer,
            latest_audio: AudioFeatures::default(),
            running: Arc::new(AtomicBool::new(true)),
            frame_count: 0,
            active_bank: 0,
        }
    }

    /// Register a physical controller adapter (e.g. Stream Deck XL or X-Touch Mini).
    pub fn register_adapter(&mut self, adapter: Box<dyn ControllerAdapter>) {
        self.adapters.push(adapter);
    }

    /// Process incoming audio PCM block and compute live audio-reactive features.
    pub fn tick_audio(&mut self, pcm_samples: &[f32], channels: usize, dt_secs: f32) -> AudioFeatures {
        self.latest_audio = self.analyzer.process_block(pcm_samples, channels, dt_secs);

        if self.latest_audio.beat_confidence > 0.5 && self.latest_audio.bpm > 0.0 {
            self.tempo.report(TempoReport {
                source: vj_types::TempoSource::Audio,
                bpm: self.latest_audio.bpm,
                beat_phase: self.latest_audio.beat_phase,
                bar_phase: self.latest_audio.bar_phase,
                confidence: self.latest_audio.beat_confidence,
                at_ns: self.latest_audio.captured_at_ns,
            });
        }

        self.latest_audio
    }

    /// Poll connected physical surfaces for incoming button presses and encoder movements.
    pub fn poll_controllers(&mut self) -> Vec<ControlEvent> {
        let mut events = Vec::new();
        for adapter in &mut self.adapters {
            if adapter.connected() {
                let _ = adapter.poll(&mut events);
            }
        }
        events
    }

    /// Enqueue an engine command.
    pub fn enqueue(&mut self, cmd: EngineCommand) {
        self.command_queue.push(cmd);
    }

    /// Handle a physical button press on the active bank grid (e.g. Stream Deck XL key).
    pub fn handle_grid_press(&mut self, row: usize, col: usize) {
        if let Some(bank) = self.show.banks.iter().find(|b| b.id == self.active_bank) {
            if let Some(slot) = bank.get_slot(row, col) {
                match slot {
                    vj_types::GridSlot::Clip { clip_id, deck, transition, quantize, .. } => {
                        self.enqueue(EngineCommand::TriggerClip {
                            request_id: vj_types::RequestId::new(format!("pad.{}.{}", row, col)),
                            deck: *deck,
                            clip: clip_id.clone(),
                            transition: *transition,
                            quantize: *quantize,
                        });
                    }
                    vj_types::GridSlot::Shader { shader_id, layer, .. } => {
                        self.enqueue(EngineCommand::SelectShader {
                            request_id: vj_types::RequestId::new(format!("shader.{}.{}", row, col)),
                            layer: *layer,
                            shader: shader_id.clone(),
                        });
                    }
                    vj_types::GridSlot::Action { action, .. } => match action {
                        vj_types::SystemAction::BankNext => {
                            let total = self.show.banks.len().max(1) as u32;
                            self.active_bank = (self.active_bank + 1) % total;
                        }
                        vj_types::SystemAction::BankPrev => {
                            let total = self.show.banks.len() as u32;
                            if self.active_bank > 0 {
                                self.active_bank -= 1;
                            } else if total > 0 {
                                self.active_bank = total - 1;
                            }
                        }
                        vj_types::SystemAction::BankSelect(target_id) => {
                            self.active_bank = *target_id;
                        }
                        vj_types::SystemAction::Blackout => {
                            let current = self.reducer.state.blackout;
                            self.enqueue(EngineCommand::Blackout { enabled: !current });
                        }
                        vj_types::SystemAction::TapTempo => {
                            self.enqueue(EngineCommand::TapTempo {
                                at_ns: self.latest_audio.captured_at_ns,
                            });
                        }
                        vj_types::SystemAction::BpmUp => {
                            self.tempo.nudge_bpm(1.0);
                        }
                        vj_types::SystemAction::BpmDown => {
                            self.tempo.nudge_bpm(-1.0);
                        }
                        vj_types::SystemAction::BeatSync => {
                            self.tempo.sync_phase(0.0);
                        }
                        vj_types::SystemAction::PhaseNudgeHalf => {
                            let curr = self.tempo.state.beat_phase;
                            self.tempo.sync_phase(curr + 0.5);
                        }
                        vj_types::SystemAction::DeckStop(deck) => {
                            self.enqueue(EngineCommand::StopDeck {
                                deck: *deck,
                                transition: TransitionSpec::default(),
                            });
                        }
                        _ => {}
                    },
                }
            }
        }
    }

    /// Execute one frame tick: processes commands, updates reducer state, evaluates effects,
    /// advances tempo, and updates controller LED/screen feedback.
    pub fn step_frame(&mut self, dt_secs: f32) -> Outcome {
        self.frame_count += 1;
        let mut total_outcome = Outcome::default();

        // 1. Drain and apply pending commands
        let commands = self.command_queue.drain();
        for cmd in commands {
            let outcome = self.reducer.apply(cmd);
            for effect in &outcome.effects {
                self.handle_effect(effect);
            }
            total_outcome.events.extend(outcome.events);
            total_outcome.effects.extend(outcome.effects);
        }

        // 2. Resolve tempo authority
        let now_ns = self.frame_count * (dt_secs * 1_000_000_000.0) as u64;
        if let Some(prev_src) = self.tempo.resolve(now_ns, 1_000_000_000) {
            total_outcome.events.push(EngineEvent::TempoAuthorityChanged {
                from: Some(prev_src),
                to: self.tempo.state.source,
            });
        }

        // 3. Emit surface feedback at max feedback rate
        if self.frame_count % 2 == 0 {
            self.update_surface_feedback();
        }

        total_outcome
    }

    fn handle_effect(&mut self, effect: &Effect) {
        match effect {
            Effect::PrepareClip { deck, clip, .. } => {
                if let Some(slot) = self.reducer.state.deck_mut(*deck) {
                    slot.prepared_clip = Some(clip.clone());
                }
            }
            Effect::ActivateDeck { deck, .. } => {
                if let Some(slot) = self.reducer.state.deck_mut(*deck) {
                    slot.visible_clip = slot.prepared_clip.clone();
                }
            }
            Effect::ScheduleQuantized { request_id, quantize } => {
                let decision = vj_tempo::schedule(
                    *quantize,
                    &self.tempo.state,
                    0.0,
                    0,
                    LaunchWindows::default(),
                );
                if decision == LaunchDecision::Now {
                    self.command_queue.push(EngineCommand::TriggerClip {
                        request_id: request_id.clone(),
                        deck: DeckId(0),
                        clip: ClipId::new("cued"),
                        transition: TransitionSpec::default(),
                        quantize: Quantize::Off,
                    });
                }
            }
            Effect::LoadShader { layer, shader } => {
                if let Some(l) = self.reducer.state.layer_mut(*layer) {
                    l.shader = Some(shader.clone());
                }
            }
            _ => {}
        }
    }

    fn update_surface_feedback(&mut self) {
        let mut frame = FeedbackFrame {
            revision: self.reducer.state.revision,
            cells: Vec::new(),
        };

        // Populate Stream Deck button statuses from the active bank
        if let Some(bank) = self.show.banks.iter().find(|b| b.id == self.active_bank) {
            for r in 0..4 {
                for c in 0..8 {
                    let control_id = ControlId::new(format!("pad.{}.{}", r, c));
                    if let Some(slot) = &bank.grid[r][c] {
                        match slot {
                            vj_types::GridSlot::Clip { clip_id, deck, .. } => {
                                let is_active = self.reducer.state.decks.iter().any(|d| {
                                    d.deck == *deck && d.visible_clip.as_ref() == Some(clip_id)
                                });
                                let color = if is_active {
                                    FeedbackColor::solid([0, 255, 128]) // Active green
                                } else {
                                    FeedbackColor::solid([0, 100, 200]) // Cued blue
                                };
                                frame.cells.push((control_id, FeedbackCell::Color(color), FeedbackPriority::Content));
                            }
                            vj_types::GridSlot::Shader { .. } => {
                                frame.cells.push((
                                    control_id,
                                    FeedbackCell::Color(FeedbackColor::solid([255, 42, 109])),
                                    FeedbackPriority::Content,
                                ));
                            }
                            vj_types::GridSlot::Action { color, .. } => {
                                let c = color.unwrap_or([255, 184, 0]);
                                frame.cells.push((
                                    control_id,
                                    FeedbackCell::Color(FeedbackColor::solid(c)),
                                    FeedbackPriority::Critical,
                                ));
                            }
                        }
                    } else {
                        frame.cells.push((control_id, FeedbackCell::Off, FeedbackPriority::Content));
                    }
                }
            }
        }

        // Populate X-Touch Mini layer faders and LED rings
        for (i, layer) in self.reducer.state.layers.iter().enumerate() {
            let enc_id = ControlId::new(format!("enc.{}", i));
            frame.cells.push((
                enc_id,
                FeedbackCell::Ring {
                    value: layer.opacity,
                    mode: vj_surface::RingMode::Single,
                },
                FeedbackPriority::Parameter,
            ));
        }

        for adapter in &mut self.adapters {
            if adapter.connected() {
                let _ = adapter.apply_feedback(&frame);
            }
        }
    }

    /// Shutdown the engine cleanly.
    pub fn shutdown(&mut self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vj_types::{ClipId, DeckId, GridSlot, RequestId, TransitionSpec};

    #[test]
    fn engine_runner_initializes_and_processes_frame() {
        let mut show = ShowManifest::new("Test Show");
        show.banks[0].set_slot(
            0,
            0,
            Some(GridSlot::Clip {
                clip_id: ClipId::new("tunnel"),
                deck: DeckId(0),
                looping: true,
                transition: TransitionSpec::default(),
                quantize: Quantize::Off,
                thumb_path: None,
            }),
        );

        let mut runner = EngineRunner::new(show, Tier::Appliance);
        assert_eq!(runner.reducer.state.decks.len(), 2);

        // Queue a clip trigger
        runner.enqueue(EngineCommand::TriggerClip {
            request_id: RequestId::new("req-1"),
            deck: DeckId(0),
            clip: ClipId::new("tunnel"),
            transition: TransitionSpec::default(),
            quantize: Quantize::Off,
        });

        let outcome = runner.step_frame(0.016);
        assert!(!outcome.events.is_empty(), "trigger must emit events");
        assert_eq!(runner.reducer.state.decks[0].visible_clip, Some(ClipId::new("tunnel")));
    }

    #[test]
    fn audio_tick_updates_features() {
        let show = ShowManifest::new("Audio Test");
        let mut runner = EngineRunner::new(show, Tier::Appliance);

        let pcm = vec![0.8_f32; 512];
        let feat = runner.tick_audio(&pcm, 1, 0.010);

        assert!(feat.volume > 0.0);
        assert_eq!(runner.latest_audio.sequence, 1);
    }
}
