//! HID input is polled between every JPEG packet; image preparation has its own worker.
use crate::{
    control::{Command, State, CLIPS_PER_PAGE},
    show::Show,
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

#[derive(Clone)]
pub struct View {
    revision: u64,
    page: usize,
    playing: Option<usize>,
    requested: Option<usize>,
    failed: Option<usize>,
    shader: bool,
    blackout: bool,
}
impl From<&State> for View {
    fn from(state: &State) -> Self {
        Self {
            revision: 0,
            page: state.page,
            playing: state.playing,
            requested: state.requested,
            failed: state.failed,
            shader: state.shader,
            blackout: state.blackout,
        }
    }
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
impl std::ops::Deref for Page {
    type Target = [Key];
    fn deref(&self) -> &Self::Target {
        &self.keys
    }
}
#[derive(Default)]
struct Health {
    online: AtomicBool,
    painted: AtomicU64,
}
pub struct Deck {
    views: Sender<View>,
    stale: Receiver<View>,
    pub commands: Receiver<Command>,
    revision: AtomicU64,
    health: Arc<Health>,
}
impl Deck {
    pub fn start(show: Arc<Show>) -> Result<Self> {
        let (views, updates) = bounded(1);
        let stale = updates.clone();
        let (images, pages) = bounded(1);
        let old_pages = pages.clone();
        let (events, commands) = bounded(64);
        let health = Arc::new(Health::default());
        let worker_health = Arc::clone(&health);
        std::thread::Builder::new()
            .name("deck-images".into())
            .spawn(move || {
                while let Ok(view) = updates.recv() {
                    match page(&show, &view) {
                        Ok(page) => {
                            let _ = old_pages.try_recv();
                            if images.send(page).is_err() {
                                break;
                            }
                        }
                        Err(error) => eprintln!("deck image preparation failed: {error:#}"),
                    }
                }
            })?;
        std::thread::Builder::new()
            .name("deck-hid".into())
            .spawn(move || {
                let mut desired = Page::default();
                loop {
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
                    std::thread::sleep(Duration::from_secs(1));
                }
            })?;
        Ok(Self {
            views,
            stale,
            commands,
            revision: AtomicU64::new(0),
            health,
        })
    }
    pub fn publish(&self, state: &State) {
        let mut view = View::from(state);
        view.revision = self.revision.fetch_add(1, Ordering::AcqRel) + 1;
        let _ = self.stale.try_recv();
        let _ = self.views.try_send(view);
    }
    pub fn ready(&self) -> bool {
        let revision = self.revision.load(Ordering::Acquire);
        self.health.online.load(Ordering::Acquire)
            && revision != 0
            && self.health.painted.load(Ordering::Acquire) == revision
    }
}
fn connect() -> Result<HidDevice> {
    let api = HidApi::new()?;
    for info in api.device_list() {
        if info.vendor_id() == 0x0fd9 && [0x006c, 0x008f].contains(&info.product_id()) {
            return Ok(info.open_device(&api)?);
        }
    }
    anyhow::bail!("no Stream Deck XL");
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
    loop {
        match pages.try_recv() {
            Ok(page) => {
                *desired = page;
                transfer = None;
            }
            Err(crossbeam_channel::TryRecvError::Disconnected) => return Ok(()),
            Err(crossbeam_channel::TryRecvError::Empty) => {}
        }
        let count = device.read_timeout(&mut input, 1)?;
        if count >= 36 && input[0..4] == [1, 0, 32, 0] {
            for key in 0..32 {
                let down = input[key + 4] != 0;
                if down && !previous[key] {
                    if let Some(command) = displayed[key].as_ref().and_then(|key| key.command) {
                        // Explicit rejection instead of blocking USB reads behind the renderer.
                        if events.try_send(command).is_err() {
                            eprintln!("deck command queue full; press rejected");
                        }
                    }
                }
                previous[key] = down;
            }
        }
        if transfer.is_none() {
            transfer = desired
                .iter()
                .enumerate()
                .find(|(i, key)| {
                    displayed[*i]
                        .as_ref()
                        .is_none_or(|old| old.jpeg != key.jpeg || old.command != key.command)
                })
                .map(|(i, _)| (i, 0));
            if transfer.is_none() && desired.len() == 32 {
                health.painted.store(desired.revision, Ordering::Release);
            }
        }
        if let Some((key, offset)) = transfer {
            let image = &desired[key].jpeg;
            let count = (image.len() - offset).min(1016);
            let last = offset + count == image.len();
            let mut packet = [0_u8; 1024];
            packet[..4].copy_from_slice(&[2, 7, key as u8, u8::from(last)]);
            packet[4..6].copy_from_slice(&(count as u16).to_le_bytes());
            packet[6..8].copy_from_slice(&((offset / 1016) as u16).to_le_bytes());
            packet[8..8 + count].copy_from_slice(&image[offset..offset + count]);
            ensure!(
                device.write(&packet)? == packet.len(),
                "short HID image write"
            );
            if last {
                displayed[key] = Some(desired[key].clone());
                transfer = None;
            } else {
                transfer = Some((key, offset + count));
            }
        }
    }
}
fn page(show: &Show, view: &View) -> Result<Page> {
    let mut page = Vec::with_capacity(32);
    for key in 0..32 {
        let clip = view.page * CLIPS_PER_PAGE + key;
        let (label, command, background, thumb) = if key < CLIPS_PER_PAGE {
            match show.clips.get(clip) {
                Some(item) => (
                    item.label.as_str(),
                    Some(Command::TriggerClip { clip }),
                    if view.playing == Some(clip) {
                        [0, 120, 40]
                    } else if view.failed == Some(clip) {
                        [160, 0, 0]
                    } else if view.requested == Some(clip) {
                        [140, 85, 0]
                    } else {
                        [18, 24, 35]
                    },
                    item.thumbnail.as_deref(),
                ),
                None => ("EMPTY", None, [0, 0, 0], None),
            }
        } else {
            match key {
                28 => (
                    "PREV",
                    Some(Command::Page { delta: -1 }),
                    [20, 40, 80],
                    None,
                ),
                29 => ("NEXT", Some(Command::Page { delta: 1 }), [20, 40, 80], None),
                30 => (
                    "SHADER",
                    Some(Command::SelectShader {
                        enabled: !view.shader,
                    }),
                    if view.shader {
                        [0, 100, 90]
                    } else {
                        [20, 40, 80]
                    },
                    None,
                ),
                _ => (
                    "BLACK",
                    Some(Command::Blackout {
                        enabled: !view.blackout,
                    }),
                    if view.blackout {
                        [160, 0, 0]
                    } else {
                        [20, 40, 80]
                    },
                    None,
                ),
            }
        };
        let mut image = RgbImage::from_pixel(96, 96, Rgb(background));
        if let Some(path) = thumb {
            // Small prepared thumbnails only. Decode failures preserve a readable label.
            if let Ok(reader) = image::ImageReader::open(path) {
                if let Ok(decoded) = reader.decode() {
                    let thumb = decoded
                        .resize_exact(88, 64, image::imageops::FilterType::Triangle)
                        .to_rgb8();
                    image::imageops::replace(&mut image, &thumb, 4, 4);
                }
            }
        }
        for (index, character) in label.to_uppercase().chars().take(20).enumerate() {
            if let Some(glyph) = font8x8::BASIC_FONTS.get(character) {
                for (y, bits) in glyph.iter().enumerate() {
                    for x in 0..8 {
                        if bits & (1 << x) != 0 {
                            image.put_pixel(
                                8 + (index % 10) as u32 * 8 + x,
                                72 + (index / 10) as u32 * 10 + y as u32,
                                Rgb([255, 255, 255]),
                            );
                        }
                    }
                }
            }
        }
        // XL native orientation is 180 degrees; keys are zero-based in report 0x07.
        let image = image::imageops::rotate180(&image);
        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 85).encode_image(&image)?;
        page.push(Key { jpeg, command });
    }
    Ok(Page {
        revision: view.revision,
        keys: page,
    })
}
