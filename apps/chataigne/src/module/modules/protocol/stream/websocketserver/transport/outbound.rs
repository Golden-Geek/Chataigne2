use super::{StreamingSendFrameKind, COMMAND_BYTES};
use std::{
    io::{Read, Write},
    mem::size_of,
    time::{Duration, Instant},
};
use tokio_tungstenite::tungstenite::{Error, Message, WebSocket};

pub(super) struct Command {
    kind: StreamingSendFrameKind,
    bytes: Vec<u8>,
}
impl Command {
    pub fn new(kind: StreamingSendFrameKind, bytes: &[u8]) -> Result<Self, String> {
        if size_of::<Self>().saturating_add(bytes.len()) > COMMAND_BYTES {
            return Err("WebSocket server command exceeds 1 MiB; command was not admitted".into());
        }
        Ok(Self {
            kind,
            bytes: bytes.to_vec(),
        })
    }
    pub fn weight(&self) -> usize {
        size_of::<Self>() + self.bytes.capacity()
    }
}

#[cfg(test)]
mod tests;
struct Recipient {
    id: u64,
    submitted: bool,
    done: bool,
    started: Option<Instant>,
}
pub(super) struct Broadcast {
    message: Message,
    recipients: Vec<Recipient>,
    next: usize,
}
impl Broadcast {
    pub fn new(command: Command, ids: impl Iterator<Item = u64>) -> Result<Self, String> {
        let message = match command.kind {
            StreamingSendFrameKind::Binary => Message::Binary(command.bytes.into()),
            StreamingSendFrameKind::Text => Message::Text(
                String::from_utf8(command.bytes)
                    .map_err(|_| "WebSocket text payload is not valid UTF-8; frame was not sent")?
                    .into(),
            ),
        };
        Ok(Self {
            message,
            recipients: ids
                .map(|id| Recipient {
                    id,
                    submitted: false,
                    done: false,
                    started: None,
                })
                .collect(),
            next: 0,
        })
    }
    pub fn next_recipient(&mut self) -> Option<usize> {
        for _ in 0..self.recipients.len() {
            let index = self.next;
            self.next = (index + 1) % self.recipients.len();
            if !self.recipients[index].done {
                return Some(index);
            }
        }
        None
    }
    pub fn id(&self, index: usize) -> u64 {
        self.recipients[index].id
    }
    pub fn finish(&mut self, index: usize) {
        self.recipients[index].done = true;
    }
    /// Tungstenite retains a frame on WouldBlock. Never submit that frame a second time.
    /// Message clones share Bytes/Utf8Bytes rather than duplicating a payload per peer.
    pub fn write_turn<S: Read + Write>(&mut self, index: usize, socket: &mut WebSocket<S>) -> Result<(), String> {
        let recipient = &mut self.recipients[index];
        if recipient.started.get_or_insert_with(Instant::now).elapsed() >= Duration::from_secs(5) {
            return Err("WebSocket frame write deadline exceeded; partial native effect may remain, no replay".into());
        }
        let result = if recipient.submitted {
            socket.flush()
        } else {
            recipient.submitted = true;
            socket.write(self.message.clone()).and_then(|()| socket.flush())
        };
        match result {
            Ok(()) => {
                recipient.done = true;
                Ok(())
            }
            Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(()),
            Err(error) => Err(format!(
                "WebSocket frame was not confirmed; partial native effect may remain, no replay: {error}"
            )),
        }
    }
}
