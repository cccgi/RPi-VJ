//! Tempo arbitration and the quantized launch scheduler.
//!
//! One component owns the answer to "what beat is it". See
//! `docs/v2/05-TEMPO-AND-SYNC.md`.

use serde::{Deserialize, Serialize};
use vj_types::{Quantize, TempoSource};

pub mod link;
pub use link::*;

const NS_PER_MIN: f64 = 60_000_000_000.0;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TempoState {
    pub bpm: f32,
    pub beat_phase: f32,
    pub bar_phase: f32,
    pub beats_per_bar: u8,
    pub confidence: f32,
    pub source: TempoSource,
    pub since_ns: u64,
}

impl Default for TempoState {
    fn default() -> Self {
        Self {
            bpm: 0.0,
            beat_phase: 0.0,
            bar_phase: 0.0,
            beats_per_bar: 4,
            confidence: 0.0,
            source: TempoSource::Audio,
            since_ns: 0,
        }
    }
}

impl TempoState {
    pub fn ns_per_beat(&self) -> Option<f64> {
        (self.bpm > 1.0).then(|| NS_PER_MIN / self.bpm as f64)
    }
}

/// A tempo report from one source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TempoReport {
    pub source: TempoSource,
    pub bpm: f32,
    pub beat_phase: f32,
    pub bar_phase: f32,
    pub confidence: f32,
    pub at_ns: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "policy", rename_all = "snake_case")]
pub enum AuthorityPolicy {
    /// First source in the list that is present and above its confidence floor.
    Priority { order: Vec<TempoSource> },
    /// One source, no failover.
    Pinned { source: TempoSource },
}

impl Default for AuthorityPolicy {
    fn default() -> Self {
        AuthorityPolicy::Priority {
            order: vec![
                TempoSource::Manual,
                TempoSource::Link,
                TempoSource::MidiClock,
                TempoSource::Audio,
            ],
        }
    }
}

/// Minimum confidence a source must report to be admitted as authority.
pub fn confidence_floor(source: TempoSource) -> f32 {
    match source {
        TempoSource::Manual => 0.0,
        TempoSource::Link | TempoSource::ProDjLink => 0.0,
        TempoSource::MidiClock => 0.3,
        // Audio-derived tempo is the least trustworthy; hold it to a bar.
        TempoSource::Audio => 0.6,
    }
}

#[derive(Debug)]
pub struct TempoArbiter {
    pub policy: AuthorityPolicy,
    pub state: TempoState,
    reports: Vec<TempoReport>,
    /// A manual tap outranks everything for this long.
    pub manual_hold_ns: u64,
}

impl TempoArbiter {
    pub fn new(policy: AuthorityPolicy) -> Self {
        Self {
            policy,
            state: TempoState::default(),
            reports: Vec::new(),
            manual_hold_ns: 30_000_000_000,
        }
    }

    pub fn report(&mut self, r: TempoReport) {
        if let Some(existing) = self.reports.iter_mut().find(|x| x.source == r.source) {
            *existing = r;
        } else {
            self.reports.push(r);
        }
    }

    /// Drop a source that has gone away (Link peer lost, audio unplugged).
    pub fn withdraw(&mut self, source: TempoSource) {
        self.reports.retain(|r| r.source != source);
    }

    /// Recompute authority. Returns the previous source when it changed, so
    /// the caller can emit `TempoAuthorityChanged`.
    pub fn resolve(&mut self, now_ns: u64, stale_after_ns: u64) -> Option<TempoSource> {
        self.reports
            .retain(|r| now_ns.saturating_sub(r.at_ns) <= stale_after_ns);

        let chosen = match &self.policy {
            AuthorityPolicy::Pinned { source } => {
                self.reports.iter().find(|r| r.source == *source).copied()
            }
            AuthorityPolicy::Priority { order } => order.iter().find_map(|src| {
                self.reports
                    .iter()
                    .find(|r| r.source == *src && r.confidence >= confidence_floor(*src))
                    .copied()
            }),
        };

        let Some(chosen) = chosen else {
            // Nothing available: hold the last known tempo and decay
            // confidence rather than zeroing the BPM.
            self.state.confidence *= 0.9;
            return None;
        };

        let previous = self.state.source;
        let changed = previous != chosen.source && self.state.confidence > 0.0;

        self.state.bpm = chosen.bpm;
        self.state.beat_phase = chosen.beat_phase;
        self.state.bar_phase = chosen.bar_phase;
        self.state.confidence = chosen.confidence;
        self.state.source = chosen.source;
        if changed {
            self.state.since_ns = now_ns;
            return Some(previous);
        }
        None
    }

    /// Nudge BPM by a delta (e.g. +1.0 or -1.0 BPM).
    pub fn nudge_bpm(&mut self, delta: f32) {
        let new_bpm = (self.state.bpm + delta).clamp(40.0, 300.0);
        self.state.bpm = new_bpm;
        self.state.confidence = 1.0;
        self.state.source = TempoSource::Manual;
    }

    /// Align or offset the beat phase (e.g. 0.0 for immediate downbeat snap, 0.5 for half-beat phase flip).
    pub fn sync_phase(&mut self, offset: f32) {
        self.state.beat_phase = offset.rem_euclid(1.0);
        self.state.bar_phase = (offset / self.state.beats_per_bar as f32).rem_euclid(1.0);
        self.state.confidence = 1.0;
    }
}

// ---------------------------------------------------------------------------
// Quantized launch scheduler
// ---------------------------------------------------------------------------

/// Windows that make hardware feel forgiving rather than late.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LaunchWindows {
    /// If the boundary is this close ahead, fire on it.
    pub ahead_ms: u32,
    /// If the boundary just passed within this, fire now — the operator
    /// pressed late and meant this beat.
    pub behind_ms: u32,
}

impl Default for LaunchWindows {
    fn default() -> Self {
        Self { ahead_ms: 40, behind_ms: 80 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LaunchDecision {
    /// Fire immediately.
    Now,
    /// Fire at this monotonic timestamp.
    At { at_ns: u64 },
}

/// Decide when a quantized trigger should fire.
///
/// `beat_position` is the running beat count including fraction — i.e.
/// `beats_elapsed + beat_phase`.
pub fn schedule(
    quantize: Quantize,
    tempo: &TempoState,
    beat_position: f64,
    now_ns: u64,
    windows: LaunchWindows,
) -> LaunchDecision {
    let Some(grid_beats) = quantize.beats(tempo.beats_per_bar).map(|b| b as f64) else {
        return LaunchDecision::Now;
    };
    let Some(ns_per_beat) = tempo.ns_per_beat() else {
        // No usable tempo: quantization is meaningless, do not stall the show.
        return LaunchDecision::Now;
    };

    let into_grid = beat_position % grid_beats;
    let behind_ns = (into_grid * ns_per_beat) as u64;
    if behind_ns <= windows.behind_ms as u64 * 1_000_000 {
        return LaunchDecision::Now;
    }

    let ahead_beats = grid_beats - into_grid;
    let ahead_ns = (ahead_beats * ns_per_beat) as u64;
    if ahead_ns <= windows.ahead_ms as u64 * 1_000_000 {
        return LaunchDecision::Now;
    }

    LaunchDecision::At { at_ns: now_ns + ahead_ns }
}

// ---------------------------------------------------------------------------
// Tempo-synced LFO
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LfoShape {
    Sine,
    Triangle,
    Saw,
    Ramp,
    Square,
    SampleAndHold,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Lfo {
    pub shape: LfoShape,
    /// Cycle length in bars when tempo-synced.
    pub bars: f32,
    pub phase_offset: f32,
    pub depth: f32,
}

impl Lfo {
    /// Evaluate against a running bar position, returning 0..1.
    pub fn value(&self, bar_position: f64, seed: f32) -> f32 {
        let bars = self.bars.max(0.001) as f64;
        let p = ((bar_position / bars) + self.phase_offset as f64).rem_euclid(1.0) as f32;
        let raw = match self.shape {
            LfoShape::Sine => 0.5 - 0.5 * (p * std::f32::consts::TAU).cos(),
            LfoShape::Triangle => 1.0 - (2.0 * p - 1.0).abs(),
            LfoShape::Saw => 1.0 - p,
            LfoShape::Ramp => p,
            LfoShape::Square => {
                if p < 0.5 {
                    1.0
                } else {
                    0.0
                }
            }
            LfoShape::SampleAndHold => {
                // Deterministic per-cycle value so visuals are reproducible.
                let cycle = (bar_position / bars).floor() as f32;
                ((cycle * 12.9898 + seed * 78.233).sin() * 43758.547).fract().abs()
            }
        };
        (raw * self.depth).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempo(bpm: f32) -> TempoState {
        TempoState { bpm, beats_per_bar: 4, confidence: 1.0, ..Default::default() }
    }

    #[test]
    fn quantize_off_fires_immediately() {
        let d = schedule(Quantize::Off, &tempo(128.0), 1.37, 0, LaunchWindows::default());
        assert_eq!(d, LaunchDecision::Now);
    }

    #[test]
    fn no_usable_tempo_does_not_stall_the_show() {
        let d = schedule(Quantize::Bar, &tempo(0.0), 0.0, 0, LaunchWindows::default());
        assert_eq!(d, LaunchDecision::Now);
    }

    #[test]
    fn pressing_just_after_the_boundary_fires_now() {
        // 128 BPM: one beat is 468.75 ms. 0.1 beats past the bar is ~47 ms,
        // inside the 80 ms "just missed" window.
        let d = schedule(Quantize::Bar, &tempo(128.0), 8.1, 0, LaunchWindows::default());
        assert_eq!(d, LaunchDecision::Now, "a late press should still land on this bar");
    }

    #[test]
    fn pressing_mid_bar_schedules_the_next_boundary() {
        let t = tempo(120.0); // 500 ms per beat, 2 s per bar
        let d = schedule(Quantize::Bar, &t, 9.0, 1_000_000_000, LaunchWindows::default());
        match d {
            LaunchDecision::At { at_ns } => {
                // 3 beats remain in the bar = 1.5 s.
                assert_eq!(at_ns, 1_000_000_000 + 1_500_000_000);
            }
            other => panic!("expected a scheduled launch, got {other:?}"),
        }
    }

    #[test]
    fn pressing_just_before_the_boundary_fires_now() {
        let t = tempo(120.0);
        // 0.05 beats before the bar = 25 ms, inside the 40 ms lookahead.
        let d = schedule(Quantize::Bar, &t, 11.95, 0, LaunchWindows::default());
        assert_eq!(d, LaunchDecision::Now);
    }

    #[test]
    fn link_outranks_audio_under_the_default_policy() {
        let mut a = TempoArbiter::new(AuthorityPolicy::default());
        a.report(TempoReport {
            source: TempoSource::Audio,
            bpm: 127.4,
            beat_phase: 0.2,
            bar_phase: 0.3,
            confidence: 0.9,
            at_ns: 100,
        });
        a.report(TempoReport {
            source: TempoSource::Link,
            bpm: 128.0,
            beat_phase: 0.25,
            bar_phase: 0.31,
            confidence: 1.0,
            at_ns: 100,
        });
        a.resolve(100, 1_000_000_000);
        assert_eq!(a.state.source, TempoSource::Link);
        assert_eq!(a.state.bpm, 128.0);
    }

    #[test]
    fn low_confidence_audio_is_not_admitted() {
        let mut a = TempoArbiter::new(AuthorityPolicy::default());
        a.report(TempoReport {
            source: TempoSource::Audio,
            bpm: 90.0,
            beat_phase: 0.0,
            bar_phase: 0.0,
            confidence: 0.4, // below the 0.6 floor
            at_ns: 0,
        });
        a.resolve(0, 1_000_000_000);
        assert_eq!(a.state.bpm, 0.0, "a shaky audio estimate must not become authority");
    }

    #[test]
    fn losing_every_source_decays_confidence_instead_of_zeroing_bpm() {
        let mut a = TempoArbiter::new(AuthorityPolicy::default());
        a.report(TempoReport {
            source: TempoSource::Link,
            bpm: 128.0,
            beat_phase: 0.0,
            bar_phase: 0.0,
            confidence: 1.0,
            at_ns: 0,
        });
        a.resolve(0, 1_000_000_000);
        a.withdraw(TempoSource::Link);
        a.resolve(0, 1_000_000_000);
        assert_eq!(a.state.bpm, 128.0, "BPM is held");
        assert!(a.state.confidence < 1.0, "confidence decays");
    }

    #[test]
    fn authority_change_is_reported_once() {
        let mut a = TempoArbiter::new(AuthorityPolicy::default());
        a.report(TempoReport {
            source: TempoSource::Audio,
            bpm: 127.0,
            beat_phase: 0.0,
            bar_phase: 0.0,
            confidence: 0.9,
            at_ns: 0,
        });
        assert_eq!(a.resolve(0, 1_000_000_000), None);
        a.report(TempoReport {
            source: TempoSource::Link,
            bpm: 128.0,
            beat_phase: 0.0,
            bar_phase: 0.0,
            confidence: 1.0,
            at_ns: 0,
        });
        assert_eq!(a.resolve(0, 1_000_000_000), Some(TempoSource::Audio));
        assert_eq!(a.resolve(0, 1_000_000_000), None, "no repeat report");
    }

    #[test]
    fn lfo_shapes_stay_in_range() {
        for shape in [
            LfoShape::Sine,
            LfoShape::Triangle,
            LfoShape::Saw,
            LfoShape::Ramp,
            LfoShape::Square,
            LfoShape::SampleAndHold,
        ] {
            let lfo = Lfo { shape, bars: 4.0, phase_offset: 0.0, depth: 1.0 };
            for i in 0..200 {
                let v = lfo.value(i as f64 * 0.05, 0.5);
                assert!((0.0..=1.0).contains(&v), "{shape:?} produced {v}");
            }
        }
    }
}
