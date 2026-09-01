mod player;
mod streamdeck;

use anyhow::{Context, Result};
use crossbeam_channel::unbounded;
use image::Rgb;
use player::Mpv;
use std::{path::PathBuf, time::Duration};
use streamdeck::{DeckCommand, DeckEvent, StreamDeck};

fn main() -> Result<()> {
    env_logger::init();
    let clips: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    anyhow::ensure!(!clips.is_empty(), "usage: rpi-vj CLIP.mp4 [CLIP.mp4 ...]");

    let mut mpv = Mpv::start().context("start DRM/KMS mpv backend")?;
    let (event_tx, event_rx) = unbounded();
    let (command_tx, command_rx) = unbounded();
    let _deck = StreamDeck::spawn(event_tx, command_rx);

    for key in 0..32_u8 {
        let label = clips
            .get(usize::from(key))
            .and_then(|p| p.file_stem())
            .and_then(|s| s.to_str())
            .unwrap_or("EMPTY");
        command_tx.send(DeckCommand::SetIcon {
            key,
            jpeg: streamdeck::make_icon(label, Rgb([18, 22, 29]), Rgb([235, 240, 245]))?,
            active_jpeg: streamdeck::make_icon(label, Rgb([12, 92, 52]), Rgb([255, 255, 255]))?,
        })?;
    }

    loop {
        match event_rx.recv_timeout(Duration::from_millis(250)) {
            Ok(DeckEvent::Pressed(key)) => {
                if let Some(path) = clips.get(usize::from(key)) {
                    mpv.load(path)?;
                    command_tx.send(DeckCommand::Active(key))?;
                }
            }
            Ok(DeckEvent::Connected) => log::info!("Stream Deck XL connected"),
            Ok(DeckEvent::Disconnected) => log::warn!("Stream Deck disconnected; reconnecting"),
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => mpv.check()?,
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                anyhow::bail!("Stream Deck worker stopped")
            }
        }
    }
}
