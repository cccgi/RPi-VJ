use crate::audio::{AudioFrame, StereoAnalyzer, HOP};
use anyhow::{ensure, Result};
use crossbeam_channel::{bounded, Receiver, Sender};
use sdl2::audio::{AudioCallback, AudioDevice, AudioSpecDesired};
use std::sync::{
    atomic::{AtomicU32, AtomicU64, Ordering},
    Arc,
};

/// Input gain as `f32` bits; written by the render loop from the control state.
static GAIN: AtomicU32 = AtomicU32::new(0x3f80_0000);
/// Highest sample since the meter last asked, as `f32` bits: at the converter, and after
/// the input gain. Bits of non-negative floats order the way the floats do.
static PEAK_RAW: AtomicU32 = AtomicU32::new(0);
static PEAK: AtomicU32 = AtomicU32::new(0);
/// For the level meter: the highest sample at the converter and after the input gain
/// since the last call.
pub fn take_peaks() -> (f32, f32) {
    (f32::from_bits(PEAK_RAW.swap(0, Ordering::Relaxed)), f32::from_bits(PEAK.swap(0, Ordering::Relaxed)))
}
pub fn set_gain(gain: f32) {
    GAIN.store(gain.to_bits(), Ordering::Relaxed);
}

/// Sound card capture gain for the two kinds of source, in its 0.5 dB steps.
const MIC_CAPTURE: u8 = 60;
const LINE_CAPTURE: u8 = 12;
/// Set the sound card's own input gain for a microphone or a line-level source. It acts
/// ahead of the converter, where a line signal at microphone gain would clip. Runs in
/// the background; the picture never waits for the mixer.
pub fn set_line_level(device: &str, line: bool) {
    let name = device.split(',').next().unwrap_or(device).trim().to_owned();
    std::thread::spawn(move || {
        let cards = std::fs::read_to_string("/proc/asound/cards").unwrap_or_default();
        let card = cards.lines().find(|l| l.contains(&name)).and_then(|l| l.split_whitespace().next());
        let Some(card) = card else {
            eprintln!("input level: no sound card named {name}");
            return;
        };
        let (label, value) = if line { ("line", LINE_CAPTURE) } else { ("mic", MIC_CAPTURE) };
        let done = std::process::Command::new("amixer")
            .args(["-q", "-c", card, "cset", "name=Mic Capture Volume", &value.to_string()])
            .status();
        match done {
            Ok(status) if status.success() => eprintln!("input level: {label}, capture gain {value}"),
            other => eprintln!("input level not set: {other:?}"),
        }
    });
}

pub struct Capture {
    samples: [f32; HOP * 2],
    used: usize,
    tx: Sender<[f32; HOP * 2]>,
    dropped: Arc<AtomicU64>,
}
impl AudioCallback for Capture {
    type Channel = f32;
    fn callback(&mut self, samples: &mut [f32]) {
        let gain = f32::from_bits(GAIN.load(Ordering::Relaxed));
        let raw = samples.iter().fold(0.0f32, |top, sample| top.max(sample.abs()));
        if raw.is_finite() && gain.is_finite() {
            PEAK_RAW.fetch_max(raw.to_bits(), Ordering::Relaxed);
            PEAK.fetch_max((raw * gain.abs()).to_bits(), Ordering::Relaxed);
        }
        for pair in samples.as_chunks::<2>().0 {
            self.samples[self.used] = pair[0] * gain;
            self.samples[self.used + 1] = pair[1] * gain;
            self.used += 2;
            if self.used == HOP * 2 {
                if self.tx.try_send(self.samples).is_err() {
                    self.dropped.fetch_add(1, Ordering::Relaxed);
                }
                self.used = 0;
            }
        }
    }
}
pub fn start(
    audio: &sdl2::AudioSubsystem,
    name: &str,
) -> Result<(AudioDevice<Capture>, Receiver<AudioFrame>, Arc<AtomicU64>)> {
    let (tx, rx) = bounded(8);
    let (frames, receiver) = bounded(1);
    let stale = receiver.clone();
    let dropped = Arc::new(AtomicU64::new(0));
    let count = Arc::clone(&dropped);
    let device = audio
        .open_capture(
            Some(name),
            &AudioSpecDesired {
                freq: Some(48000),
                channels: Some(2),
                samples: Some(512),
            },
            move |_| Capture {
                samples: [0.0; HOP * 2],
                used: 0,
                tx,
                dropped: count,
            },
        )
        .map_err(anyhow::Error::msg)?;
    ensure!(
        device.spec().freq == 48000 && device.spec().channels == 2,
        "capture must negotiate stereo 48 kHz"
    );
    std::thread::Builder::new()
        .name("audio-dsp".into())
        .spawn(move || {
            let mut analyzer = StereoAnalyzer::default();
            while let Ok(samples) = rx.recv() {
                let frame = analyzer.process(&samples);
                // Latest-only mailbox; a slow renderer never blocks capture or DSP.
                if let Err(crossbeam_channel::TrySendError::Full(frame)) = frames.try_send(frame) {
                    let _ = stale.try_recv();
                    let _ = frames.try_send(frame);
                }
            }
        })?;
    device.resume();
    Ok((device, receiver, dropped))
}
