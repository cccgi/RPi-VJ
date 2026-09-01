//! Audio capture and analysis.
//!
//! Platform capture backends live behind `AudioCapture`; the analyzer above
//! them is platform-free and testable against WAV fixtures.
//!
//! Two rules that prevent the classic failures:
//!   * dual-rate EMA — symmetric smoothing produces the flicker you are
//!     trying to avoid;
//!   * never normalise a spectrum by its own maximum — silence would look
//!     active. Track a noise floor and an upper envelope instead.

use vj_types::Tier;

pub mod analyzer;
pub mod capture;

pub use analyzer::*;
pub use capture::*;

pub const SPECTRUM_BINS: usize = 256;
pub const WAVEFORM_SAMPLES: usize = 256;

/// Fixed layout. No allocation per update; the render thread reads the newest
/// complete snapshot and never waits for the analyzer.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct AudioFeatures {
    pub sequence: u64,
    pub captured_at_ns: u64,
    pub rms: [f32; 2],
    pub volume: f32,
    pub bass: f32,
    pub mid: f32,
    pub treble: f32,
    pub onset: f32,
    pub beat_trigger: u32,
    pub beat_phase: f32,
    pub bar_phase: f32,
    pub bpm: f32,
    pub beat_confidence: f32,
    pub spectrum_len: u32,
    pub spectrum: [f32; SPECTRUM_BINS],
    pub waveform: [f32; WAVEFORM_SAMPLES],
}

impl Default for AudioFeatures {
    fn default() -> Self {
        Self {
            sequence: 0,
            captured_at_ns: 0,
            rms: [0.0; 2],
            volume: 0.0,
            bass: 0.0,
            mid: 0.0,
            treble: 0.0,
            onset: 0.0,
            beat_trigger: 0,
            beat_phase: 0.0,
            bar_phase: 0.0,
            bpm: 0.0,
            beat_confidence: 0.0,
            spectrum_len: 128,
            spectrum: [0.0; SPECTRUM_BINS],
            waveform: [0.0; WAVEFORM_SAMPLES],
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("no capture device: {0}")]
    NoDevice(String),
    #[error("unsupported format: {0}")]
    Format(String),
    #[error("backend error: {0}")]
    Backend(String),
}

#[derive(Clone, Debug)]
pub struct CaptureInfo {
    pub device: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub quantum: u32,
}

/// The real-time sink signature: `(interleaved samples, frame count)`.
pub type CaptureSink = Box<dyn FnMut(&[f32], u32) + Send>;

/// Implemented by `PipeWireCapture` (Linux), `CpalCapture` (Windows/macOS).
///
/// The sink runs on the real-time callback thread: it may only copy into a
/// preallocated ring. No allocation, no locks, no logging, no I/O.
pub trait AudioCapture: Send {
    fn start(&mut self, sink: CaptureSink) -> Result<(), AudioError>;
    fn stop(&mut self);
    fn info(&self) -> CaptureInfo;
}

/// Rate-independent one-pole smoothing coefficient: `1 - exp(-dt / tau)`.
pub fn ema_alpha(dt_secs: f32, tau_secs: f32) -> f32 {
    if tau_secs <= 0.0 {
        return 1.0;
    }
    1.0 - (-dt_secs / tau_secs).exp()
}

/// Dual-rate envelope follower. Fast attack keeps transients punchy; slow
/// release stops the visual stuttering between frames.
#[derive(Clone, Copy, Debug)]
pub struct Envelope {
    pub value: f32,
    pub tau_attack: f32,
    pub tau_release: f32,
}

impl Envelope {
    pub fn new(tau_attack: f32, tau_release: f32) -> Self {
        Self {
            value: 0.0,
            tau_attack,
            tau_release,
        }
    }
    pub fn update(&mut self, input: f32, dt_secs: f32) -> f32 {
        let tau = if input > self.value {
            self.tau_attack
        } else {
            self.tau_release
        };
        self.value += ema_alpha(dt_secs, tau) * (input - self.value);
        self.value
    }
}

/// Analyzer budget per tier — FFT size stays constant, bin count does not.
pub fn spectrum_bins_for(tier: Tier) -> usize {
    match tier {
        Tier::Appliance => 128,
        Tier::Portable | Tier::Workstation => 256,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_rises_fast_and_falls_slow() {
        let mut e = Envelope::new(0.015, 0.25);
        // Step to 1.0 for 50 ms.
        for _ in 0..5 {
            e.update(1.0, 0.010);
        }
        let risen = e.value;
        assert!(
            risen > 0.9,
            "fast attack should be nearly there in 50 ms, got {risen}"
        );

        // Drop to 0 for the same 50 ms.
        for _ in 0..5 {
            e.update(0.0, 0.010);
        }
        assert!(
            e.value > 0.5,
            "slow release must not collapse in 50 ms, got {}",
            e.value
        );
    }

    #[test]
    fn ema_alpha_is_rate_independent() {
        // One 20 ms step and two 10 ms steps must land in the same place.
        let tau = 0.1;
        let mut a = 0.0_f32;
        a += ema_alpha(0.020, tau) * (1.0 - a);

        let mut b = 0.0_f32;
        for _ in 0..2 {
            b += ema_alpha(0.010, tau) * (1.0 - b);
        }
        assert!((a - b).abs() < 1e-6, "{a} vs {b}");
    }

    #[test]
    fn appliance_tier_uses_fewer_bins() {
        assert_eq!(spectrum_bins_for(Tier::Appliance), 128);
        assert_eq!(spectrum_bins_for(Tier::Workstation), 256);
    }
}
