use super::{StreamingSendFrameKind, COMMAND_BYTES};
use crate::app::module::common::streaming::commands::{OutgoingBytes, SendInput};
use std::{
    io::{Read, Write},
    mem::size_of,
    time::{Duration, Instant},
};
use tokio_tungstenite::tungstenite::{Error, Message, WebSocket};

pub(super) struct Command {
    kind: StreamingSendFrameKind,
    bytes: OutgoingBytes,
    log: bool,
}
impl Command {
    pub fn new<'a>(kind: StreamingSendFrameKind, bytes: impl Into<SendInput<'a>>, log: bool) -> Result<Self, String> {
        let bytes = OutgoingBytes::new(bytes, COMMAND_BYTES - size_of::<Self>())
            .map_err(|error| format!("WebSocket server (1 MiB command limit): {error}"))?;
        Ok(Self { kind, bytes, log })
    }
    pub fn weight(&self) -> usize {
        size_of::<Self>() + self.bytes.weight()
    }
}

#[cfg(test)]
mod tests;
#[derive(Clone, Copy, PartialEq, Eq)]
enum RecipientState {
    Pending,
    Confirmed,
    Failed,
}

struct Recipient {
    id: u64,
    submitted: bool,
    state: RecipientState,
    started: Option<Instant>,
}
pub(super) struct Broadcast {
    message: Message,
    recipients: Vec<Recipient>,
    next: usize,
    log: bool,
}
impl Broadcast {
    pub fn new(command: Command, ids: impl Iterator<Item = u64>) -> Result<Self, String> {
        let bytes = command.bytes.into_vec()?;
        let message = match command.kind {
            StreamingSendFrameKind::Binary => Message::Binary(bytes.into()),
            StreamingSendFrameKind::Text => Message::Text(
                String::from_utf8(bytes)
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
                    state: RecipientState::Pending,
                    started: None,
                })
                .collect(),
            next: 0,
            log: command.log,
        })
    }
    pub fn next_recipient(&mut self) -> Option<usize> {
        for _ in 0..self.recipients.len() {
            let index = self.next;
            self.next = (index + 1) % self.recipients.len();
            if self.recipients[index].state == RecipientState::Pending {
                return Some(index);
            }
        }
        None
    }
    pub fn id(&self, index: usize) -> u64 {
        self.recipients[index].id
    }
    pub fn fail(&mut self, index: usize) {
        self.recipients[index].state = RecipientState::Failed;
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
                recipient.state = RecipientState::Confirmed;
                Ok(())
            }
            Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(()),
            Err(error) => Err(format!(
                "WebSocket frame was not confirmed; partial native effect may remain, no replay: {error}"
            )),
        }
    }
    pub fn completion(self) -> Option<super::Event> {
        debug_assert!(self
            .recipients
            .iter()
            .all(|recipient| recipient.state != RecipientState::Pending));
        let confirmed = self
            .recipients
            .iter()
            .filter(|recipient| recipient.state == RecipientState::Confirmed)
            .count();
        let bytes = match &self.message {
            Message::Binary(bytes) => bytes.as_ref(),
            Message::Text(text) => text.as_bytes(),
            _ => unreachable!("broadcasts contain only data frames"),
        };
        (self.log && confirmed > 0).then(|| {
            super::Event::OutgoingLog(crate::app::module::common::streaming::outgoing::broadcast_log(
                "WebSocket server",
                bytes,
                confirmed,
                self.recipients.len(),
            ))
        })
    }
}
