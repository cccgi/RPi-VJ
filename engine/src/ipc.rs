//! Versioned, bounded local IPC. Controller owns desired state; renderer only reports facts.
use crate::control::State;
#[cfg(unix)]
use anyhow::Context;
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};

pub const MAX_FRAME: usize = 64 * 1024;
pub const VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Playing {
    pub clip: usize,
    pub generation: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Failure {
    pub generation: u64,
    pub reason: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShaderFailure {
    pub shader: usize,
    pub reason: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Message {
    Snapshot {
        session: u64,
        revision: u64,
        state: State,
    },
    Report {
        session: u64,
        frame: u64,
        playing: Option<Playing>,
        failed: Option<Failure>,
        shader_errors: Vec<ShaderFailure>,
        /// Song sections the renderer has counted since it started.
        #[serde(default)]
        sections: u32,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    version: u32,
    message: Message,
}

pub fn encode(message: &Message) -> Result<Vec<u8>> {
    let payload = serde_json::to_vec(&Envelope {
        version: VERSION,
        message: message.clone(),
    })?;
    ensure!(payload.len() <= MAX_FRAME, "IPC message exceeds limit");
    let mut bytes = Vec::with_capacity(payload.len() + 4);
    bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    bytes.extend(payload);
    Ok(bytes)
}

#[derive(Default)]
pub struct Decoder {
    bytes: Vec<u8>,
}
impl Decoder {
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<Message>> {
        ensure!(
            self.bytes.len() + bytes.len() <= MAX_FRAME * 2 + 8,
            "IPC input backlog exceeded"
        );
        self.bytes.extend_from_slice(bytes);
        let mut messages = Vec::new();
        let mut consumed = 0;
        while self.bytes.len() - consumed >= 4 {
            let prefix = &self.bytes[consumed..consumed + 4];
            let size = u32::from_le_bytes([prefix[0], prefix[1], prefix[2], prefix[3]]) as usize;
            ensure!(
                size > 0 && size <= MAX_FRAME,
                "invalid IPC frame length {size}"
            );
            if self.bytes.len() - consumed < size + 4 {
                break;
            }
            let envelope: Envelope =
                serde_json::from_slice(&self.bytes[consumed + 4..consumed + 4 + size])?;
            ensure!(
                envelope.version == VERSION,
                "unsupported IPC protocol version"
            );
            messages.push(envelope.message);
            ensure!(
                messages.len() <= 128,
                "too many IPC messages in one receive batch"
            );
            consumed += size + 4;
        }
        self.bytes.drain(..consumed);
        Ok(messages)
    }
}

/// Complete the current frame before replacing the queued latest state/report.
/// Dropping a partially written frame would corrupt the stream framing.
#[derive(Default)]
pub struct Outbox {
    current: Vec<u8>,
    offset: usize,
    pending: Option<Vec<u8>>,
}
impl Outbox {
    pub fn queue(&mut self, message: &Message) -> Result<()> {
        let bytes = encode(message)?;
        if self.current.is_empty() {
            self.current = bytes;
            self.offset = 0;
        } else {
            self.pending = Some(bytes);
        }
        Ok(())
    }
    pub fn flush(&mut self, writer: &mut impl std::io::Write) -> Result<()> {
        // At most two frames per invocation; never block caller on a slow peer.
        for _ in 0..2 {
            while self.offset < self.current.len() {
                match writer.write(&self.current[self.offset..]) {
                    Ok(0) => anyhow::bail!("IPC peer stopped accepting output"),
                    Ok(count) => self.offset += count,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(error) => return Err(error.into()),
                }
            }
            self.current = self.pending.take().unwrap_or_default();
            self.offset = 0;
            if self.current.is_empty() {
                break;
            }
        }
        Ok(())
    }
}

#[cfg(unix)]
pub struct Connection {
    stream: std::os::unix::net::UnixStream,
    decoder: Decoder,
    outbox: Outbox,
}
#[cfg(unix)]
impl Connection {
    pub fn new(stream: std::os::unix::net::UnixStream) -> Result<Self> {
        stream.set_nonblocking(true)?;
        Ok(Self {
            stream,
            decoder: Decoder::default(),
            outbox: Outbox::default(),
        })
    }
    pub fn connect(path: &std::path::Path) -> Result<Self> {
        Self::new(
            std::os::unix::net::UnixStream::connect(path).context("connect controller socket")?,
        )
    }
    pub fn queue(&mut self, message: &Message) -> Result<()> {
        self.outbox.queue(message)
    }
    pub fn poll(&mut self) -> Result<Vec<Message>> {
        use std::io::Read;
        self.outbox.flush(&mut self.stream)?;
        let mut messages = Vec::new();
        let mut buffer = [0_u8; 4096];
        for _ in 0..4 {
            match self.stream.read(&mut buffer) {
                Ok(0) => anyhow::bail!("IPC peer disconnected"),
                Ok(count) => messages.extend(self.decoder.push(&buffer[..count])?),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Ok(messages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn report(frame: u64) -> Message {
        Message::Report {
            session: 7,
            frame,
            playing: None,
            failed: None,
            shader_errors: Vec::new(),
            sections: 0,
        }
    }
    #[test]
    fn handles_fragmented_and_coalesced_frames() {
        let first = encode(&report(1)).unwrap();
        let second = encode(&report(2)).unwrap();
        let mut decoder = Decoder::default();
        for byte in &first[..first.len() - 1] {
            assert!(decoder.push(&[*byte]).unwrap().is_empty());
        }
        let mut tail = vec![first[first.len() - 1]];
        tail.extend(second);
        assert_eq!(decoder.push(&tail).unwrap().len(), 2);
    }
    #[test]
    fn rejects_large_or_unknown_frames() {
        assert!(Decoder::default().push(&u32::MAX.to_le_bytes()).is_err());
        let body = br#"{"version":99,"message":{"kind":"Report","session":0,"frame":0,"playing":null,"failed":null,"shader_errors":[]}}"#;
        let mut bytes = (body.len() as u32).to_le_bytes().to_vec();
        bytes.extend(body);
        assert!(Decoder::default().push(&bytes).is_err());
    }
    #[test]
    fn latest_wins_without_corrupting_partial_write() {
        struct Slow {
            bytes: Vec<u8>,
            block: bool,
        }
        impl std::io::Write for Slow {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                if self.block {
                    return Err(std::io::ErrorKind::WouldBlock.into());
                }
                let n = bytes.len().min(3);
                self.bytes.extend_from_slice(&bytes[..n]);
                self.block = true;
                Ok(n)
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut outbox = Outbox::default();
        let mut slow = Slow {
            bytes: Vec::new(),
            block: false,
        };
        outbox.queue(&report(1)).unwrap();
        outbox.flush(&mut slow).unwrap();
        outbox.queue(&report(2)).unwrap();
        outbox.queue(&report(3)).unwrap();
        for _ in 0..200 {
            slow.block = false;
            outbox.flush(&mut slow).unwrap();
        }
        let messages = Decoder::default().push(&slow.bytes).unwrap();
        assert_eq!(messages.len(), 2);
        assert!(matches!(messages[1], Message::Report { frame: 3, .. }));
    }
}
