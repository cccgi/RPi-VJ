//! Native HID controller, deliberately independent of SDL, OpenGL, and libmpv.
//! Input is polled between JPEG packets; decoding/encoding never runs on the HID thread.
use crate::{
    control::{Command, State},
    show::Show,
    surface::{self, KeyDescriptor},
};
use anyhow::{ensure, Result};
use crossbeam_channel::{bounded, Receiver, Sender};
use font8x8::UnicodeFonts;
use hidapi::{HidApi, HidDevice};
use image::{Rgb, RgbImage};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

struct View {
    revision: u64,
    keys: Vec<KeyDescriptor>,
    renderer_online: bool,
}
#[derive(Clone)]
struct Key {
    jpeg: Vec<u8>,
    command: Option<Command>,
}
#[derive(Default)]
struct Page {
    revision: u64,
    keys: Vec<Key>,
}
#[derive(Default)]
struct Health {
    online: AtomicBool,
    painted: AtomicU64,
    progress: AtomicU64,
    rejected: AtomicU64,
    stopping: AtomicBool,
    images_progress: AtomicU64,
}
/// Counters permit a supervisor to distinguish absent hardware from a wedged worker.
#[derive(Debug, Clone, Copy)]
pub struct DeckHealth {
    pub connected: bool,
    pub desired_revision: u64,
    pub painted_revision: u64,
    pub hid_progress: u64,
    pub image_progress: u64,
    pub rejected_commands: u64,
}
pub struct Deck {
    views: Sender<View>,
    stale: Receiver<View>,
    pub commands: Receiver<Command>,
    revision: AtomicU64,
    health: Arc<Health>,
    show: Arc<Show>,
}
impl Deck {
    pub fn start(show: Arc<Show>) -> Result<Self> {
        let (views, updates) = bounded::<View>(1);
        let stale = updates.clone();
        let (images, pages) = bounded(1);
        let old_pages = pages.clone();
        let (events, commands) = bounded(64);
        let health = Arc::new(Health::default());
        let image_health = Arc::clone(&health);
        std::thread::Builder::new()
            .name("deck-images".into())
            .spawn(move || {
                while !image_health.stopping.load(Ordering::Acquire) {
                    image_health.images_progress.fetch_add(1, Ordering::Relaxed);
                    let view = match updates.recv_timeout(Duration::from_millis(100)) {
                        Ok(view) => view,
                        Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
                        Err(_) => break,
                    };
                    match prepare(view) {
                        Ok(page) => {
                            let _ = old_pages.try_recv();
                            let _ = images.try_send(page);
                        }
                        Err(error) => eprintln!("deck image preparation failed: {error:#}"),
                    }
                }
            })?;
        let worker_health = Arc::clone(&health);
        if let Err(error) = std::thread::Builder::new()
            .name("deck-hid".into())
            .spawn(move || {
                let mut desired = Page::default();
                while !worker_health.stopping.load(Ordering::Acquire) {
                    worker_health.progress.fetch_add(1, Ordering::Relaxed);
                    while let Ok(page) = pages.try_recv() {
                        desired = page;
                    }
                    match connect().and_then(|device| {
                        connected(device, &pages, &events, &mut desired, &worker_health)
                    }) {
                        Ok(()) => break,
                        Err(error) => eprintln!("Stream Deck offline: {error:#}"),
                    }
                    worker_health.online.store(false, Ordering::Release);
                    for _ in 0..10 {
                        if worker_health.stopping.load(Ordering::Acquire) {
                            break;
                        }
                        worker_health.progress.fetch_add(1, Ordering::Relaxed);
                        std::thread::sleep(Duration::from_millis(100));
                    }
                }
                worker_health.online.store(false, Ordering::Release);
            })
        {
            health.stopping.store(true, Ordering::Release);
            return Err(error.into());
        }
        Ok(Self {
            views,
            stale,
            commands,
            revision: AtomicU64::new(0),
            health,
            show,
        })
    }
    pub fn publish(&self, state: &State, renderer_online: bool) {
        let revision = self.revision.fetch_add(1, Ordering::AcqRel) + 1;
        let view = View {
            revision,
            keys: surface::page(&self.show, state),
            renderer_online,
        };
        let _ = self.stale.try_recv();
        let _ = self.views.try_send(view);
    }
    pub fn ready(&self) -> bool {
        let h = self.health();
        h.connected && h.desired_revision != 0 && h.painted_revision == h.desired_revision
    }
    pub fn health(&self) -> DeckHealth {
        DeckHealth {
            connected: self.health.online.load(Ordering::Acquire),
            desired_revision: self.revision.load(Ordering::Acquire),
            painted_revision: self.health.painted.load(Ordering::Acquire),
            hid_progress: self.health.progress.load(Ordering::Relaxed),
            image_progress: self.health.images_progress.load(Ordering::Relaxed),
            rejected_commands: self.health.rejected.load(Ordering::Relaxed),
        }
    }
}
impl Drop for Deck {
    fn drop(&mut self) {
        self.health.stopping.store(true, Ordering::Release);
    }
}
fn connect() -> Result<HidDevice> {
    let api = HidApi::new()?;
    for info in api.device_list() {
        if info.vendor_id() == 0x0fd9 && [0x006c, 0x008f].contains(&info.product_id()) {
            return Ok(info.open_device(&api)?);
        }
    }
    anyhow::bail!("no Stream Deck XL")
}
fn connected(
    device: HidDevice,
    pages: &Receiver<Page>,
    events: &Sender<Command>,
    desired: &mut Page,
    health: &Health,
) -> Result<()> {
    let mut reset = [0_u8; 32];
    reset[..2].copy_from_slice(&[3, 2]);
    device.send_feature_report(&reset)?;
    let mut brightness = [0_u8; 32];
    brightness[..3].copy_from_slice(&[3, 8, 70]);
    device.send_feature_report(&brightness)?;
    health.painted.store(0, Ordering::Release);
    health.online.store(true, Ordering::Release);
    let mut displayed: Vec<Option<Key>> = vec![None; 32];
    let mut previous = [false; 32];
    let mut transfer: Option<(usize, usize)> = None;
    let mut input = [0_u8; 1024];
    while !health.stopping.load(Ordering::Acquire) {
        health.progress.fetch_add(1, Ordering::Relaxed);
        match pages.try_recv() {
            Ok(page) => {
                *desired = page;
                transfer = None;
            }
            Err(crossbeam_channel::TryRecvError::Disconnected) => return Ok(()),
            Err(crossbeam_channel::TryRecvError::Empty) => {}
        }
        let count = device.read_timeout(&mut input, 1)?;
        if let Some(pressed) = pressed_edges(&input[..count], &mut previous) {
            for key in pressed {
                // Bindings change only when that key's complete image is uploaded.
                if let Some(command) = displayed[key].as_ref().and_then(|key| key.command) {
                    if events.try_send(command).is_err() {
                        health.rejected.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }
        if transfer.is_none() {
            transfer = desired
                .keys
                .iter()
                .enumerate()
                .find(|(i, key)| {
                    displayed[*i]
                        .as_ref()
                        .is_none_or(|old| old.jpeg != key.jpeg || old.command != key.command)
                })
                .map(|(i, _)| (i, 0));
            if transfer.is_none() && desired.keys.len() == 32 {
                health.painted.store(desired.revision, Ordering::Release);
            }
        }
        if let Some((key, offset)) = transfer {
            let image = &desired.keys[key].jpeg;
            let (packet, next, last) = image_packet(key, image, offset)?;
            ensure!(
                device.write(&packet)? == packet.len(),
                "short HID image write"
            );
            if last {
                displayed[key] = Some(desired.keys[key].clone());
                transfer = None;
            } else {
                transfer = Some((key, next));
            }
        }
    }
    Ok(())
}
fn pressed_edges(input: &[u8], previous: &mut [bool; 32]) -> Option<Vec<usize>> {
    if input.len() < 36 || input[..4] != [1, 0, 32, 0] {
        return None;
    }
    let mut pressed = Vec::new();
    for key in 0..32 {
        let down = input[key + 4] != 0;
        if down && !previous[key] {
            pressed.push(key);
        }
        previous[key] = down;
    }
    Some(pressed)
}
fn image_packet(key: usize, jpeg: &[u8], offset: usize) -> Result<([u8; 1024], usize, bool)> {
    ensure!(
        key < 32 && offset < jpeg.len() && offset.is_multiple_of(1016),
        "invalid XL image packet"
    );
    ensure!(jpeg.len() <= 96 * 96 * 4, "oversized XL image");
    let count = (jpeg.len() - offset).min(1016);
    let next = offset + count;
    let last = next == jpeg.len();
    let mut packet = [0_u8; 1024];
    packet[..4].copy_from_slice(&[2, 7, key as u8, u8::from(last)]);
    packet[4..6].copy_from_slice(&(count as u16).to_le_bytes());
    packet[6..8].copy_from_slice(&((offset / 1016) as u16).to_le_bytes());
    packet[8..8 + count].copy_from_slice(&jpeg[offset..next]);
    Ok((packet, next, last))
}
fn prepare(view: View) -> Result<Page> {
    ensure!(view.keys.len() == 32, "surface must contain 32 keys");
    let mut keys = Vec::with_capacity(32);
    for descriptor in view.keys {
        let mut canvas = RgbImage::from_pixel(96, 96, Rgb(descriptor.color));
        if let Some(path) = descriptor.thumbnail.as_deref() {
            // Limit compressed bytes, dimensions and decoder allocations independently.
            if std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.len() <= 4 * 1024 * 1024) {
                if let Ok(mut reader) = image::ImageReader::open(path) {
                    let mut limits = image::Limits::default();
                    limits.max_image_width = Some(2048);
                    limits.max_image_height = Some(2048);
                    limits.max_alloc = Some(32 * 1024 * 1024);
                    reader.limits(limits);
                    if let Ok(decoded) = reader.decode() {
                        let thumb = decoded
                            .resize_exact(88, 64, image::imageops::FilterType::Triangle)
                            .to_rgb8();
                        image::imageops::replace(&mut canvas, &thumb, 4, 4);
                    }
                }
            }
        }
        let label = descriptor.label;
        if label.starts_with('#') {
            draw_large_index(&mut canvas, &label);
        } else {
            draw_text(&mut canvas, &label, 72, 20);
        }
        if !view.renderer_online {
            // Preserve navigation and emergency bindings during renderer recovery.
            for y in 0..10 {
                for x in 0..96 {
                    canvas.put_pixel(x, y, Rgb([160, 60, 0]));
                }
            }
            draw_text(&mut canvas, "OFFLINE", 1, 7);
        }
        let canvas = image::imageops::rotate180(&canvas);
        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 85).encode_image(&canvas)?;
        keys.push(Key {
            jpeg,
            command: descriptor.command,
        });
    }
    Ok(Page {
        revision: view.revision,
        keys,
    })
}
fn draw_large_index(canvas: &mut RgbImage, text: &str) {
    let scale = 3_u32;
    let width = text.chars().count() as u32 * 8 * scale;
    let left = (96_u32.saturating_sub(width)) / 2;
    let top = 36;
    for (index, character) in text.chars().enumerate() {
        if let Some(glyph) = font8x8::BASIC_FONTS.get(character) {
            for (y, bits) in glyph.iter().enumerate() {
                for x in 0..8 {
                    if bits & (1 << x) != 0 {
                        for dy in 0..scale {
                            for dx in 0..scale {
                                let px = left + index as u32 * 8 * scale + x * scale + dx;
                                let py = top + y as u32 * scale + dy;
                                if px < 96 && py < 96 { canvas.put_pixel(px, py, Rgb([255, 255, 255])); }
                            }
                        }
                    }
                }
            }
        }
    }
}
fn draw_text(canvas: &mut RgbImage, text: &str, top: u32, max: usize) {
    for (index, character) in text.to_uppercase().chars().take(max).enumerate() {
        if let Some(glyph) = font8x8::BASIC_FONTS.get(character) {
            for (y, bits) in glyph.iter().enumerate() {
                for x in 0..8 {
                    if bits & (1 << x) != 0 {
                        canvas.put_pixel(
                            8 + (index % 10) as u32 * 8 + x,
                            top + (index / 10) as u32 * 10 + y as u32,
                            Rgb([255, 255, 255]),
                        );
                    }
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn xl_packets_use_zero_based_keys_and_little_endian_sequence() {
        let jpeg = vec![0x5a; 2200];
        let (first, next, last) = image_packet(0, &jpeg, 0).unwrap();
        assert_eq!(&first[..8], &[2, 7, 0, 0, 248, 3, 0, 0]);
        assert_eq!(next, 1016);
        assert!(!last);
        let (last_packet, next, last) = image_packet(31, &jpeg, 2032).unwrap();
        assert_eq!(&last_packet[..8], &[2, 7, 31, 1, 168, 0, 2, 0]);
        assert_eq!(next, 2200);
        assert!(last);
        assert!(image_packet(32, &jpeg, 0).is_err());
        assert!(image_packet(0, &jpeg, 2200).is_err());
    }
    #[test]
    fn held_keys_do_not_retrigger_and_malformed_reports_preserve_edges() {
        let mut previous = [false; 32];
        let mut input = [0; 36];
        input[..4].copy_from_slice(&[1, 0, 32, 0]);
        input[35] = 1;
        assert_eq!(pressed_edges(&input, &mut previous), Some(vec![31]));
        assert_eq!(pressed_edges(&input, &mut previous), Some(vec![]));
        assert_eq!(pressed_edges(&input[..4], &mut previous), None);
        input[35] = 0;
        pressed_edges(&input, &mut previous);
        input[35] = 1;
        assert_eq!(pressed_edges(&input, &mut previous), Some(vec![31]));
    }
}
