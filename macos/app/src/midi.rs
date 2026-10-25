//! MIDI in and out: every input port the system has, each message handed to the app as
//! three bytes (status, number, value); and every output port, for the pad lights.
use std::sync::mpsc::{channel, Receiver, Sender};

pub struct Midi {
    _open: Vec<midir::MidiInputConnection<()>>,
    outputs: Vec<midir::MidiOutputConnection>,
    /// The ports being listened to, and the ports being lit.
    pub names: Vec<String>,
    pub output_names: Vec<String>,
    pub incoming: Receiver<[u8; 3]>,
    /// The sending end, for feeding messages in by hand.
    pub inject: Sender<[u8; 3]>,
    /// Messages sent to the outputs so far.
    pub sent: u64,
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
    let (mut outputs, mut output_names) = (Vec::new(), Vec::new());
    let count = midir::MidiOutput::new("RPi-VJ Desktop").map_or(0, |output| output.ports().len());
    for index in 0..count {
        let Ok(output) = midir::MidiOutput::new("RPi-VJ Desktop") else { continue };
        let ports = output.ports();
        let Some(port) = ports.get(index) else { continue };
        let name = output.port_name(port).unwrap_or_else(|_| format!("port {}", index + 1));
        // The software synthesiser that Windows ships would play the lights as notes.
        if name.contains("Wavetable") {
            continue;
        }
        if let Ok(connection) = output.connect(port, "rpi-vj-desktop-out") {
            outputs.push(connection);
            output_names.push(name);
        }
    }
    Midi { _open: open, outputs, names, output_names, incoming, inject, sent: 0 }
}
impl Midi {
    /// Send one message to every output.
    pub fn send(&mut self, message: [u8; 3]) {
        for output in &mut self.outputs {
            if output.send(&message).is_ok() {
                self.sent += 1;
            }
        }
    }
    /// Whether a device whose name holds `part` is connected, either way.
    pub fn has(&self, part: &str) -> bool {
        self.names.iter().chain(&self.output_names).any(|name| name.contains(part))
    }
}
