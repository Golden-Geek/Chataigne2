use super::{Event, COMMAND_BYTES};
use crate::app::module::common::streaming::commands::{OutgoingBytes, SendInput};
use std::{
    io::{self, Write},
    mem::size_of,
    time::{Duration, Instant},
};

const WRITE_BYTES: usize = 8192;
const WRITE_DEADLINE: Duration = Duration::from_secs(5);

pub(super) struct Outbound {
    pub generation: u64,
    bytes: OutgoingBytes,
    offset: usize,
    started: Option<Instant>,
    log: bool,
}
impl Outbound {
    pub fn new<'a>(bytes: impl Into<SendInput<'a>>, generation: u64, log: bool) -> Result<Self, String> {
        let bytes = OutgoingBytes::new(bytes, COMMAND_BYTES - size_of::<Self>())
            .map_err(|error| format!("Serial (256 KiB command limit): {error}"))?;
        Ok(Self {
            generation,
            bytes,
            offset: 0,
            started: None,
            log,
        })
    }
    pub fn prepare(&mut self) -> Result<(), String> {
        self.bytes.prepare()
    }
    pub fn weight(&self) -> usize {
        size_of::<Self>() + self.bytes.weight()
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
        if self.bytes.encoded_len().is_none() {
            return Event::Warning(format!("Serial script command failed before encoding completed; it was not sent and will not be replayed: {reason}"));
        }
        Event::Warning(format!("Serial {}-byte command was not confirmed ({} bytes confirmed); a partial native effect may remain and it will not be replayed: {reason}", self.bytes.len(), self.offset))
    }
    pub fn completion(self) -> Option<Event> {
        debug_assert_eq!(self.offset, self.bytes.len());
        self.log.then(|| {
            Event::OutgoingLog(crate::app::module::common::streaming::outgoing::sent_log(
                "serial",
                &self.bytes,
            ))
        })
    }
}

#[cfg(test)]
mod tests;
