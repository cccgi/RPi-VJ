use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, Sender};
use hidapi::{HidApi, HidDevice};
use image::{codecs::jpeg::JpegEncoder, ImageBuffer, Rgb, RgbImage};
use std::{io::Cursor, thread, time::Duration};

const ELGATO_VENDOR: u16 = 0x0fd9;
const XL_PRODUCTS: [u16; 2] = [0x006c, 0x008f];
const KEYS: usize = 32;
const ICON: u32 = 96;
const REPORT: usize = 1024;

#[derive(Debug)]
pub enum DeckEvent {
    Connected,
    Disconnected,
    Pressed(u8),
}

pub enum DeckCommand {
    SetIcon {
        key: u8,
        jpeg: Vec<u8>,
        active_jpeg: Vec<u8>,
    },
    Active(u8),
}

struct IconSet {
    normal: Vec<u8>,
    active: Vec<u8>,
}

pub struct StreamDeck {
    _thread: thread::JoinHandle<()>,
}

impl StreamDeck {
    pub fn spawn(events: Sender<DeckEvent>, commands: Receiver<DeckCommand>) -> Self {
        let worker = thread::Builder::new()
            .name("streamdeck-hid".into())
            .spawn(move || {
                let mut icons: Vec<Option<IconSet>> = (0..KEYS).map(|_| None).collect();
                let mut active = None;
                loop {
                    match connect() {
                        Ok(device) => {
                            let _ = events.send(DeckEvent::Connected);
                            let result = replay_icons(&device, &icons, active).and_then(|()| {
                                run_connected(
                                    &device,
                                    &events,
                                    &commands,
                                    &mut icons,
                                    &mut active,
                                )
                            });
                            if let Err(error) = result {
                                log::debug!("Stream Deck connection lost: {error:#}");
                                let _ = events.send(DeckEvent::Disconnected);
                            }
                        }
                        Err(error) => log::debug!("Stream Deck unavailable: {error:#}"),
                    }
                    thread::sleep(Duration::from_secs(1));
                }
            })
            .expect("create Stream Deck worker thread");
        Self { _thread: worker }
    }
}

fn connect() -> Result<HidDevice> {
    let api = HidApi::new().context("initialize hidapi")?;
    for info in api.device_list() {
        if info.vendor_id() == ELGATO_VENDOR && XL_PRODUCTS.contains(&info.product_id()) {
            return info.open_device(&api).context("open Stream Deck XL");
        }
    }
    anyhow::bail!("no supported Stream Deck XL found")
}

fn run_connected(
    device: &HidDevice,
    events: &Sender<DeckEvent>,
    commands: &Receiver<DeckCommand>,
    icons: &mut [Option<IconSet>],
    active: &mut Option<u8>,
) -> Result<()> {
    let mut previous = [false; KEYS];
    let mut report = [0_u8; REPORT];
    loop {
        while let Ok(command) = commands.try_recv() {
            match command {
                DeckCommand::SetIcon {
                    key,
                    jpeg,
                    active_jpeg,
                } if usize::from(key) < KEYS => {
                    write_xl_image(device, key, &jpeg)?;
                    icons[usize::from(key)] = Some(IconSet {
                        normal: jpeg,
                        active: active_jpeg,
                    });
                }
                DeckCommand::Active(new_active) => {
                    if let Some(previous) = active.replace(new_active) {
                        if let Some(Some(icon)) = icons.get(usize::from(previous)) {
                            write_xl_image(device, previous, &icon.normal)?;
                        }
                    }
                    if let Some(Some(icon)) = icons.get(usize::from(new_active)) {
                        write_xl_image(device, new_active, &icon.active)?;
                    }
                    log::info!("active Stream Deck key {new_active}");
                }
                DeckCommand::SetIcon { .. } => log::warn!("ignoring invalid Stream Deck key"),
            }
        }
        let count = device.read_timeout(&mut report, 20)?;
        if count >= KEYS + 4 && report[0] == 0x01 {
            for key in 0..KEYS {
                let down = report[key + 4] != 0;
                if down && !previous[key] {
                    events.send(DeckEvent::Pressed(key as u8))?;
                }
                previous[key] = down;
            }
        }
    }
}

fn replay_icons(device: &HidDevice, icons: &[Option<IconSet>], active: Option<u8>) -> Result<()> {
    for (key, icon) in icons.iter().enumerate() {
        if let Some(icon) = icon {
            let jpeg = if active == Some(key as u8) {
                &icon.active
            } else {
                &icon.normal
            };
            write_xl_image(device, key as u8, jpeg)?;
        }
    }
    Ok(())
}

// Stream Deck XL protocol: report 0x02, 1-based key, page metadata, JPEG payload.
fn write_xl_image(device: &HidDevice, key: u8, jpeg: &[u8]) -> Result<()> {
    anyhow::ensure!(usize::from(key) < KEYS, "invalid Stream Deck key {key}");
    let mut offset = 0;
    let mut page: u16 = 0;
    while offset < jpeg.len() {
        let chunk = (jpeg.len() - offset).min(REPORT - 8);
        let last = offset + chunk == jpeg.len();
        let mut packet = vec![0_u8; REPORT];
        packet[0] = 0x02;
        packet[1] = 0x07;
        packet[2] = key + 1;
        packet[3] = u8::from(last);
        packet[4..6].copy_from_slice(&(chunk as u16).to_le_bytes());
        packet[6..8].copy_from_slice(&page.to_le_bytes());
        packet[8..8 + chunk].copy_from_slice(&jpeg[offset..offset + chunk]);
        device.write(&packet)?;
        offset += chunk;
        page += 1;
    }
    Ok(())
}

pub fn make_icon(label: &str, background: Rgb<u8>, foreground: Rgb<u8>) -> Result<Vec<u8>> {
    let mut image: RgbImage = ImageBuffer::from_pixel(ICON, ICON, background);
    // Allocation-free status motif; deployments normally replace this with extracted thumbnails.
    let hash = label
        .bytes()
        .fold(0_u32, |h, b| h.wrapping_mul(33).wrapping_add(u32::from(b)));
    for y in 12..84 {
        for x in 12..84 {
            if ((x / 8 + y / 8 + hash) & 3) == 0 {
                image.put_pixel(x, y, foreground);
            }
        }
    }
    let mut output = Cursor::new(Vec::with_capacity(8 * 1024));
    JpegEncoder::new_with_quality(&mut output, 85).encode_image(&image)?;
    Ok(output.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn icon_is_jpeg() {
        let icon =
            make_icon("clip", Rgb([0, 0, 0]), Rgb([255, 255, 255])).expect("encode icon");
        assert_eq!(&icon[..2], &[0xff, 0xd8]);
    }
}
