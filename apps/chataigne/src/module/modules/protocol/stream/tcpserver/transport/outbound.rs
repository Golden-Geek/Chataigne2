use std::{
    io::{self, Write},
    time::{Duration, Instant},
};

const WRITE_BYTES: usize = 64 * 1024;
const WRITE_DEADLINE: Duration = Duration::from_secs(5);

pub(super) struct Command {
    pub bytes: Vec<u8>,
    pub log: bool,
}
impl Command {
    pub fn new(bytes: &[u8], log: bool) -> Result<Self, String> {
        if std::mem::size_of::<Self>().saturating_add(bytes.len()) > super::COMMAND_BYTES {
            return Err("TCP server command exceeds 1 MiB; command was not admitted".into());
        }
        Ok(Self {
            bytes: bytes.to_vec(),
            log,
        })
    }
    pub fn weight(&self) -> usize {
        std::mem::size_of::<Self>() + self.bytes.capacity()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RecipientState {
    Pending,
    Confirmed,
    Failed,
}

struct Recipient {
    id: u64,
    offset: usize,
    state: RecipientState,
}

/// One immutable payload and a fixed cohort captured when the native command starts.
/// Round-robin cursors keep a blocked peer from delaying writes to healthy peers.
pub(super) struct Broadcast {
    bytes: Vec<u8>,
    recipients: Vec<Recipient>,
    next: usize,
    started: Instant,
    log: bool,
}
impl Broadcast {
    pub fn new(bytes: Vec<u8>, clients: impl Iterator<Item = u64>, log: bool) -> Self {
        Self {
            bytes,
            recipients: clients
                .map(|id| Recipient {
                    id,
                    offset: 0,
                    state: RecipientState::Pending,
                })
                .collect(),
            next: 0,
            started: Instant::now(),
            log,
        }
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
    pub fn failure(&self, index: usize, reason: &str) -> String {
        format!(
            "TCP server command failed after {}/{} bytes for its original client; it will not be replayed: {reason}",
            self.recipients[index].offset,
            self.bytes.len()
        )
    }
    pub fn write_turn(&mut self, index: usize, writer: &mut impl Write) -> io::Result<()> {
        let recipient = &mut self.recipients[index];
        if self.started.elapsed() >= WRITE_DEADLINE {
            return Err(io::ErrorKind::TimedOut.into());
        }
        if recipient.offset == self.bytes.len() {
            recipient.state = RecipientState::Confirmed;
            return Ok(());
        }
        let end = recipient.offset.saturating_add(WRITE_BYTES).min(self.bytes.len());
        match writer.write(&self.bytes[recipient.offset..end]) {
            Ok(0) => Err(io::ErrorKind::WriteZero.into()),
            Ok(count) if count <= end - recipient.offset => {
                recipient.offset += count;
                recipient.state = if recipient.offset == self.bytes.len() {
                    RecipientState::Confirmed
                } else {
                    RecipientState::Pending
                };
                Ok(())
            }
            Ok(_) => Err(io::ErrorKind::InvalidData.into()),
            Err(error) if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted) => Ok(()),
            Err(error) => Err(error),
        }
    }
    pub fn completion(self) -> Option<super::TcpServerWorkerEvent> {
        debug_assert!(self
            .recipients
            .iter()
            .all(|recipient| recipient.state != RecipientState::Pending));
        let confirmed = self
            .recipients
            .iter()
            .filter(|recipient| recipient.state == RecipientState::Confirmed)
            .count();
        (self.log && confirmed > 0).then(|| {
            super::TcpServerWorkerEvent::OutgoingLog(crate::app::module::common::streaming::outgoing::broadcast_log(
                "TCP server",
                &self.bytes,
                confirmed,
                self.recipients.len(),
            ))
        })
    }
}
#[cfg(test)]
mod tests;
