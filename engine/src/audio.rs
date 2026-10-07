//! Allocation-free analysis after construction. Capture never calls this directly.
use rustfft::{num_complex::Complex, Fft, FftPlanner};
use std::sync::Arc;
pub const FFT_SIZE: usize = 1024;
pub const HOP: usize = 256;
pub const BINS: usize = 128;
pub const SAMPLE_RATE: f32 = 48000.0;

/// Preserve stereo energy even when left and right have opposite polarity.
#[derive(Default)]
pub struct StereoAnalyzer {
    left: Analyzer,
    right: Analyzer,
}
impl StereoAnalyzer {
    pub fn process(&mut self, samples: &[f32; HOP * 2]) -> AudioFrame {
        let left = self.left.process(&std::array::from_fn(|i| samples[i * 2]));
        let right = self
            .right
            .process(&std::array::from_fn(|i| samples[i * 2 + 1]));
        let power = |a: f32, b: f32| ((a * a + b * b) * 0.5).sqrt();
        AudioFrame {
            bass: power(left.bass, right.bass),
            mid: power(left.mid, right.mid),
            treble: power(left.treble, right.treble),
            volume: power(left.volume, right.volume),
            onset: left.onset.max(right.onset),
            sequence: left.sequence,
            spectrum: std::array::from_fn(|i| power(left.spectrum[i], right.spectrum[i])),
        }
    }
}
#[derive(Clone, Debug)]
pub struct AudioFrame {
    pub bass: f32,
    pub mid: f32,
    pub treble: f32,
    pub volume: f32,
    pub onset: f32,
    pub spectrum: [f32; BINS],
    pub sequence: u64,
}
impl Default for AudioFrame {
    fn default() -> Self {
        Self {
            bass: 0.0,
            mid: 0.0,
            treble: 0.0,
            volume: 0.0,
            onset: 0.0,
            spectrum: [0.0; BINS],
            sequence: 0,
        }
    }
}
pub struct Analyzer {
    fft: Arc<dyn Fft<f32>>,
    scratch: Vec<Complex<f32>>,
    window: [f32; FFT_SIZE],
    history: [f32; FFT_SIZE],
    work: [Complex<f32>; FFT_SIZE],
    previous: [f32; FFT_SIZE / 2],
    frame: AudioFrame,
    flux_mean: f32,
    refractory: usize,
}
impl Default for Analyzer {
    fn default() -> Self {
        Self::new()
    }
}
impl Analyzer {
    pub fn new() -> Self {
        let fft = FftPlanner::new().plan_fft_forward(FFT_SIZE);
        Self {
            scratch: vec![Complex::default(); fft.get_inplace_scratch_len()],
            fft,
            window: std::array::from_fn(|i| {
                0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / FFT_SIZE as f32).cos()
            }),
            history: [0.0; FFT_SIZE],
            work: [Complex::default(); FFT_SIZE],
            previous: [0.0; FFT_SIZE / 2],
            frame: AudioFrame::default(),
            flux_mean: 0.0,
            refractory: 0,
        }
    }
    pub fn process(&mut self, samples: &[f32; HOP]) -> AudioFrame {
        self.history.copy_within(HOP.., 0);
        for (out, input) in self.history[FFT_SIZE - HOP..].iter_mut().zip(samples) {
            *out = if input.is_finite() {
                input.clamp(-1.0, 1.0)
            } else {
                0.0
            };
        }
        let mean = self.history.iter().sum::<f32>() / FFT_SIZE as f32;
        for (i, value) in self.work.iter_mut().enumerate() {
            *value = Complex::new((self.history[i] - mean) * self.window[i], 0.0);
        }
        self.fft
            .process_with_scratch(&mut self.work, &mut self.scratch);
        let mut bands = [0.0_f32; 3];
        let mut spectrum = [0.0_f32; BINS];
        let mut flux = 0.0;
        for i in 1..FFT_SIZE / 2 {
            let frequency = i as f32 * SAMPLE_RATE / FFT_SIZE as f32;
            let magnitude = self.work[i].norm() * 4.0 / FFT_SIZE as f32;
            if (40.0..=8000.0).contains(&frequency) {
                flux += (magnitude - self.previous[i]).max(0.0);
            }
            self.previous[i] = magnitude;
            if (30.0..=16000.0).contains(&frequency) {
                let band = if frequency < 250.0 {
                    0
                } else if frequency < 4000.0 {
                    1
                } else {
                    2
                };
                bands[band] += magnitude * magnitude;
                let bin =
                    ((frequency / 30.0).ln() / (16000.0_f32 / 30.0).ln() * BINS as f32) as usize;
                spectrum[bin.min(BINS - 1)] = spectrum[bin.min(BINS - 1)].max(magnitude);
            }
        }
        let rms =
            (self.history.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / FFT_SIZE as f32).sqrt();
        let onset = rms > 0.003 && flux > (self.flux_mean * 1.8).max(0.08) && self.refractory == 0;
        self.flux_mean += 0.025 * (flux - self.flux_mean);
        self.refractory = if onset {
            18
        } else {
            self.refractory.saturating_sub(1)
        };
        self.frame.onset = if onset { 1.0 } else { self.frame.onset * 0.90 };
        self.frame.bass = smooth(self.frame.bass, bands[0].sqrt());
        self.frame.mid = smooth(self.frame.mid, bands[1].sqrt());
        self.frame.treble = smooth(self.frame.treble, bands[2].sqrt());
        self.frame.volume = smooth(self.frame.volume, rms);
        for (old, new) in self.frame.spectrum.iter_mut().zip(spectrum) {
            *old = smooth(*old, new);
        }
        self.frame.sequence += 1;
        self.frame.clone()
    }
}
fn smooth(previous: f32, next: f32) -> f32 {
    let tau = if next > previous { 0.010 } else { 0.080 };
    let alpha = 1.0 - (-(HOP as f32 / SAMPLE_RATE) / tau).exp();
    (previous + alpha * (next - previous)).clamp(0.0, 1.0)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn antiphase_stereo_preserves_energy() {
        let mut analyzer = StereoAnalyzer::default();
        let mut frame = AudioFrame::default();
        for hop in 0..80 {
            let samples = std::array::from_fn(|i| {
                let sample = 0.5
                    * (std::f32::consts::TAU * 100.0 * (hop * HOP + i / 2) as f32 / SAMPLE_RATE)
                        .sin();
                if i % 2 == 0 {
                    sample
                } else {
                    -sample
                }
            });
            frame = analyzer.process(&samples);
        }
        assert!(frame.bass > 0.4);
        assert!(frame.volume > 0.3);
    }
    #[test]
    fn silence_and_nonfinite_input_are_safe() {
        let mut analyzer = Analyzer::new();
        for _ in 0..20 {
            let frame = analyzer.process(&[f32::NAN; HOP]);
            assert_eq!(frame.volume, 0.0);
            assert_eq!(frame.onset, 0.0);
            assert!(frame.spectrum.iter().all(|x| *x == 0.0));
        }
    }
    #[test]
    fn tones_land_in_correct_bands_and_decay() {
        for (frequency, expected) in [(100.0, 0), (1000.0, 1), (6000.0, 2)] {
            let mut analyzer = Analyzer::new();
            let mut frame = AudioFrame::default();
            for hop in 0..80 {
                frame = analyzer.process(&std::array::from_fn(|i| {
                    0.5 * (std::f32::consts::TAU * frequency * (hop * HOP + i) as f32 / SAMPLE_RATE)
                        .sin()
                }));
            }
            let bands = [frame.bass, frame.mid, frame.treble];
            assert!(bands[expected] > 0.4);
            for (i, value) in bands.iter().enumerate() {
                if i != expected {
                    assert!(*value < 0.02);
                }
            }
            for _ in 0..200 {
                frame = analyzer.process(&[0.0; HOP]);
            }
            assert!(frame.volume < 0.001);
        }
    }
}


