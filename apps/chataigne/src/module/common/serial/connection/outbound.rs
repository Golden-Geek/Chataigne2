use super::{Event, COMMAND_BYTES};
use std::{
    io::{self, Write},
    mem::size_of,
    time::{Duration, Instant},
};

const WRITE_BYTES: usize = 8192;
const WRITE_DEADLINE: Duration = Duration::from_secs(5);

pub(super) struct Outbound {
    pub generation: u64,
    bytes: Vec<u8>,
    offset: usize,
    started: Option<Instant>,
}
impl Outbound {
    pub fn new(bytes: &[u8], generation: u64) -> Result<Self, String> {
        if size_of::<Self>().saturating_add(bytes.len()) > COMMAND_BYTES {
            return Err("Serial command exceeds the 256 KiB retained-byte limit; command was not admitted".into());
        }
        Ok(Self {
            generation,
            bytes: bytes.to_vec(),
            offset: 0,
            started: None,
        })
    }
    pub fn weight(&self) -> usize {
        size_of::<Self>() + self.bytes.capacity()
    }
    pub fn write_turn(&mut self, port: &mut impl Write) -> Result<bool, String> {
        let started = *self.started.get_or_insert_with(Instant::now);
        if started.elapsed() >= WRITE_DEADLINE {
            return Err("Serial write deadline exceeded".into());
        }
        if self.offset == self.bytes.len() {
            return Ok(true);
        }
        let end = self.offset + (self.bytes.len() - self.offset).min(WRITE_BYTES);
        match port.write(&self.bytes[self.offset..end]) {
            Ok(0) => Err("Serial write made no progress".into()),
            Ok(written) if written <= end - self.offset => {
                self.offset += written;
                Ok(self.offset == self.bytes.len())
            }
            Ok(_) => Err("Serial driver reported an invalid write count".into()),
            Err(error) if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted) => Ok(false),
            // Windows WriteFile errors may hide partial effects; a timeout cannot be retried.
            Err(error) => Err(format!("Serial write failed: {error}")),
        }
    }
    pub fn failure(&self, reason: &str) -> Event {
        Event::Warning(format!("Serial {}-byte command was not confirmed ({} bytes confirmed); a partial native effect may remain and it will not be replayed: {reason}", self.bytes.len(), self.offset))
    }
}

#[cfg(test)]
mod tests;
