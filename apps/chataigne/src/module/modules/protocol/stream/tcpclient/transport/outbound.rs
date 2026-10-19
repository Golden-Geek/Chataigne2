use super::{StreamingWorkerEvent, COMMAND_BYTES};
use std::{
    io::{self, Write},
    mem::size_of,
    time::{Duration, Instant},
};

const WRITE_BYTES_PER_TURN: usize = 64 * 1024;
const WRITE_DEADLINE: Duration = Duration::from_secs(5);

#[derive(Debug, PartialEq, Eq)]
pub(super) enum WriteTurn {
    Complete,
    Pending { progressed: bool },
}

#[derive(Debug)]
pub(super) struct Outbound {
    bytes: Vec<u8>,
    pub generation: u64,
    offset: usize,
    started: Option<Instant>,
    log: bool,
}

#[cfg(test)]
mod tests;

impl Outbound {
    pub(super) fn new(bytes: Vec<u8>, generation: u64, log: bool) -> Result<Self, String> {
        if size_of::<Self>().saturating_add(bytes.capacity()) > COMMAND_BYTES {
            return Err("TCP command exceeds the 1 MiB retained-byte limit; command was not admitted".into());
        }
        Ok(Self {
            bytes,
            generation,
            offset: 0,
            started: None,
            log,
        })
    }
    pub(super) fn weight(&self) -> usize {
        size_of::<Self>() + self.bytes.capacity()
    }
    pub(super) fn write_turn(&mut self, writer: &mut impl Write) -> io::Result<WriteTurn> {
        let started = *self.started.get_or_insert_with(Instant::now);
        if started.elapsed() >= WRITE_DEADLINE {
            return Err(io::ErrorKind::TimedOut.into());
        }
        if self.offset == self.bytes.len() {
            return Ok(WriteTurn::Complete);
        }
        let end = self.offset.saturating_add(WRITE_BYTES_PER_TURN).min(self.bytes.len());
        match writer.write(&self.bytes[self.offset..end]) {
            Ok(0) => Err(io::ErrorKind::WriteZero.into()),
            Ok(count) if count <= end - self.offset => {
                self.offset += count;
                Ok(if self.offset == self.bytes.len() {
                    WriteTurn::Complete
                } else {
                    WriteTurn::Pending { progressed: true }
                })
            }
            Ok(_) => Err(io::ErrorKind::InvalidData.into()),
            Err(error) if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted) => {
                Ok(WriteTurn::Pending { progressed: false })
            }
            Err(error) => Err(error),
        }
    }
    pub(super) fn failure(&self, reason: &str) -> StreamingWorkerEvent {
        StreamingWorkerEvent::Error(format!(
            "TCP command failed after {}/{} bytes; it will not be replayed: {reason}",
            self.offset,
            self.bytes.len()
        ))
    }
    pub(super) fn completion(self) -> Option<StreamingWorkerEvent> {
        self.log.then(|| {
            let prefix = &self.bytes[..self.bytes.len().min(1024)];
            StreamingWorkerEvent::OutgoingLog(format!(
                "Sent TCP {} ({} bytes)",
                crate::app::module::common::streaming::module_helpers::format_bytes_for_log(prefix),
                self.bytes.len()
            ))
        })
    }
}
