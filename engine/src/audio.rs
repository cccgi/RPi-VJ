//! Allocation-free analysis after construction. Capture never calls this directly.
use rustfft::{num_complex::Complex, Fft, FftPlanner};
use std::sync::Arc;
pub const FFT_SIZE: usize = 1024;
pub const HOP: usize = 256;
pub const BINS: usize = 128;
pub const SAMPLE_RATE: f32 = 48000.0;
/// Band edges in Hz. `bass` (30..250), `mid` (250..4000) and `treble` (4000..16000) are
/// the three levels the engine has always published. `sub` is the bottom of `bass`
/// (30..100: the two lowest usable FFT bins, 47 and 94 Hz, where a kick fundamental
/// lands), `low_mid` and `high_mid` split `mid` at 1 kHz. In power, `low_mid` and
/// `high_mid` add up to `mid`, and `sub` is never above `bass`.
pub const BAND_FLOOR_HZ: f32 = 30.0;
pub const SUB_MAX_HZ: f32 = 100.0;
pub const BASS_MAX_HZ: f32 = 250.0;
pub const LOW_MID_MAX_HZ: f32 = 1000.0;
pub const MID_MAX_HZ: f32 = 4000.0;
pub const BAND_CEILING_HZ: f32 = 16000.0;
/// Integrated flow clocks advance by `base + gain * level` per second (`.0`, `.1`).
pub const FLOW_RATE: (f32, f32) = (0.15, 2.0);
pub const SPEC_FLOW_RATE: (f32, f32) = (0.05, 2.0);

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
            sub: power(left.sub, right.sub),
            low_mid: power(left.low_mid, right.low_mid),
            high_mid: power(left.high_mid, right.high_mid),
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
    /// 30..100 Hz, the bottom of `bass`.
    pub sub: f32,
    /// 250..1000 Hz, the lower part of `mid`.
    pub low_mid: f32,
    /// 1000..4000 Hz, the upper part of `mid`.
    pub high_mid: f32,
    pub spectrum: [f32; BINS],
    pub sequence: u64,
}
/// Seconds for the idle sweep to rise from rest to its top and return.
pub const IDLE_PERIOD: f32 = 44.0;
/// Top of the idle sweep: half of full loudness.
pub const IDLE_TOP: f32 = 0.5;
/// Stand-in levels for silence. One loudness value eases from 0 to `IDLE_TOP` and back
/// over `IDLE_PERIOD`, so a shader travels the path it takes when music rises from
/// silence, slowly and only half way. It drives the low bands and the volume; mids and
/// highs get a tenth of it, because in the bank wrapper they turn and shake the picture,
/// which reads as wrong with no music. No onsets. `time` counts from the start of the
/// silence so the sweep begins at rest; `amount` (0..=1) fades it in. The result is added
/// to `frame`, which is silent when this is used.
pub fn idle_audio(frame: &AudioFrame, time: f32, amount: f32) -> AudioFrame {
    let sweep = 0.5 - 0.5 * (time * std::f32::consts::TAU / IDLE_PERIOD).cos();
    let loud = amount * IDLE_TOP * sweep;
    let mut out = frame.clone();
    out.bass += loud;
    out.sub += loud;
    out.low_mid += loud;
    out.volume += loud * 0.6;
    out.mid += loud * 0.1;
    out.high_mid += loud * 0.1;
    out.treble += loud * 0.1;
    let bins = out.spectrum.len() as f32;
    for (index, bin) in out.spectrum.iter_mut().enumerate() {
        let low = 1.0 - index as f32 / bins;
        *bin += loud * low * low;
    }
    out
}
/// Shortest time between two section changes, seconds.
pub const SECTION_MIN_GAP: f32 = 12.0;
/// A section that never changes still ends after this long.
pub const SECTION_MAX_LENGTH: f32 = 60.0;
/// Loudness or one band must rise by this factor against the seconds before, or fall by its
/// inverse, to count as a new section.
pub const SECTION_JUMP: f32 = 1.5;
/// Below this on every level there is no music, only the noise of the input.
const SECTION_SILENCE: f32 = 0.04;
/// Seconds of sound before levels are trusted, after a start or a silence.
const SECTION_SETTLE: f32 = 4.0;
/// Notices where a song changes character: a drop, a breakdown, a chorus, a new track.
/// It follows loudness, bass and the two halves of the mids twice, over about a second
/// and over about eight, and calls it a change when a short view leaves its long one by
/// `SECTION_JUMP`. Mastered music often keeps loudness and bass level throughout, and
/// only the mids show where a part enters or leaves. Sound
/// returning after silence is a change too, and so is `SECTION_MAX_LENGTH` without one.
#[derive(Clone, Debug, Default)]
pub struct SectionDetector {
    fast: [f32; 4],
    slow: [f32; 4],
    since_change: f32,
    heard: f32,
    quiet: f32,
    fresh: bool,
}
impl SectionDetector {
    /// Advance by `dt` seconds with the levels heard now; true when a new section begins.
    pub fn tick(&mut self, dt: f32, audio: &AudioFrame) -> bool {
        let now = [audio.volume, (audio.sub + audio.bass) * 0.5, audio.low_mid, audio.high_mid].map(|v| v.max(0.0));
        self.since_change += dt;
        if now.iter().all(|v| *v < SECTION_SILENCE) {
            self.quiet += dt;
            if self.quiet > 2.0 {
                self.fresh = self.heard > 0.0 || self.fresh;
                self.heard = 0.0;
            }
            if self.quiet > 0.5 {
                return false;
            }
        } else {
            self.quiet = 0.0;
        }
        if self.heard == 0.0 {
            self.fast = now;
        }
        self.heard += dt;
        let (short, long) = (1.0 - (-dt / 1.2).exp(), 1.0 - (-dt / 8.0).exp());
        // While levels settle, the long view just follows the short one.
        let settling = self.heard < SECTION_SETTLE || self.since_change < 3.0;
        for ((fast, slow), now) in self.fast.iter_mut().zip(&mut self.slow).zip(now) {
            *fast += (now - *fast) * short;
            *slow = if settling { *fast } else { *slow + (*fast - *slow) * long };
        }
        if self.since_change < SECTION_MIN_GAP {
            return false;
        }
        let jumped = (0..4).any(|index| {
            let ratio = (self.fast[index] + 0.05) / (self.slow[index] + 0.05);
            !(1.0 / SECTION_JUMP..=SECTION_JUMP).contains(&ratio)
        });
        let new_track = self.fresh && self.heard > 0.5;
        // An overdue change waits for a beat, but not for long.
        let overdue = self.since_change > SECTION_MAX_LENGTH
            && (audio.onset > 0.5 || self.since_change > SECTION_MAX_LENGTH + 2.0);
        let change = new_track || (self.heard >= SECTION_SETTLE && (jumped || overdue));
        if change {
            self.since_change = 0.0;
            self.fresh = false;
        }
        change
    }
}
impl Default for AudioFrame {
    fn default() -> Self {
        Self {
            bass: 0.0,
            mid: 0.0,
            treble: 0.0,
            volume: 0.0,
            onset: 0.0,
            sub: 0.0,
            low_mid: 0.0,
            high_mid: 0.0,
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
        // sub, low-mid, high-mid: accumulated beside the three legacy bands so those keep
        // their exact arithmetic.
        let mut split = [0.0_f32; 3];
        let mut spectrum = [0.0_f32; BINS];
        let mut flux = 0.0;
        for i in 1..FFT_SIZE / 2 {
            let frequency = i as f32 * SAMPLE_RATE / FFT_SIZE as f32;
            let magnitude = self.work[i].norm() * 4.0 / FFT_SIZE as f32;
            if (40.0..=8000.0).contains(&frequency) {
                flux += (magnitude - self.previous[i]).max(0.0);
            }
            self.previous[i] = magnitude;
            if (BAND_FLOOR_HZ..=BAND_CEILING_HZ).contains(&frequency) {
                let band = if frequency < BASS_MAX_HZ {
                    0
                } else if frequency < MID_MAX_HZ {
                    1
                } else {
                    2
                };
                bands[band] += magnitude * magnitude;
                if frequency < SUB_MAX_HZ {
                    split[0] += magnitude * magnitude;
                } else if (BASS_MAX_HZ..LOW_MID_MAX_HZ).contains(&frequency) {
                    split[1] += magnitude * magnitude;
                } else if (LOW_MID_MAX_HZ..MID_MAX_HZ).contains(&frequency) {
                    split[2] += magnitude * magnitude;
                }
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
        self.frame.sub = smooth(self.frame.sub, split[0].sqrt());
        self.frame.low_mid = smooth(self.frame.low_mid, split[1].sqrt());
        self.frame.high_mid = smooth(self.frame.high_mid, split[2].sqrt());
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
/// Beat and bar phase in 0..1 and how far they can be trusted (0 = free-running clock).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tempo {
    pub beat: f32,
    pub bar: f32,
    pub locked: f32,
}
impl Tempo {
    /// Phases of an uncorrected clock at `time` seconds.
    pub fn free(time: f32, bpm: f32, locked: f32) -> Self {
        Self {
            beat: (time * bpm / 60.0).fract(),
            bar: (time * bpm / 240.0).fract(),
            locked,
        }
    }
}
/// Fraction of the phase error taken out per onset (the browser renderer's value).
pub const LOCK_PLL_GAIN: f32 = 0.25;
/// Seconds without an on-beat onset after which `locked` has faded to 0 (browser value).
pub const LOCK_DECAY_S: f32 = 6.0;
/// An onset within this many beats of the grid counts as on the beat.
pub const LOCK_ON_BEAT: f32 = 0.125;
/// An onset at least this far from the grid sits on the half beat and says nothing.
pub const LOCK_HALF_BEAT: f32 = 0.375;
/// The decaying onset level must jump by this much to count as a new onset.
const ONSET_EDGE: f32 = 0.25;
/// Below this agreement the clock is acquiring: every onset pulls the phase.
const LOCK_ACQUIRE_BELOW: f32 = 0.3;

/// Beat clock at the operator's BPM whose phase is pulled onto the detected onsets.
///
/// This is not a tempo tracker: it never estimates BPM. `locked` rises only while
/// onsets keep landing on the beat grid of the BPM the operator set, which holds when
/// that BPM is within about 3% of the music, and fades over `LOCK_DECAY_S` without
/// such onsets. The bar phase counts four beats from wherever the phase settled; no
/// downbeat is detected. Runs on the render thread: a few float operations per frame.
#[derive(Clone, Debug, Default)]
pub struct BeatClock {
    beats: f64,
    agree: f32,
    age: f32,
    previous_onset: f32,
}
impl BeatClock {
    pub fn tick(&mut self, dt: f32, bpm: f32, audio: &AudioFrame) -> Tempo {
        let dt = if dt.is_finite() {
            dt.clamp(0.0, 0.1)
        } else {
            0.0
        };
        let bpm = if bpm.is_finite() {
            bpm.clamp(30.0, 300.0)
        } else {
            120.0
        };
        self.beats = (self.beats + f64::from(dt * bpm / 60.0)).rem_euclid(4.0);
        self.age += dt;
        if self.age >= LOCK_DECAY_S {
            // Lock lost: acquire again from scratch.
            self.age = LOCK_DECAY_S;
            self.agree = 0.0;
        }
        let rising = audio.onset - self.previous_onset > ONSET_EDGE;
        self.previous_onset = audio.onset;
        if rising {
            let error = (self.beats.round() - self.beats) as f32;
            let on_beat = error.abs() <= LOCK_ON_BEAT;
            if on_beat {
                self.agree += LOCK_PLL_GAIN * (1.0 - self.agree);
                self.age = 0.0;
            } else if error.abs() < LOCK_HALF_BEAT {
                self.agree *= 1.0 - LOCK_PLL_GAIN;
            }
            if on_beat || self.agree < LOCK_ACQUIRE_BELOW {
                // Onsets that come with low end (kicks) pull harder than hats.
                let weight = 0.25 + 0.75 * (audio.bass * 2.0).clamp(0.0, 1.0);
                self.beats =
                    (self.beats + f64::from(error * LOCK_PLL_GAIN * weight)).rem_euclid(4.0);
            }
        }
        Tempo {
            beat: (self.beats.fract() as f32).min(0.999_999),
            bar: ((self.beats / 4.0) as f32).min(0.999_999),
            locked: ((self.agree - 0.5) / 0.4).clamp(0.0, 1.0)
                * (1.0 - self.age / LOCK_DECAY_S).clamp(0.0, 1.0),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn tone(frequency: f32, hops: usize) -> AudioFrame {
        let mut analyzer = Analyzer::new();
        let mut frame = AudioFrame::default();
        for hop in 0..hops {
            frame = analyzer.process(&std::array::from_fn(|i| {
                0.5 * (std::f32::consts::TAU * frequency * (hop * HOP + i) as f32 / SAMPLE_RATE)
                    .sin()
            }));
        }
        frame
    }
    #[test]
    fn five_bands_separate_pure_tones() {
        // (tone Hz, which of [sub, bass, low_mid, high_mid, treble] must be loud)
        for (frequency, loud) in [
            (55.0, [true, true, false, false, false]),
            (190.0, [false, true, false, false, false]),
            (500.0, [false, false, true, false, false]),
            (2000.0, [false, false, false, true, false]),
            (8000.0, [false, false, false, false, true]),
        ] {
            let f = tone(frequency, 80);
            let levels = [f.sub, f.bass, f.low_mid, f.high_mid, f.treble];
            for (level, loud) in levels.iter().zip(loud) {
                assert!((0.0..=1.0).contains(level), "{frequency} Hz: {levels:?}");
                if loud {
                    assert!(*level > 0.4, "{frequency} Hz: {levels:?}");
                } else {
                    assert!(*level < 0.05, "{frequency} Hz: {levels:?}");
                }
            }
            // The legacy mid is the power sum of its two halves.
            let halves = (f.low_mid * f.low_mid + f.high_mid * f.high_mid).sqrt();
            assert!((f.mid - halves).abs() < 0.01, "{frequency} Hz");
            assert!(f.sub <= f.bass + 1e-4);
        }
    }
    #[test]
    fn new_bands_decay_in_silence_and_stay_bounded_when_overdriven() {
        let mut analyzer = StereoAnalyzer::default();
        let mut frame = AudioFrame::default();
        // Every band at once, far above full scale.
        for hop in 0..80 {
            frame = analyzer.process(&std::array::from_fn(|i| {
                let t = (hop * HOP + i / 2) as f32 / SAMPLE_RATE;
                [55.0, 500.0, 2000.0, 8000.0]
                    .iter()
                    .map(|f| 4.0 * (std::f32::consts::TAU * f * t).sin())
                    .sum()
            }));
            for v in [
                frame.sub,
                frame.bass,
                frame.low_mid,
                frame.high_mid,
                frame.treble,
            ] {
                assert!((0.0..=1.0).contains(&v));
            }
        }
        assert!(frame.sub > 0.5 && frame.low_mid > 0.5 && frame.high_mid > 0.5);
        for _ in 0..200 {
            frame = analyzer.process(&[0.0; HOP * 2]);
        }
        assert!(frame.sub < 0.001 && frame.low_mid < 0.001 && frame.high_mid < 0.001);
    }
    /// Drive a clock at 60 fps with a kick every `period` seconds, the first at `offset`.
    fn run_clock(
        clock: &mut BeatClock,
        bpm: f32,
        period: f32,
        offset: f32,
        seconds: f32,
    ) -> Vec<Tempo> {
        let dt = 1.0 / 60.0;
        let mut audio = AudioFrame::default();
        let mut next = offset;
        (0..(seconds / dt) as usize)
            .map(|frame| {
                let time = frame as f32 * dt;
                audio.onset *= 0.9_f32.powi(3);
                audio.bass *= 0.8;
                if time >= next {
                    next += period;
                    audio.onset = 1.0;
                    audio.bass = 0.9;
                }
                clock.tick(dt, bpm, &audio)
            })
            .collect()
    }
    /// Times at which the detector reports a change, for `(seconds, volume, bass, mids)` parts
    /// played one after another with a kick on every beat at 128 BPM.
    fn section_changes(parts: &[(f32, f32, f32, f32)]) -> Vec<f32> {
        let (dt, beat) = (1.0 / 60.0, 60.0 / 128.0);
        let mut detector = SectionDetector::default();
        let (mut time, mut changes) = (0.0f32, Vec::new());
        for &(length, volume, bass, mids) in parts {
            let end = time + length;
            while time < end {
                let kick = (1.0 - (time / beat).fract()).powi(3);
                let audio = AudioFrame {
                    volume: volume * (0.6 + 0.4 * kick),
                    bass: bass * kick,
                    sub: bass * kick,
                    low_mid: mids,
                    high_mid: mids,
                    onset: if (time / beat).fract() < dt / beat { 1.0 } else { 0.0 },
                    ..AudioFrame::default()
                };
                if detector.tick(dt, &audio) {
                    changes.push(time);
                }
                time += dt;
            }
        }
        changes
    }
    #[test]
    fn a_section_change_is_a_clear_move_in_loudness_or_bass() {
        // Verse into a drop, a breakdown that only loses the bass, then one where only
        // the mids thin out.
        let changes = section_changes(&[(20.0, 0.3, 0.3, 0.4), (20.0, 0.8, 0.9, 0.4), (20.0, 0.8, 0.05, 0.4), (20.0, 0.8, 0.05, 0.18)]);
        assert_eq!(changes.len(), 3, "{changes:?}");
        for (change, start) in changes.iter().zip([20.0, 40.0, 60.0]) {
            assert!((start..start + 2.5).contains(change), "{changes:?}");
        }
        // A steady groove is one section until it has run too long.
        let changes = section_changes(&[(SECTION_MAX_LENGTH + 10.0, 0.6, 0.7, 0.4)]);
        assert_eq!(changes.len(), 1, "{changes:?}");
        assert!(changes[0] > SECTION_MAX_LENGTH, "{changes:?}");
    }
    #[test]
    fn silence_changes_nothing_and_a_new_track_starts_a_section() {
        // The silence is the hiss of an open input, not zero.
        let changes = section_changes(&[(15.0, 0.6, 0.7, 0.4), (90.0, 0.02, 0.03, 0.01), (15.0, 0.6, 0.7, 0.4)]);
        assert_eq!(changes.len(), 1, "{changes:?}");
        assert!((105.0..106.5).contains(&changes[0]), "{changes:?}");
    }
    #[test]
    fn beat_clock_locks_onto_kicks_at_the_set_tempo_from_any_phase() {
        let beat = 60.0 / 128.0;
        for offset in [0.0, 0.1, 0.2, 0.3, 0.4] {
            let mut clock = BeatClock::default();
            let run = run_clock(&mut clock, 128.0, beat, offset, 20.0);
            assert!(run.iter().all(|t| (0.0..=1.0).contains(&t.locked)
                && (0.0..1.0).contains(&t.beat)
                && (0.0..1.0).contains(&t.bar)));
            assert_eq!(run[0].locked, 0.0);
            let last = run.last().unwrap();
            assert!(last.locked > 0.9, "offset {offset}: {last:?}");
            // The beat phase must wrap where the kicks are.
            let since_kick = ((20.0 - offset) / beat).fract();
            let error = (last.beat - since_kick + 0.5).rem_euclid(1.0) - 0.5;
            assert!(error.abs() < 0.1, "offset {offset}: phase error {error}");
        }
    }
    #[test]
    fn beat_clock_stays_unlocked_at_the_wrong_tempo_and_fades_in_silence() {
        // Music at 140 BPM against a clock set to 128.
        let mut clock = BeatClock::default();
        let run = run_clock(&mut clock, 128.0, 60.0 / 140.0, 0.0, 30.0);
        let worst = run.iter().map(|t| t.locked).fold(0.0, f32::max);
        assert!(worst < 0.3, "locked reached {worst}");
        // A locked clock fades within the decay time once the kicks stop.
        let mut clock = BeatClock::default();
        run_clock(&mut clock, 128.0, 60.0 / 128.0, 0.0, 20.0);
        let silent = AudioFrame::default();
        let mut locked = 1.0;
        for _ in 0..(LOCK_DECAY_S * 60.0) as usize + 2 {
            locked = clock.tick(1.0 / 60.0, 128.0, &silent).locked;
        }
        assert_eq!(locked, 0.0);
        // Nonsense input never produces nonsense output.
        let t = clock.tick(f32::NAN, f32::INFINITY, &silent);
        assert!(t.beat.is_finite() && t.bar.is_finite() && t.locked == 0.0);
    }
    #[test]
    fn idle_audio_sweeps_slowly_from_rest_to_half_and_back() {
        let silent = AudioFrame::default();
        assert_eq!(idle_audio(&silent, 3.0, 0.0).bass, 0.0);
        // Starts at rest, peaks at half loudness mid-period, returns to rest.
        assert!(idle_audio(&silent, 0.0, 1.0).bass.abs() < 1e-6);
        assert!((idle_audio(&silent, IDLE_PERIOD / 2.0, 1.0).bass - IDLE_TOP).abs() < 1e-4);
        assert!(idle_audio(&silent, IDLE_PERIOD, 1.0).bass < 1e-4);
        let mut previous = idle_audio(&silent, 0.0, 1.0);
        for step in 1..(IDLE_PERIOD as usize * 60) {
            let frame = idle_audio(&silent, step as f32 / 60.0, 1.0);
            assert!((0.0..=IDLE_TOP + 1e-4).contains(&frame.bass));
            assert_eq!(frame.onset, 0.0);
            // The bands that turn and shake the picture stay nearly at rest.
            assert!(frame.mid <= 0.051 && frame.treble <= 0.051);
            // Ken Burns pace: under a thousandth of full scale per frame.
            assert!((frame.bass - previous.bass).abs() < 0.001);
            previous = frame;
        }
    }
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


