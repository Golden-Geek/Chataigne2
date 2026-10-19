use super::{StreamingSendFrameKind, StreamingWorkerEvent, COMMAND_BYTES};
use std::{
    io::{Read, Write},
    mem::size_of,
    time::{Duration, Instant},
};
use tokio_tungstenite::tungstenite::{Error, Message, WebSocket};

const WRITE_DEADLINE: Duration = Duration::from_secs(5);

pub(super) struct Outbound {
    pub generation: u64,
    kind: StreamingSendFrameKind,
    bytes: Option<Vec<u8>>,
    message: Option<Message>,
    size: usize,
    log: bool,
    log_message: Option<String>,
    submitted: bool,
    started: Option<Instant>,
}
impl Outbound {
    pub fn new(kind: StreamingSendFrameKind, bytes: &[u8], generation: u64, log: bool) -> Result<Self, String> {
        if size_of::<Self>().saturating_add(bytes.len()) > COMMAND_BYTES {
            return Err("WebSocket command exceeds the 1 MiB retained-byte limit; command was not admitted".into());
        }
        Ok(Self {
            generation,
            kind,
            bytes: Some(bytes.to_vec()),
            message: None,
            size: bytes.len(),
            log,
            log_message: None,
            submitted: false,
            started: None,
        })
    }
    pub fn weight(&self) -> usize {
        size_of::<Self>() + self.bytes.as_ref().map_or(0, Vec::capacity)
    }
    pub fn prepare(&mut self) -> Result<(), String> {
        let bytes = self.bytes.take().expect("one native frame preparation");
        if self.log {
            self.log_message = Some(format!(
                "Sent WebSocket {} ({} bytes)",
                crate::app::module::common::streaming::module_helpers::format_bytes_for_log(
                    &bytes[..bytes.len().min(1024)]
                ),
                bytes.len()
            ));
        }
        self.message = Some(match self.kind {
            StreamingSendFrameKind::Binary => Message::Binary(bytes.into()),
            StreamingSendFrameKind::Text => Message::Text(
                String::from_utf8(bytes)
                    .map_err(|_| "WebSocket text payload is not valid UTF-8; frame was not sent".to_string())?
                    .into(),
            ),
        });
        Ok(())
    }
    /// Submit once, then flush only. WouldBlock retains Tungstenite's encoded frame.
    pub fn write_turn<S: Read + Write>(&mut self, socket: &mut WebSocket<S>) -> Result<bool, String> {
        let started = *self.started.get_or_insert_with(Instant::now);
        if started.elapsed() >= WRITE_DEADLINE {
            return Err("WebSocket frame write deadline exceeded".into());
        }
        if !self.submitted {
            let message = self.message.take().expect("prepared native frame");
            self.submitted = true;
            match socket.write(message) {
                Ok(()) => {}
                Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(false),
                Err(error) => return Err(format!("WebSocket frame submission failed: {error}")),
            }
        }
        match socket.flush() {
            Ok(()) => Ok(true),
            Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(false),
            Err(error) => Err(format!("WebSocket frame flush failed: {error}")),
        }
    }
    pub fn failure(&self, reason: &str) -> StreamingWorkerEvent {
        StreamingWorkerEvent::Error(format!("WebSocket {}-byte frame was not confirmed; a partial native effect may remain and it will not be replayed: {reason}", self.size))
    }
    pub fn completion(self) -> Option<StreamingWorkerEvent> {
        self.log_message.map(StreamingWorkerEvent::OutgoingLog)
    }
}

#[cfg(test)]
mod tests;
