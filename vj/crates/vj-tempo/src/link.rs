//! Ableton Link session tracking, phase calculation, and tempo reporting.

use crate::{TempoReport, TempoSource};

#[derive(Clone, Debug, PartialEq)]
pub struct LinkSession {
    pub enabled: bool,
    pub num_peers: u32,
    pub bpm: f64,
    pub quantum: f64,
    pub beat_time: f64,
}

impl Default for LinkSession {
    fn default() -> Self {
        Self {
            enabled: true,
            num_peers: 0,
            bpm: 128.0,
            quantum: 4.0, // 1 Bar = 4 beats
            beat_time: 0.0,
        }
    }
}

impl LinkSession {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_peers(&mut self, peers: u32) {
        self.num_peers = peers;
    }

    pub fn set_bpm(&mut self, bpm: f64) {
        if bpm > 20.0 && bpm < 400.0 {
            self.bpm = bpm;
        }
    }

    pub fn update_beat_time(&mut self, beat: f64) {
        self.beat_time = beat;
    }

    /// Calculate phase (0.0 .. 1.0) within a specific quantum (e.g. 1.0 for beat, 4.0 for bar).
    pub fn phase(&self, quantum: f64) -> f32 {
        let q = quantum.max(0.001);
        let p = (self.beat_time % q) / q;
        p.rem_euclid(1.0) as f32
    }

    /// Convert current Link state to a `TempoReport` for the `TempoArbiter`.
    pub fn to_tempo_report(&self, at_ns: u64) -> Option<TempoReport> {
        if !self.enabled || self.num_peers == 0 {
            return None;
        }

        Some(TempoReport {
            source: TempoSource::Link,
            bpm: self.bpm as f32,
            beat_phase: self.phase(1.0),
            bar_phase: self.phase(self.quantum),
            confidence: 1.0,
            at_ns,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_phase_calculates_correct_modulo() {
        let mut session = LinkSession::new();
        session.set_bpm(128.0);
        session.set_peers(1);

        session.update_beat_time(0.0);
        assert_eq!(session.phase(1.0), 0.0);
        assert_eq!(session.phase(4.0), 0.0);

        session.update_beat_time(1.5);
        assert_eq!(session.phase(1.0), 0.5);
        assert_eq!(session.phase(4.0), 1.5 / 4.0);

        session.update_beat_time(4.0);
        assert_eq!(session.phase(1.0), 0.0);
        assert_eq!(session.phase(4.0), 0.0);
    }

    #[test]
    fn link_generates_report_only_when_peers_connected() {
        let mut session = LinkSession::new();
        assert_eq!(session.to_tempo_report(100), None, "zero peers should return None");

        session.set_peers(1);
        let report = session.to_tempo_report(100).expect("should produce report with 1 peer");
        assert_eq!(report.source, TempoSource::Link);
        assert_eq!(report.bpm, 128.0);
        assert_eq!(report.confidence, 1.0);
    }
}
