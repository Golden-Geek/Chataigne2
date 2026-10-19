use std::{
    io::{self, Write},
    time::{Duration, Instant},
};

const WRITE_BYTES: usize = 64 * 1024;
const WRITE_DEADLINE: Duration = Duration::from_secs(5);

struct Recipient {
    id: u64,
    offset: usize,
    done: bool,
}

/// One immutable payload and a fixed cohort captured when the native command starts.
/// Round-robin cursors keep a blocked peer from delaying writes to healthy peers.
pub(super) struct Broadcast {
    bytes: Vec<u8>,
    recipients: Vec<Recipient>,
    next: usize,
    started: Instant,
}
impl Broadcast {
    pub fn new(bytes: Vec<u8>, clients: impl Iterator<Item = u64>) -> Self {
        Self {
            bytes,
            recipients: clients
                .map(|id| Recipient {
                    id,
                    offset: 0,
                    done: false,
                })
                .collect(),
            next: 0,
            started: Instant::now(),
        }
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
            recipient.done = true;
            return Ok(());
        }
        let end = recipient.offset.saturating_add(WRITE_BYTES).min(self.bytes.len());
        match writer.write(&self.bytes[recipient.offset..end]) {
            Ok(0) => Err(io::ErrorKind::WriteZero.into()),
            Ok(count) if count <= end - recipient.offset => {
                recipient.offset += count;
                recipient.done = recipient.offset == self.bytes.len();
                Ok(())
            }
            Ok(_) => Err(io::ErrorKind::InvalidData.into()),
            Err(error) if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted) => Ok(()),
            Err(error) => Err(error),
        }
    }
}
#[cfg(test)]
mod tests;
