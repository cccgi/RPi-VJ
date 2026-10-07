use crate::audio::{AudioFrame, StereoAnalyzer, HOP};
use anyhow::{ensure, Result};
use crossbeam_channel::{bounded, Receiver, Sender};
use sdl2::audio::{AudioCallback, AudioDevice, AudioSpecDesired};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

pub struct Capture {
    samples: [f32; HOP * 2],
    used: usize,
    tx: Sender<[f32; HOP * 2]>,
    dropped: Arc<AtomicU64>,
}
impl AudioCallback for Capture {
    type Channel = f32;
    fn callback(&mut self, samples: &mut [f32]) {
        for pair in samples.as_chunks::<2>().0 {
            self.samples[self.used..self.used + 2].copy_from_slice(pair);
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
