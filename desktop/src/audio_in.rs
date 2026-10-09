//! Audio input: any capture device, or what an output device is playing (loopback),
//! brought to the analyzer's 48 kHz stereo and analysed on the audio thread.
use anyhow::{bail, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rpi_vj_native::audio::{AudioFrame, StereoAnalyzer, HOP, SAMPLE_RATE};
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc, Mutex,
};

pub struct Source {
    pub name: String,
    /// An output device, heard through loopback: the music the computer is playing.
    pub loopback: bool,
    device: cpal::Device,
}
impl Source {
    pub fn label(&self) -> String {
        if self.loopback { format!("{} (what is playing)", self.name) } else { self.name.clone() }
    }
}
/// What the audio thread hands to the render loop.
pub struct Shared {
    pub frame: Mutex<AudioFrame>,
    /// Input gain and the highest sample since last asked, before and after it, as `f32` bits.
    pub gain: AtomicU32,
    pub peak_raw: AtomicU32,
    pub peak: AtomicU32,
    pub error: Mutex<Option<String>>,
}
impl Default for Shared {
    fn default() -> Self {
        Self {
            frame: Mutex::new(AudioFrame::default()),
            gain: AtomicU32::new(1.0f32.to_bits()),
            peak_raw: AtomicU32::new(0),
            peak: AtomicU32::new(0),
            error: Mutex::new(None),
        }
    }
}
pub struct Capture {
    _stream: cpal::Stream,
    pub rate: u32,
}
/// The name of the output device the system plays through by default.
pub fn default_output() -> Option<String> {
    cpal::default_host().default_output_device()?.name().ok()
}
pub fn sources() -> Vec<Source> {
    let host = cpal::default_host();
    let mut found = Vec::new();
    for (loopback, devices) in [(false, host.input_devices().ok()), (true, host.output_devices().ok())] {
        for device in devices.into_iter().flatten() {
            if let Ok(name) = device.name() {
                found.push(Source { name, loopback, device });
            }
        }
    }
    found
}
/// Takes interleaved input at any rate and channel count; emits analyzer frames.
struct Feed {
    channels: usize,
    /// Input samples per output sample.
    step: f32,
    phase: f32,
    previous: [f32; 2],
    block: [f32; HOP * 2],
    used: usize,
    analyzer: StereoAnalyzer,
    shared: Arc<Shared>,
}
impl Feed {
    fn push(&mut self, samples: impl Iterator<Item = f32>) {
        let gain = f32::from_bits(self.shared.gain.load(Ordering::Relaxed));
        let (mut raw, mut frame, mut at) = (0.0f32, [0.0f32; 2], 0);
        for sample in samples {
            let sample = if sample.is_finite() { sample } else { 0.0 };
            if at < 2 {
                frame[at] = sample;
            }
            at += 1;
            if at < self.channels {
                continue;
            }
            if self.channels == 1 {
                frame[1] = frame[0];
            }
            at = 0;
            raw = raw.max(frame[0].abs()).max(frame[1].abs());
            let now = frame.map(|value| value * gain);
            // Straight-line resampling: plenty for level and beat analysis.
            while self.phase < 1.0 {
                for side in 0..2 {
                    self.block[self.used + side] = self.previous[side] + (now[side] - self.previous[side]) * self.phase;
                }
                self.used += 2;
                if self.used == HOP * 2 {
                    let analysed = self.analyzer.process(&self.block);
                    if let Ok(mut latest) = self.shared.frame.lock() {
                        *latest = analysed;
                    }
                    self.used = 0;
                }
                self.phase += self.step;
            }
            self.phase -= 1.0;
            self.previous = now;
        }
        self.shared.peak_raw.fetch_max(raw.to_bits(), Ordering::Relaxed);
        self.shared.peak.fetch_max((raw * gain.abs()).to_bits(), Ordering::Relaxed);
    }
}
pub fn start(source: &Source, shared: Arc<Shared>) -> Result<Capture> {
    let supported = if source.loopback {
        source.device.default_output_config()
    } else {
        source.device.default_input_config()
    }
    .with_context(|| format!("{} has no usable format", source.name))?;
    let (format, config) = (supported.sample_format(), supported.config());
    let mut feed = Feed {
        channels: usize::from(config.channels).max(1),
        step: config.sample_rate.0 as f32 / SAMPLE_RATE,
        phase: 0.0,
        previous: [0.0; 2],
        block: [0.0; HOP * 2],
        used: 0,
        analyzer: StereoAnalyzer::default(),
        shared: Arc::clone(&shared),
    };
    let failed = move |error: cpal::StreamError| {
        if let Ok(mut slot) = shared.error.lock() {
            *slot = Some(error.to_string());
        }
    };
    let stream = match format {
        cpal::SampleFormat::F32 => source.device.build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| feed.push(data.iter().copied()),
            failed,
            None,
        ),
        cpal::SampleFormat::I16 => source.device.build_input_stream(
            &config,
            move |data: &[i16], _: &cpal::InputCallbackInfo| feed.push(data.iter().map(|s| f32::from(*s) / 32768.0)),
            failed,
            None,
        ),
        other => bail!("{} delivers {other:?} samples, which is not handled yet", source.name),
    }?;
    stream.play()?;
    Ok(Capture { _stream: stream, rate: config.sample_rate.0 })
}
