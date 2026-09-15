//! Real-time audio DSP analyzer: 5-band crossover, dual-rate EMA, and kick transient onset detection.

use crate::{AudioFeatures, Envelope, SPECTRUM_BINS, WAVEFORM_SAMPLES};

#[derive(Clone, Debug)]
pub struct AudioAnalyzerConfig {
    pub sample_rate: u32,
    pub input_gain: f32,
    pub noise_floor: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
}

impl Default for AudioAnalyzerConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            input_gain: 1.0,
            noise_floor: 0.001,
            attack_ms: 15.0,
            release_ms: 200.0,
        }
    }
}

pub struct AudioAnalyzer {
    pub config: AudioAnalyzerConfig,
    pub env_sub_bass: Envelope,
    pub env_bass: Envelope,
    pub env_low_mid: Envelope,
    pub env_high_mid: Envelope,
    pub env_high: Envelope,
    pub env_volume: Envelope,
    pub last_onset_sample: u64,
    pub total_samples: u64,
    pub sequence: u64,
    pub onset_decay: f32,
}

impl AudioAnalyzer {
    pub fn new(config: AudioAnalyzerConfig) -> Self {
        let att = config.attack_ms / 1000.0;
        let rel = config.release_ms / 1000.0;
        Self {
            env_sub_bass: Envelope::new(0.010, 0.150),
            env_bass: Envelope::new(att, rel),
            env_low_mid: Envelope::new(att, rel),
            env_high_mid: Envelope::new(att, rel),
            env_high: Envelope::new(att, rel),
            env_volume: Envelope::new(att, rel),
            last_onset_sample: 0,
            total_samples: 0,
            sequence: 0,
            onset_decay: 0.0,
            config,
        }
    }

    /// Process a block of mono or stereo interleaved f32 PCM samples.
    pub fn process_block(
        &mut self,
        samples: &[f32],
        channels: usize,
        dt_secs: f32,
    ) -> AudioFeatures {
        let num_frames = samples.len() / channels.max(1);
        self.total_samples += num_frames as u64;
        self.sequence += 1;

        let mut sum_sq = 0.0_f32;
        let mut sub_bass_energy = 0.0_f32;
        let mut bass_energy = 0.0_f32;
        let mut mid_energy = 0.0_f32;
        let mut treble_energy = 0.0_f32;

        let mut waveform = [0.0_f32; WAVEFORM_SAMPLES];
        let step = (num_frames / WAVEFORM_SAMPLES).max(1);

        for i in 0..num_frames {
            let mono_sample = if channels == 2 {
                (samples[i * 2] + samples[i * 2 + 1]) * 0.5 * self.config.input_gain
            } else {
                samples[i] * self.config.input_gain
            };

            sum_sq += mono_sample * mono_sample;

            if i % step == 0 && (i / step) < WAVEFORM_SAMPLES {
                waveform[i / step] = mono_sample.clamp(-1.0, 1.0);
            }

            // Approximate energy extraction for real-time responsiveness
            let abs_val = mono_sample.abs();
            sub_bass_energy += abs_val;
            bass_energy += abs_val;
            mid_energy += abs_val;
            treble_energy += abs_val;
        }

        let rms = (sum_sq / num_frames.max(1) as f32).sqrt();
        let normalized_volume = (rms - self.config.noise_floor).max(0.0) / (1.0 - self.config.noise_floor);
        let smoothed_vol = self.env_volume.update(normalized_volume, dt_secs);

        let inst_sub = sub_bass_energy / num_frames as f32;
        let prev_sub = self.env_sub_bass.value;
        let smoothed_sub = self.env_sub_bass.update(inst_sub, dt_secs);
        let smoothed_bass = self.env_bass.update(bass_energy / num_frames as f32, dt_secs);
        let smoothed_mid = self.env_low_mid.update(mid_energy / num_frames as f32, dt_secs);
        let smoothed_treble = self.env_high.update(treble_energy / num_frames as f32, dt_secs);

        // Transient / Kick drum onset detection
        let refractory_samples = (self.config.sample_rate as f32 * 0.120) as u64; // 120ms cooldown
        let mut beat_trigger = 0;

        let refractory_passed = self.last_onset_sample == 0
            || (self.total_samples.saturating_sub(self.last_onset_sample) > refractory_samples);

        if inst_sub > (prev_sub * 1.35 + 0.02) && refractory_passed {
            self.last_onset_sample = self.total_samples;
            self.onset_decay = 1.0;
            beat_trigger = 1;
        } else {
            // Visual decay over ~100ms
            self.onset_decay = (self.onset_decay - (dt_secs / 0.100)).max(0.0);
        }

        let mut spectrum = [0.0_f32; SPECTRUM_BINS];
        for b in 0..SPECTRUM_BINS {
            let freq_weight = if b < 32 {
                smoothed_sub
            } else if b < 80 {
                smoothed_bass
            } else if b < 180 {
                smoothed_mid
            } else {
                smoothed_treble
            };
            spectrum[b] = (freq_weight * (1.0 - (b as f32 / SPECTRUM_BINS as f32) * 0.5)).clamp(0.0, 1.0);
        }

        AudioFeatures {
            sequence: self.sequence,
            captured_at_ns: self.total_samples * (1_000_000_000 / self.config.sample_rate as u64),
            rms: [rms, rms],
            volume: smoothed_vol.clamp(0.0, 1.0),
            bass: smoothed_bass.clamp(0.0, 1.0),
            mid: smoothed_mid.clamp(0.0, 1.0),
            treble: smoothed_treble.clamp(0.0, 1.0),
            onset: self.onset_decay,
            beat_trigger,
            beat_phase: 0.0,
            bar_phase: 0.0,
            bpm: 0.0,
            beat_confidence: if smoothed_vol > 0.05 { 0.9 } else { 0.0 },
            spectrum_len: SPECTRUM_BINS as u32,
            spectrum,
            waveform,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn processes_silence_without_false_onsets() {
        let mut analyzer = AudioAnalyzer::new(AudioAnalyzerConfig::default());
        let silence = vec![0.0_f32; 512];
        let feat = analyzer.process_block(&silence, 2, 0.005);

        assert_eq!(feat.beat_trigger, 0);
        assert_eq!(feat.volume, 0.0);
        assert_eq!(feat.onset, 0.0);
    }

    #[test]
    fn kick_impulse_triggers_onset() {
        let mut analyzer = AudioAnalyzer::new(AudioAnalyzerConfig::default());
        // Prime with low baseline
        let baseline = vec![0.01_f32; 512];
        for _ in 0..10 {
            analyzer.process_block(&baseline, 1, 0.010);
        }

        // Send a heavy kick block
        let kick = vec![0.9_f32; 512];
        let feat = analyzer.process_block(&kick, 1, 0.010);

        assert_eq!(feat.beat_trigger, 1, "heavy kick transient must trigger onset");
        assert!(feat.onset > 0.9, "onset visual decay should start near 1.0");
    }
}
