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
    tempo: TempoTracker,
}
impl StereoAnalyzer {
    pub fn process(&mut self, samples: &[f32; HOP * 2]) -> AudioFrame {
        let left = self.left.process(&std::array::from_fn(|i| samples[i * 2]));
        let right = self
            .right
            .process(&std::array::from_fn(|i| samples[i * 2 + 1]));
        let power = |a: f32, b: f32| ((a * a + b * b) * 0.5).sqrt();
        let (beat_period, beat_ahead, beat_sure) = self.tempo.push(self.left.flux.max(self.right.flux));
        AudioFrame {
            beat_period,
            beat_ahead,
            beat_sure,
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
    /// From `TempoTracker`: seconds per beat (0 while unknown), seconds from this frame
    /// to the next beat, and how sure that is (0..1).
    pub beat_period: f32,
    pub beat_ahead: f32,
    pub beat_sure: f32,
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
/// The ways shaders can follow the music, in the order the key steps through them.
pub const REACT_MODES: [&str; 6] = ["PUMP", "STROBE", "STEP", "RIDE", "TWIST", "AUTO"];
/// AUTO is not a way of its own: it moves through the others.
const REACT_AUTO: u8 = 5;
/// AUTO moves on after this many beats, or after this many seconds when beats are scarce.
const AUTO_BEATS: u32 = 32;
const AUTO_SECONDS: f32 = 30.0;
/// What AUTO picks from while the music drives, and while it does not.
const AUTO_STRONG: [u8; 3] = [0, 1, 4];
const AUTO_SOFT: [u8; 2] = [3, 2];
/// Share of the zoom and of the turn in each pose that STEP walks through.
const STEP_POSES: [(f32, f32); 4] = [(1.0, 0.2), (0.3, 1.0), (0.7, 0.5), (0.1, 0.8)];
/// Reshapes the levels handed to the shaders, so one shader answers the music in
/// several ways without being changed itself:
/// - PUMP: the levels as heard; every kick shows.
/// - STROBE: the picture only swells gently, and the light does the work: every beat
///   flashes it bright from near dark, washes it with colour and turns its hues.
/// - STEP: each beat snaps to a new pose, which holds until the next beat.
/// - RIDE: a steady travel in and out whose speed and depth follow the energy.
/// - TWIST: bass and mids trade places, so kicks turn the picture and melody pumps it.
/// - AUTO: one of the above at a time, moving to another every `AUTO_BEATS` beats and
///   when the song enters a new section, never to the same one twice in a row. While
///   the music drives (steady kicks with bass under them) it picks from PUMP, STROBE and
///   TWIST; in bridges and wherever kicks or bass are thin, from RIDE and STEP.
#[derive(Clone, Debug, Default)]
pub struct Reactor {
    /// Followers of bass, mid, treble, volume, sub, low mid and high mid.
    slow: [f32; 7],
    held: [f32; 7],
    shown: [f32; 7],
    steps: usize,
    since_beat: f32,
    in_beat: bool,
    phase: f32,
    /// STROBE: the flash fading after a beat, the hue turn it left, and how surely there
    /// is a beat to flash on.
    flash: f32,
    hue: f32,
    beating: f32,
    /// AUTO: the way in use, and the beats and seconds it has had.
    auto: u8,
    auto_beats: u32,
    auto_time: f32,
    /// Beats per second lately, the bass lately, and the bass at its fullest in the last
    /// minutes: together they say whether the music is driving.
    rate: f32,
    low: f32,
    low_top: f32,
    strong: bool,
}
impl Reactor {
    fn resolve(&self, mode: u8) -> u8 {
        if mode == REACT_AUTO { self.auto } else { mode }
    }
    /// AUTO: move to another way, picked unevenly so the order does not read as a loop.
    fn advance(&mut self) {
        let pool: &[u8] = if self.strong { &AUTO_STRONG } else { &AUTO_SOFT };
        let skip = ((self.steps as u32).wrapping_mul(2_654_435_761) >> 16) as usize;
        let others = pool.iter().filter(|mode| **mode != self.auto);
        self.auto = others.clone().nth(skip % others.count()).copied().unwrap_or(pool[0]);
        (self.auto_beats, self.auto_time) = (0, 0.0);
    }
    /// The song entered a new section: AUTO moves on, unless it only just did.
    pub fn new_section(&mut self) {
        if self.auto_time > 5.0 {
            self.advance();
        }
    }
    /// For the compositor: brightness, colour wash, hue turn (0..1), and 1 while STROBE
    /// is on. Without a beat the picture is left as it is rather than held dark.
    pub fn flash(&self, mode: u8) -> [f32; 4] {
        if self.resolve(mode) != 1 {
            return [1.0, 0.0, 0.0, 0.0];
        }
        let lit = 0.25 + 1.5 * self.flash;
        [1.0 + (lit - 1.0) * self.beating, 0.6 * self.flash * self.beating, self.hue, 1.0]
    }
    pub fn apply(&mut self, mode: u8, dt: f32, audio: &AudioFrame) -> AudioFrame {
        let heard = [audio.bass, audio.mid, audio.treble, audio.volume, audio.sub, audio.low_mid, audio.high_mid];
        for (slow, level) in self.slow.iter_mut().zip(heard) {
            let seconds = if level > *slow { 0.5 } else { 1.5 };
            *slow += (level - *slow) * (1.0 - (-dt / seconds).exp());
        }
        // A follower sits well under the peaks it follows; lift it to a comparable size.
        let glide = self.slow.map(|v| (v * 1.6).min(1.0));
        let beat = audio.onset > 0.4 && !self.in_beat;
        self.in_beat = audio.onset > 0.4;
        self.since_beat += dt;
        self.flash *= (-dt / 0.1).exp();
        if self.since_beat > 1.0 {
            self.beating *= (-dt / 0.5).exp();
        }
        if beat {
            self.flash = 1.0;
            self.beating = 1.0;
            // The golden angle: successive beats never land on neighbouring hues.
            self.hue = (self.hue + 0.382).fract();
            self.since_beat = 0.0;
            self.steps += 1;
            let (zoom, turn) = STEP_POSES[self.steps % STEP_POSES.len()];
            let (low, middle) = ((glide[0] * 1.3 * zoom).min(1.0), (glide[1] * 1.3 * turn).min(1.0));
            self.held = [low, middle, glide[2], glide[3], low, middle, middle];
        } else if self.since_beat > 2.0 {
            // No beat to step on: follow the energy instead of freezing.
            self.held = glide;
        }
        for (shown, held) in self.shown.iter_mut().zip(self.held) {
            *shown += (held - *shown) * (1.0 - (-dt / 0.05).exp());
        }
        // Driving music brings beats at a steady rate with the bass near its fullest. Both
        // marks are looser once it drives, so it does not flip on one thin bar.
        self.rate += (f32::from(u8::from(beat)) / dt.max(1e-3) - self.rate) * (1.0 - (-dt / 3.0).exp());
        self.low += (audio.bass - self.low) * (1.0 - (-dt / 2.0).exp());
        self.low_top = self.low.max(self.low_top * (-dt / 90.0).exp());
        let (beats, bass) = if self.strong { (1.0, 0.4) } else { (1.4, 0.55) };
        self.strong = self.rate > beats && self.low > bass * self.low_top.max(0.01);
        if mode == REACT_AUTO {
            self.auto_time += dt;
            self.auto_beats += u32::from(beat);
            let pool: &[u8] = if self.strong { &AUTO_STRONG } else { &AUTO_SOFT };
            if !pool.contains(&self.auto) || (beat && self.auto_beats >= AUTO_BEATS) || self.auto_time > AUTO_SECONDS {
                self.advance();
            }
        }
        let mode = self.resolve(mode);
        let energy = glide[3].max(glide[0]);
        self.phase = (self.phase + dt * (0.05 + 0.45 * energy)).rem_euclid(1000.0);
        let levels = match mode {
            1 => glide,
            2 => self.shown,
            3 => {
                let depth = (0.3 + 0.9 * energy).min(1.0) * energy.min(0.1) * 10.0;
                let wave = |rate: f32| (0.5 - 0.5 * (self.phase * rate * std::f32::consts::TAU).cos()) * depth;
                let (low, middle) = (wave(1.0), wave(0.618));
                [low, middle, glide[2], glide[3], low, middle, middle]
            }
            4 => [heard[1], (heard[0] + 0.5 * audio.onset).min(1.0), heard[2], heard[3], heard[5], heard[4], heard[0]],
            _ => return audio.clone(),
        };
        let mut out = audio.clone();
        [out.bass, out.mid, out.treble, out.volume, out.sub, out.low_mid, out.high_mid] = levels;
        out.onset = 0.0;
        out
    }
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
            beat_period: 0.0,
            beat_ahead: 0.0,
            beat_sure: 0.0,
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
    /// Rise of the spectrum in the last hop: the strength of whatever just began.
    flux: f32,
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
            flux: 0.0,
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
        self.flux = flux;
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
const HOP_SECONDS: f32 = HOP as f32 / SAMPLE_RATE;
/// Hops of onset strength the tempo tracker looks back over: about eight seconds.
const TEMPO_HISTORY: usize = 1536;
/// Hops of history before the first estimate, and hops between estimates.
const TEMPO_WARMUP: usize = 750;
const TEMPO_EVERY: usize = 48;
/// Hops the beats are placed from: about five seconds.
const TEMPO_RECENT: usize = 900;
/// Beat lengths considered, in hops: 181 down to 70 BPM.
const TEMPO_SHORTEST: usize = 62;
const TEMPO_LONGEST: usize = 160;
/// Finds the tempo of the music and where its beats fall, so the next beat is known
/// before it is heard. Every `TEMPO_EVERY` hops it compares the recent onset strength
/// with itself shifted by each beat length (the best match is the tempo, with a
/// preference for tempos near 125 BPM where a half or a double fits as well), then lays
/// a grid of about that length over the last five seconds at every offset (the best fit
/// is the phase, and the length that fits best is the tempo right now, so a tempo being
/// ridden up or down is followed and a cut to another tempo is noticed).
/// `sure` says how far that best offset stands out from the others: steady drums score
/// high, speech and beatless music low. Runs on the audio thread, well under a
/// millisecond per estimate; allocates only when made.
pub struct TempoTracker {
    ring: Vec<f32>,
    at: usize,
    filled: usize,
    line: Vec<f32>,
    match_at: Vec<f32>,
    fit: Vec<f32>,
    /// Beat length over the whole history, in hops.
    rough: f32,
    /// Beat length now and distance to the next beat, in hops.
    period: f32,
    ahead: f32,
    sure: f32,
}
impl Default for TempoTracker {
    fn default() -> Self {
        Self {
            ring: vec![0.0; TEMPO_HISTORY],
            at: 0,
            filled: 0,
            line: vec![0.0; TEMPO_HISTORY],
            match_at: vec![0.0; 2 * TEMPO_LONGEST + 2],
            fit: vec![0.0; TEMPO_LONGEST + 1],
            rough: 0.0,
            period: 0.0,
            ahead: 0.0,
            sure: 0.0,
        }
    }
}
/// Offset of the top of a parabola through three evenly spaced values, from the middle one.
fn peak_offset(before: f32, top: f32, after: f32) -> f32 {
    let bend = before - 2.0 * top + after;
    if bend < 0.0 { (0.5 * (before - after) / bend).clamp(-0.5, 0.5) } else { 0.0 }
}
impl TempoTracker {
    /// Take one hop's onset strength; returns seconds per beat (0 while unknown), seconds
    /// until the next beat, and how sure that is.
    pub fn push(&mut self, flux: f32) -> (f32, f32, f32) {
        // The root keeps one loud hit from outweighing everything else in the history.
        self.ring[self.at] = if flux.is_finite() { flux.max(0.0).sqrt() } else { 0.0 };
        self.at = (self.at + 1) % TEMPO_HISTORY;
        self.filled = (self.filled + 1).min(TEMPO_HISTORY);
        if self.period > 0.0 {
            self.ahead -= 1.0;
            if self.ahead < 0.0 {
                self.ahead += self.period;
            }
        }
        if self.at % TEMPO_EVERY == 0 && self.filled >= TEMPO_WARMUP {
            self.estimate();
        }
        (self.period * HOP_SECONDS, self.ahead * HOP_SECONDS, self.sure)
    }
    fn estimate(&mut self) {
        let n = self.filled;
        for (index, value) in self.line[..n].iter_mut().enumerate() {
            *value = self.ring[(self.at + TEMPO_HISTORY - n + index) % TEMPO_HISTORY];
        }
        let line = &mut self.line[..n];
        let mean = line.iter().sum::<f32>() / n as f32;
        if mean < 1e-4 {
            self.sure *= 0.5;
            return;
        }
        line.iter_mut().for_each(|value| *value -= mean);
        let longest = (2 * TEMPO_LONGEST + 1).min(n / 2);
        for lag in TEMPO_SHORTEST - 1..=longest {
            let sum: f32 = line[lag..].iter().zip(line.iter()).map(|(now, then)| now * then).sum();
            self.match_at[lag] = sum / (n - lag) as f32;
        }
        // A beat length fits when the history matches itself one and two beats back.
        let fit = |lag: usize, match_at: &[f32]| {
            let twice = match_at[2 * lag - 1].max(match_at[2 * lag]).max(match_at[2 * lag + 1]);
            let bpm = 60.0 / (lag as f32 * HOP_SECONDS);
            let near = (-0.5 * ((bpm / 125.0).log2() / 0.5).powi(2)).exp();
            (match_at[lag] + twice) * near
        };
        let (mut best, mut top) = (0, 0.0);
        for lag in TEMPO_SHORTEST..=TEMPO_LONGEST {
            self.fit[lag] += 0.4 * (fit(lag, &self.match_at) - self.fit[lag]);
            if self.fit[lag] > top {
                (best, top) = (lag, self.fit[lag]);
            }
        }
        if best == 0 {
            self.sure *= 0.5;
            return;
        }
        // Stay with the tempo already in use unless another is clearly better.
        let held = self.rough.round() as usize;
        if self.rough > 0.0 && best.abs_diff(held) > 2 {
            let (low, high) = (held.saturating_sub(1).max(TEMPO_SHORTEST), (held + 1).min(TEMPO_LONGEST));
            let stay = (low..=high).max_by(|a, b| self.fit[*a].total_cmp(&self.fit[*b])).unwrap_or(held);
            if top < 1.15 * self.fit[stay] {
                best = stay;
            }
        }
        let near = |lag: usize| if (TEMPO_SHORTEST..=TEMPO_LONGEST).contains(&lag) { self.fit[lag] } else { 0.0 };
        self.rough = best as f32 + peak_offset(near(best - 1), near(best), near(best + 1));
        // That is the tempo of the last eight seconds. Where the beats are now, and how
        // long they are now, comes from the last five alone: lay a beat grid over them
        // at every offset back from the newest hop, for lengths a little either side.
        let recent = n.min(TEMPO_RECENT);
        let (mut top, mut period, mut span) = (f32::MIN, self.rough, 0);
        let mut scores = [0.0f32; TEMPO_LONGEST + 8];
        for step in -6..=6 {
            let length = self.rough * (1.0 + step as f32 * 0.005);
            let mut tried = [0.0f32; TEMPO_LONGEST + 8];
            let offsets = length.ceil() as usize;
            for (offset, score) in tried[..offsets].iter_mut().enumerate() {
                let (mut back, mut beats) = (offset as f32, 0.0);
                while (back.round() as usize) < recent {
                    *score += line[n - 1 - back.round() as usize];
                    back += length;
                    beats += 1.0;
                }
                *score /= beats;
            }
            // A length away from the long view has to fit clearly better to be taken.
            let high = tried[..offsets].iter().copied().fold(f32::MIN, f32::max) * (1.0 - 0.02 * (step as f32).abs());
            if high > top {
                (top, period, span, scores) = (high, length, offsets, tried);
            }
        }
        let scores = &scores[..span];
        let offset = (0..span).max_by(|a, b| scores[*a].total_cmp(&scores[*b])).unwrap_or(0);
        let mean = scores.iter().sum::<f32>() / span as f32;
        let spread = (scores.iter().map(|score| (score - mean).powi(2)).sum::<f32>() / span as f32).sqrt();
        let stands_out = (scores[offset] - mean) / spread.max(1e-9);
        let exact = offset as f32
            + peak_offset(scores[(offset + span - 1) % span], scores[offset], scores[(offset + 1) % span]);
        // A beat that is not where the last estimate said it would be means the tempo or
        // the track has changed: doubt at once, and earn the trust back.
        let ahead = (period - exact).rem_euclid(period);
        let moved = ((ahead - self.ahead) / period + 0.5).rem_euclid(1.0) - 0.5;
        // So does a grid whose two newest beats came with nothing to hear.
        let newest = [exact, exact + period].map(|back| {
            let at = n - 1 - back.round() as usize;
            line[at - 1].max(line[at]).max(line[(at + 1).min(n - 1)])
        });
        let landed = newest[0] + newest[1] > scores[offset];
        let kept = self.period > 0.0 && (period / self.period - 1.0).abs() < 0.02 && moved.abs() < 0.06 && landed;
        (self.period, self.ahead) = (period, ahead);
        if kept {
            self.sure += 0.3 * (((stands_out - 3.5) / 2.0).clamp(0.0, 1.0) - self.sure);
        } else {
            self.sure *= 0.6;
        }
    }
}
/// How far ahead of the heard beat the predicted one is drawn, seconds. It covers what
/// lies between a sound reaching the input and its picture reaching the eye: capture and
/// analysis (about 25 ms), handing the result to the render loop (about 10 ms), rendering
/// and scanning out a frame (about 25 ms), and the delay of the display (unknown; 20 ms
/// assumed).
pub const BEAT_LEAD: f32 = 0.08;
/// Seconds for the predicted swell of the levels to fall away.
const SWELL_FALL: f32 = 0.08;
/// Replaces reactions that follow a beat with reactions that arrive with it. It runs a
/// beat clock on the tempo and phase from `TempoTracker` and fires each beat `lead`
/// seconds early: the onset pulse, and a swell of bass, sub and volume to the height
/// they usually reach on the beat. The heard onset of that same beat, which comes late,
/// is dropped. Hits off the beat are passed on as heard. When the tracker is unsure, or
/// the beats stop bringing onsets (a breakdown), the heard levels pass unchanged.
#[derive(Clone, Debug, Default)]
pub struct BeatPredictor {
    phase: f32,
    shown: f32,
    period: f32,
    seen: u64,
    since_fire: f32,
    pulse: f32,
    /// Onset, bass, sub and volume: highest around the current beat, and usual height.
    peak: [f32; 4],
    usual: [f32; 4],
    past_half: bool,
    /// Whether the tracker is trusted, and how far the hand-over has gone (0..1).
    engaged: bool,
    hand: f32,
    /// How far the tracker's beat moves about between frames, seconds; for the log.
    pub jitter: f32,
}
impl BeatPredictor {
    /// The tempo in use while beats are being predicted, else 0.
    pub fn bpm(&self) -> f32 {
        if self.hand > 0.5 && self.period > 0.0 { 60.0 / self.period } else { 0.0 }
    }
    pub fn apply(&mut self, dt: f32, lead: f32, audio: &AudioFrame) -> AudioFrame {
        if !(audio.beat_period > 0.0 && audio.beat_sure > 0.0) {
            self.period = 0.0;
            self.usual = [0.0; 4];
            return audio.clone();
        }
        let acquired = self.period > 0.0;
        self.period += (audio.beat_period - self.period) * if acquired { 0.2 } else { 1.0 };
        self.phase = (self.phase + dt / self.period).fract();
        if audio.sequence != self.seen {
            self.seen = audio.sequence;
            // Frames reach the render loop at uneven moments; lean on them, do not jump.
            let told = 1.0 - audio.beat_ahead / audio.beat_period;
            let error = (told - self.phase + 0.5).rem_euclid(1.0) - 0.5;
            let pull = if !acquired || error.abs() > 0.2 { 1.0 } else { 0.1 };
            self.jitter += 0.02 * (error.abs() * self.period - self.jitter);
            self.phase = (self.phase + error * pull).rem_euclid(1.0);
        }
        // The heard beat lands at phase 0; around it, note how high the levels go.
        let heard = [audio.onset, audio.bass, audio.sub, audio.volume];
        let near = self.phase < 0.2 || self.phase > 0.88;
        if near {
            for (peak, level) in self.peak.iter_mut().zip(heard) {
                *peak = peak.max(level);
            }
        }
        let past_half = (0.5..0.88).contains(&self.phase);
        if past_half && !self.past_half {
            for (usual, peak) in self.usual.iter_mut().zip(&mut self.peak) {
                *usual += 0.5 * (*peak - *usual);
                *peak = 0.0;
            }
        }
        self.past_half = past_half;
        // Fire on the frame nearest to `lead` before the beat.
        let shown = (self.phase + (lead + 0.5 * dt) / self.period).fract();
        self.since_fire += dt;
        self.pulse *= (-dt / 0.05).exp();
        if shown < self.shown && self.since_fire > 0.5 * self.period {
            self.pulse = 1.0;
            self.since_fire = 0.0;
        }
        self.shown = shown;
        // An onset seen a frame late has already faded a little; count it as whole.
        // Half trust would draw every beat twice, early and late: take over fully or not
        // at all, and do not flicker between the two.
        self.engaged = audio.beat_sure > if self.engaged { 0.25 } else { 0.45 };
        self.hand += (f32::from(u8::from(self.engaged)) - self.hand) * (1.0 - (-dt / 0.4).exp());
        let trust = self.hand * (self.usual[0] * 1.3).min(1.0);
        let swell = if self.since_fire < lead { 1.0 } else { (-(self.since_fire - lead) / SWELL_FALL).exp() };
        let mut out = audio.clone();
        out.onset = (self.pulse * trust).max(audio.onset * if near { 1.0 - trust } else { 1.0 });
        let lift = |heard: f32, usual: f32| heard.max(swell * usual * trust).min(1.0);
        out.bass = lift(audio.bass, self.usual[1]);
        out.sub = lift(audio.sub, self.usual[2]);
        out.volume = lift(audio.volume, self.usual[3]);
        out
    }
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
    /// Each mode's bass over ten seconds of kicks at 128 BPM, with the onsets heard.
    fn react(mode: u8) -> Vec<(f32, f32)> {
        let (dt, beat) = (1.0 / 60.0, 60.0 / 128.0);
        let mut reactor = Reactor::default();
        (0..600)
            .map(|frame| {
                let since = (frame as f32 * dt / beat).fract();
                let kick = (1.0 - since).powi(3);
                let audio = AudioFrame {
                    volume: 0.4 + 0.3 * kick,
                    bass: 0.9 * kick,
                    sub: 0.9 * kick,
                    mid: 0.3,
                    low_mid: 0.3,
                    high_mid: 0.3,
                    treble: 0.1,
                    onset: (1.0 - since * 6.0).max(0.0),
                    ..AudioFrame::default()
                };
                let out = reactor.apply(mode, dt, &audio);
                for level in [out.bass, out.mid, out.treble, out.volume, out.sub, out.low_mid, out.high_mid] {
                    assert!((0.0..=1.0).contains(&level), "mode {mode}: {level}");
                }
                (out.bass, out.onset)
            })
            .collect()
    }
    /// Largest move of the bass from one frame to the next, once settled.
    fn roughest(run: &[(f32, f32)]) -> f32 {
        run[300..].windows(2).map(|pair| (pair[1].0 - pair[0].0).abs()).fold(0.0, f32::max)
    }
    #[test]
    fn strobe_flashes_on_the_beat_and_leaves_a_beatless_picture_alone() {
        let (dt, beat) = (1.0 / 60.0, 60.0 / 128.0);
        let mut reactor = Reactor::default();
        assert_eq!(reactor.flash(1), [1.0, 0.0, 0.0, 1.0]);
        let (mut brightest, mut darkest, mut hues) = (0.0f32, 9.0f32, Vec::new());
        for frame in 0..600 {
            let since = (frame as f32 * dt / beat).fract();
            let audio = AudioFrame { volume: 0.5, onset: (1.0 - since * 6.0).max(0.0), ..AudioFrame::default() };
            reactor.apply(1, dt, &audio);
            let [gain, wash, hue, on] = reactor.flash(1);
            assert!(on == 1.0 && (0.0..=0.6).contains(&wash));
            if frame > 60 {
                brightest = brightest.max(gain);
                darkest = darkest.min(gain);
                hues.push(hue);
            }
            assert_eq!(reactor.flash(0), [1.0, 0.0, 0.0, 0.0]);
        }
        assert!(brightest > 1.6 && darkest < 0.35, "{brightest} {darkest}");
        hues.dedup();
        assert!(hues.len() > 15, "{} hues", hues.len());
        // The music stops having a beat: brightness returns to normal.
        for _ in 0..300 {
            reactor.apply(1, dt, &AudioFrame { volume: 0.5, ..AudioFrame::default() });
        }
        assert!((reactor.flash(1)[0] - 1.0).abs() < 0.02);
    }
    fn drum_loop(bpm: f32, seconds: f32, drums: f32) -> Vec<AudioFrame> {
        drum_mix(&[(bpm, seconds)], drums).into_iter().map(|(_, frame)| frame).collect()
    }
    /// One frame per hop of drums playing each `(bpm, seconds)` part in turn, with the beat
    /// count at that hop: a kick on every beat, a snare on every second one and hats
    /// between, over hiss. `drums` scales everything but the hiss.
    fn drum_mix(parts: &[(f32, f32)], drums: f32) -> Vec<(f64, AudioFrame)> {
        let mut analyzer = StereoAnalyzer::default();
        let mut seed = 12345u32;
        let mut noise = move || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (seed >> 8) as f32 / 8_388_608.0 - 1.0
        };
        let total: f32 = parts.iter().map(|part| part.1).sum();
        let mut beats = 0.0f64;
        (0..(total / HOP_SECONDS) as usize)
            .map(|hop| {
                let mut block = [0.0f32; HOP * 2];
                let mut left = hop as f32 * HOP_SECONDS;
                let part = parts.iter().find(|part| {
                    left -= part.1;
                    left < 0.0
                });
                let bpm = part.map_or(parts[parts.len() - 1].0, |part| part.0);
                let beat = 60.0 / bpm;
                for pair in block.chunks_exact_mut(2) {
                    beats += f64::from(bpm) / 60.0 / f64::from(SAMPLE_RATE);
                    let (count, since) = (beats.floor(), beats.fract() as f32 * beat);
                    let half = (beats + 0.5).fract() as f32 * beat;
                    let kick = (std::f32::consts::TAU * 55.0 * since).sin() * (-since * 25.0).exp() * 0.8;
                    let snare = if count as usize % 2 == 1 { noise() * (-since * 30.0).exp() * 0.4 } else { 0.0 };
                    let hat = noise() * (-half * 120.0).exp() * 0.15;
                    let sample = (kick + snare + hat) * drums + noise() * 0.01;
                    pair.copy_from_slice(&[sample, sample]);
                }
                (beats, analyzer.process(&block))
            })
            .collect()
    }
    #[test]
    fn the_tempo_tracker_follows_a_mix_from_one_tempo_to_the_next() {
        // A cut to a faster track, then the tempo ridden up a little at a time.
        let mut parts = vec![(128.0f32, 16.0f32), (140.0, 16.0)];
        parts.extend((1..=20).map(|step| (140.0 + step as f32 * 0.4, 1.5)));
        parts.push((148.0, 6.0));
        let run = drum_mix(&parts, 1.0);
        let at = |seconds: f32| &run[(seconds / HOP_SECONDS) as usize];
        let bpm = |seconds: f32| 60.0 / at(seconds).1.beat_period;
        let mut log = String::new();
        for second in 10..68 {
            let (beats, frame) = at(second as f32);
            let next = beats + f64::from(frame.beat_ahead / frame.beat_period);
            let off = (next + 0.5).fract() - 0.5;
            log += &format!("{second}s {:.1} sure {:.2} off {off:+.2}; ", bpm(second as f32), frame.beat_sure);
        }
        assert!((bpm(15.0) - 128.0).abs() < 0.7, "{log}");
        // The cut is doubted within two seconds and the new tempo trusted within eight.
        assert!(at(18.0).1.beat_sure < 0.25 && at(20.0).1.beat_sure < 0.25, "{log}");
        assert!((bpm(24.0) - 140.0).abs() < 0.8 && at(24.0).1.beat_sure > 0.6, "{log}");
        for second in [40.0, 50.0, 60.0] {
            let truth = 140.0 + ((second - 32.0) / 1.5f32).floor().min(20.0) * 0.4;
            assert!((bpm(second) - truth).abs() < 0.8 && at(second).1.beat_sure > 0.6, "{second}: {log}");
        }
        assert!((bpm(67.0) - 148.0).abs() < 0.8, "{log}");
        // All through the ride the next beat is named to within a twentieth of a beat.
        for (beats, frame) in &run[(22.0 / HOP_SECONDS) as usize..] {
            let off = (beats + f64::from(frame.beat_ahead / frame.beat_period) + 0.5).fract() - 0.5;
            assert!((-0.01..0.06).contains(&off), "off {off}: {log}");
        }
    }
    #[test]
    fn the_tempo_tracker_finds_tempo_and_phase_and_doubts_noise() {
        for bpm in [92.0f32, 110.0, 128.0, 140.0, 174.0] {
            let beat = 60.0 / bpm;
            let frames = drum_loop(bpm, 14.0, 1.0);
            for (hop, frame) in frames.iter().enumerate().skip(frames.len() - 400) {
                let multiple = frame.beat_period / beat;
                let ratio = if bpm > 170.0 && multiple > 1.5 { multiple / 2.0 } else { multiple };
                assert!((ratio - 1.0).abs() < 0.005, "{bpm}: period {}", frame.beat_period);
                // The next beat it names is a real one, give or take the analysis delay.
                let next = (hop + 1) as f32 * HOP_SECONDS + frame.beat_ahead;
                let off = (next / beat + 0.5).fract() * beat - 0.5 * beat;
                assert!((-0.004..0.02).contains(&off), "{bpm}: off by {off} at hop {hop}");
                assert!(frame.beat_sure > 0.6, "{bpm}: sure {}", frame.beat_sure);
            }
        }
        let hiss = drum_loop(128.0, 14.0, 0.0);
        let most = hiss.iter().map(|frame| frame.beat_sure).fold(0.0, f32::max);
        assert!(most < 0.3, "sure of hiss: {most}");
    }
    /// The predictor run at 60 frames a second over hop frames; `(time, heard, shown)` onsets.
    fn predicted(frames: &[AudioFrame]) -> Vec<(f32, f32, f32)> {
        let dt = 1.0 / 60.0;
        let mut predictor = BeatPredictor::default();
        (1..(frames.len() as f32 * HOP_SECONDS / dt) as usize)
            .map(|step| {
                let time = step as f32 * dt;
                let frame = &frames[((time / HOP_SECONDS) as usize).saturating_sub(1)];
                let out = predictor.apply(dt, BEAT_LEAD, frame);
                for (shown, heard) in [(out.bass, frame.bass), (out.sub, frame.sub), (out.volume, frame.volume)] {
                    assert!((heard..=1.0).contains(&shown));
                }
                (time, frame.onset, out.onset)
            })
            .collect()
    }
    #[test]
    fn predicted_beats_lead_the_heard_ones_and_noise_passes_untouched() {
        let beat = 60.0 / 128.0;
        let run = predicted(&drum_loop(128.0, 20.0, 1.0));
        let settled = &run[run.len() - 480..];
        let rises = |pick: fn(&(f32, f32, f32)) -> f32| -> Vec<f32> {
            settled.windows(2).filter(|pair| pick(&pair[1]) - pick(&pair[0]) > 0.3).map(|pair| pair[1].0).collect()
        };
        let (heard, shown) = (rises(|frame| frame.1), rises(|frame| frame.2));
        // Every beat is drawn a lead early, and its late heard onset is dropped; the hats
        // between the beats still arrive as heard.
        let to_beat = |time: &f32| beat - (time / beat).fract() * beat;
        let early = shown.iter().filter(|time| (to_beat(time) - BEAT_LEAD).abs() < 0.03).count();
        let late = shown.iter().filter(|time| to_beat(time) > beat - 0.1).count();
        let on_beat = heard.iter().filter(|time| to_beat(time) > beat - 0.1).count();
        assert!((16..=18).contains(&early) && late == 0 && on_beat >= 16, "{early} {late} {on_beat} {shown:?}");
        assert_eq!(shown.len(), heard.len());
        let hiss = drum_loop(128.0, 14.0, 0.0);
        assert!(predicted(&hiss).iter().all(|frame| frame.1 == frame.2));
    }
    #[test]
    fn auto_keeps_the_hard_ways_for_driving_music_and_the_soft_ones_for_the_rest() {
        let (dt, beat) = (1.0 / 60.0, 60.0 / 128.0);
        let mut reactor = Reactor::default();
        // A drop, a bridge that keeps the beat but loses the bass, a drop, and no beat at all.
        for (part, (kicks, bass, pool)) in
            [(true, 0.5, &AUTO_STRONG[..]), (true, 0.05, &AUTO_SOFT[..]), (true, 0.5, &AUTO_STRONG[..]), (false, 0.3, &AUTO_SOFT[..])]
                .into_iter()
                .enumerate()
        {
            let (mut used, mut held, mut longest) = (Vec::new(), 0.0f32, 0.0f32);
            for frame in 0..(120.0 / dt) as usize {
                let time = frame as f32 * dt;
                let onset = if kicks && (time / beat).fract() < dt / beat { 1.0 } else { 0.0 };
                reactor.apply(REACT_AUTO, dt, &AudioFrame { volume: 0.5, bass, onset, ..AudioFrame::default() });
                held += dt;
                // The first seconds of a part still belong to the one before.
                if time > 8.0 && used.last() != Some(&reactor.auto) {
                    used.push(reactor.auto);
                    (longest, held) = (longest.max(held), 0.0);
                }
            }
            assert!(used.iter().all(|mode| pool.contains(mode)), "part {part}: {used:?}");
            assert!(pool.iter().all(|mode| used.contains(mode)) && used.len() > 3, "part {part}: {used:?}");
            assert!(longest < AUTO_SECONDS + 9.0, "part {part}: {longest}");
        }
        // A new section moves it on at once, but not twice in a breath.
        let before = reactor.auto;
        reactor.new_section();
        let after = reactor.auto;
        reactor.new_section();
        assert!(before != after && reactor.auto == after);
    }
    #[test]
    fn react_modes_reshape_how_the_levels_move() {
        let pump = react(0);
        assert!(roughest(&pump) > 0.5 && pump.iter().any(|frame| frame.1 > 0.9));
        // An unknown mode is the plain one.
        assert_eq!(react(200), pump);
        for mode in 1..REACT_AUTO {
            let run = react(mode);
            assert!(run.iter().all(|frame| frame.1 == 0.0), "mode {mode} passes hits on");
            assert!(run[300..].iter().any(|frame| frame.0 > 0.15), "mode {mode} stays flat");
        }
        // STROBE and RIDE move without jumps; TWIST moves the kick off the bass.
        for mode in [1, 3, 4] {
            assert!(roughest(&react(mode)) < 0.05, "mode {mode}: {}", roughest(&react(mode)));
        }
        // STEP holds its pose: most frames barely move, and the beats between them do.
        let step = react(2);
        let moves: Vec<f32> = step[300..].windows(2).map(|pair| (pair[1].0 - pair[0].0).abs()).collect();
        let still = moves.iter().filter(|m| **m < 0.005).count();
        assert!(still * 2 > moves.len() && roughest(&step) > 0.05, "{still} of {}", moves.len());
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


