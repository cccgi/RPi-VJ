//! MIDI in: every input port the system has, each message handed to the app as three
//! bytes (status, number, value).
use std::sync::mpsc::{channel, Receiver, Sender};

pub struct Midi {
    _open: Vec<midir::MidiInputConnection<()>>,
    /// The ports being listened to.
    pub names: Vec<String>,
    pub incoming: Receiver<[u8; 3]>,
    /// The sending end, for feeding messages in by hand.
    pub inject: Sender<[u8; 3]>,
}
pub fn open() -> Midi {
    let (inject, incoming) = channel();
    let (mut open, mut names) = (Vec::new(), Vec::new());
    let count = midir::MidiInput::new("RPi-VJ Desktop").map_or(0, |input| input.ports().len());
    // A connection takes its client with it, so each port gets a client of its own.
    for index in 0..count {
        let Ok(mut input) = midir::MidiInput::new("RPi-VJ Desktop") else { continue };
        input.ignore(midir::Ignore::All);
        let ports = input.ports();
        let Some(port) = ports.get(index) else { continue };
        let name = input.port_name(port).unwrap_or_else(|_| format!("port {}", index + 1));
        let sender = inject.clone();
        let heard = move |_: u64, message: &[u8], _: &mut ()| {
            if message.len() >= 2 {
                let _ = sender.send([message[0], message[1], message.get(2).copied().unwrap_or(0)]);
            }
        };
        if let Ok(connection) = input.connect(port, "rpi-vj-desktop-in", heard, ()) {
            open.push(connection);
            names.push(name);
        }
    }
    Midi { _open: open, names, incoming, inject }
}
