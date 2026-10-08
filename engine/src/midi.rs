//! Raw MIDI link to the X-Touch Mini through the kernel rawmidi device; no MIDI library.
//! The port is optional: it is found by card name, reopened after an unplug, and the
//! controller runs unchanged without it.
use crate::xtouch::{Event, Parser};
use crossbeam_channel::{bounded, Receiver};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

pub struct Midi {
    /// `(channel, event)` as received.
    pub events: Receiver<(u8, Event)>,
    port: Arc<Mutex<Option<File>>>,
    connections: Arc<AtomicU64>,
}
impl Midi {
    /// Rises each time the unit is (re)opened; rings and LEDs must then be sent again.
    pub fn connections(&self) -> u64 {
        self.connections.load(Ordering::Relaxed)
    }
    /// Best effort: a failed write drops the message, the reader notices a dead port.
    pub fn send(&self, bytes: &[u8]) {
        if let Ok(mut port) = self.port.lock() {
            if let Some(file) = port.as_mut() {
                let _ = file.write_all(bytes);
            }
        }
    }
}

/// `/dev/snd/midiC<n>D0` of the first sound card whose name mentions the X-Touch.
fn find() -> Option<PathBuf> {
    let cards = std::fs::read_to_string("/proc/asound/cards").ok()?;
    let line = cards
        .lines()
        .find(|line| line.to_ascii_lowercase().contains("x-touch") && line.contains('['))?;
    let card: u32 = line.split_whitespace().next()?.parse().ok()?;
    Some(PathBuf::from(format!("/dev/snd/midiC{card}D0")))
}

pub fn start() -> Midi {
    let (tx, events) = bounded(256);
    let port = Arc::new(Mutex::new(None));
    let connections = Arc::new(AtomicU64::new(0));
    let (shared, count) = (Arc::clone(&port), Arc::clone(&connections));
    let spawned = std::thread::Builder::new().name("midi".into()).spawn(move || {
        let mut reported = false;
        loop {
            let opened = find().and_then(|path| {
                let file = OpenOptions::new().read(true).write(true).open(&path);
                match file {
                    Ok(file) => Some((path, file)),
                    Err(error) => {
                        if !reported {
                            eprintln!("midi: cannot open {}: {error}", path.display());
                            reported = true;
                        }
                        None
                    }
                }
            });
            let Some((path, mut file)) = opened else {
                std::thread::sleep(Duration::from_secs(1));
                continue;
            };
            eprintln!("midi: X-Touch Mini at {}", path.display());
            reported = false;
            if let Ok(mut slot) = shared.lock() {
                *slot = file.try_clone().ok();
            }
            count.fetch_add(1, Ordering::Relaxed);
            let mut parser = Parser::default();
            let mut buffer = [0u8; 64];
            loop {
                match file.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(read) => {
                        for byte in &buffer[..read] {
                            if let Some(event) = parser.push(*byte) {
                                // A full queue means the controller is not keeping up; drop.
                                let _ = tx.try_send(event);
                            }
                        }
                    }
                }
            }
            eprintln!("midi: X-Touch Mini disconnected");
            if let Ok(mut slot) = shared.lock() {
                *slot = None;
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    });
    if let Err(error) = spawned {
        eprintln!("midi: no reader thread: {error}");
    }
    Midi {
        events,
        port,
        connections,
    }
}
