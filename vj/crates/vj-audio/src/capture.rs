//! Real-time audio stream capture and lock-free ring buffer handoff.
//!
//! Designed for low-latency capture (e.g., Creative Sound Blaster Play! 3 USB Audio)
//! under PipeWire (Linux) or CPAL (Windows/macOS). Real-time callbacks run with strict
//! zero-allocation, zero-lock discipline.

use std::sync::atomic::{AtomicUsize, Ordering};

use crate::{AudioCapture, AudioError, CaptureInfo, CaptureSink};

pub const DEFAULT_SAMPLE_RATE: u32 = 48_000;
pub const DEFAULT_CHANNELS: u16 = 2;
pub const DEFAULT_QUANTUM: u32 = 256;
pub const DEFAULT_GAIN: f32 = 0.25; // Attenuation for Pioneer FLX-10 booth line level

pub const SOUND_BLASTER_PLAY_3_DEVICE: &str = "Creative Sound Blaster Play! 3";

/// Configuration for an audio capture stream.
#[derive(Clone, Debug)]
pub struct AudioStreamConfig {
    pub device: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub quantum: u32,
    pub gain: f32,
}

impl Default for AudioStreamConfig {
    fn default() -> Self {
        Self {
            device: SOUND_BLASTER_PLAY_3_DEVICE.to_string(),
            sample_rate: DEFAULT_SAMPLE_RATE,
            channels: DEFAULT_CHANNELS,
            quantum: DEFAULT_QUANTUM,
            gain: DEFAULT_GAIN,
        }
    }
}

/// Applies gain scaling to a slice of audio samples in-place.
#[inline]
pub fn scale_audio_gain(samples: &mut [f32], gain: f32) {
    for s in samples.iter_mut() {
        *s *= gain;
    }
}

/// Lock-free, preallocated Single-Producer Single-Consumer (SPSC) ring buffer
/// for streaming real-time audio samples without dynamic memory allocation or mutexes.
pub struct AudioRingBuffer {
    buffer: Vec<f32>,
    capacity: usize,
    head: AtomicUsize, // Write index (producer)
    tail: AtomicUsize, // Read index (consumer)
}

impl AudioRingBuffer {
    /// Create a new preallocated ring buffer with the given sample capacity.
    pub fn new(capacity: usize) -> Self {
        let actual_cap = capacity.next_power_of_two().max(512);
        Self {
            buffer: vec![0.0; actual_cap],
            capacity: actual_cap,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    /// Push samples into the ring buffer from the real-time audio callback.
    ///
    /// This method is lock-free and allocation-free.
    /// Returns the number of samples actually written.
    #[inline]
    pub fn push_slice(&self, samples: &[f32], gain: f32) -> usize {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);

        let available = self.capacity.wrapping_sub(head.wrapping_sub(tail)).saturating_sub(1);
        let to_write = samples.len().min(available);

        let mask = self.capacity - 1;
        let mut idx = head;
        // Unsafe block to write into buffer without locking or allocating
        let buf_ptr = self.buffer.as_ptr() as *mut f32;

        for &sample in &samples[..to_write] {
            unsafe {
                *buf_ptr.add(idx & mask) = sample * gain;
            }
            idx = idx.wrapping_add(1);
        }

        self.head.store(idx, Ordering::Release);
        to_write
    }

    /// Pop samples from the ring buffer into the consumer's preallocated slice.
    ///
    /// This method is lock-free and allocation-free.
    /// Returns the number of samples actually read.
    #[inline]
    pub fn pop_slice(&self, out: &mut [f32]) -> usize {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);

        let available = head.wrapping_sub(tail);
        let to_read = out.len().min(available);

        let mask = self.capacity - 1;
        let mut idx = tail;
        let buf_ptr = self.buffer.as_ptr();

        for out_sample in &mut out[..to_read] {
            unsafe {
                *out_sample = *buf_ptr.add(idx & mask);
            }
            idx = idx.wrapping_add(1);
        }

        self.tail.store(idx, Ordering::Release);
        to_read
    }

    /// Number of samples currently available to read.
    #[inline]
    pub fn available_read(&self) -> usize {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);
        head.wrapping_sub(tail)
    }

    /// Number of samples available to write.
    #[inline]
    pub fn available_write(&self) -> usize {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        self.capacity.wrapping_sub(head.wrapping_sub(tail)).saturating_sub(1)
    }

    /// Reset ring buffer positions.
    pub fn clear(&self) {
        self.tail.store(0, Ordering::Relaxed);
        self.head.store(0, Ordering::Release);
    }
}

/// Simulated capture device for tests and environments without live hardware.
pub struct SimulatedAudioCapture {
    info: CaptureInfo,
    gain: f32,
    sink: Option<CaptureSink>,
    running: bool,
}

impl SimulatedAudioCapture {
    pub fn new(config: AudioStreamConfig) -> Self {
        Self {
            info: CaptureInfo {
                device: config.device,
                sample_rate: config.sample_rate,
                channels: config.channels,
                quantum: config.quantum,
            },
            gain: config.gain,
            sink: None,
            running: false,
        }
    }

    pub fn set_gain(&mut self, gain: f32) {
        self.gain = gain;
    }

    pub fn gain(&self) -> f32 {
        self.gain
    }

    /// Feed audio samples directly to simulate a real-time hardware buffer callback.
    pub fn feed_raw_samples(&mut self, samples: &[f32]) {
        if self.running {
            if let Some(ref mut sink) = self.sink {
                let frame_count = (samples.len() / self.info.channels as usize) as u32;
                sink(samples, frame_count);
            }
        }
    }
}

impl AudioCapture for SimulatedAudioCapture {
    fn start(&mut self, sink: CaptureSink) -> Result<(), AudioError> {
        self.sink = Some(sink);
        self.running = true;
        Ok(())
    }

    fn stop(&mut self) {
        self.running = false;
        self.sink = None;
    }

    fn info(&self) -> CaptureInfo {
        self.info.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_capture_trait_ring_handoff() {
        let config = AudioStreamConfig {
            device: SOUND_BLASTER_PLAY_3_DEVICE.to_string(),
            sample_rate: 48_000,
            channels: 2,
            quantum: 256,
            gain: 1.0, // Unity gain for raw value verification
        };

        let ring = Arc::new(AudioRingBuffer::new(2048));
        let ring_producer = Arc::clone(&ring);

        let mut capture = SimulatedAudioCapture::new(config);

        // Start capture with a real-time sink writing into the preallocated ring buffer
        capture
            .start(Box::new(move |samples: &[f32], _frames: u32| {
                let written = ring_producer.push_slice(samples, 1.0);
                assert_eq!(written, samples.len(), "Ring buffer should accept all samples");
            }))
            .expect("Capture start should succeed");

        // Simulate 4 quantum frames (4 * 256 stereo frames = 2048 samples total) in chunks of 512 samples
        let total_samples = 1024;
        let synthetic_samples: Vec<f32> = (0..total_samples).map(|i| i as f32 * 0.1).collect();

        // Feed first quantum (512 samples)
        capture.feed_raw_samples(&synthetic_samples[..512]);
        assert_eq!(ring.available_read(), 512);

        // Consumer reads first 256 samples
        let mut read_buf = vec![0.0; 256];
        let read1 = ring.pop_slice(&mut read_buf);
        assert_eq!(read1, 256);
        assert_eq!(&read_buf[..], &synthetic_samples[..256]);

        // Feed second quantum (512 samples)
        capture.feed_raw_samples(&synthetic_samples[512..1024]);
        assert_eq!(ring.available_read(), 512 - 256 + 512);

        // Consumer reads remaining samples
        let mut remaining_buf = vec![0.0; 768];
        let read2 = ring.pop_slice(&mut remaining_buf);
        assert_eq!(read2, 768);
        assert_eq!(&remaining_buf[..], &synthetic_samples[256..1024]);

        assert_eq!(ring.available_read(), 0);
        capture.stop();
    }

    #[test]
    fn test_audio_gain_scaling() {
        let hot_signal: Vec<f32> = vec![1.0, -1.0, 2.0, -0.8, 0.5];

        // 1. In-place scaling function
        let mut in_place = hot_signal.clone();
        scale_audio_gain(&mut in_place, DEFAULT_GAIN);
        assert_eq!(in_place[0], 0.25);
        assert_eq!(in_place[1], -0.25);
        assert_eq!(in_place[2], 0.5);
        assert_eq!(in_place[3], -0.2);
        assert_eq!(in_place[4], 0.125);

        // 2. Ring buffer push with gain scaling
        let ring = AudioRingBuffer::new(512);
        let written = ring.push_slice(&hot_signal, DEFAULT_GAIN);
        assert_eq!(written, hot_signal.len());

        let mut read_out = vec![0.0; hot_signal.len()];
        let read = ring.pop_slice(&mut read_out);
        assert_eq!(read, hot_signal.len());

        for (original, &scaled) in hot_signal.iter().zip(read_out.iter()) {
            assert!(
                (scaled - (original * DEFAULT_GAIN)).abs() < 1e-6,
                "Scaled sample {} does not match expected {}",
                scaled,
                original * DEFAULT_GAIN
            );
        }
    }
}
