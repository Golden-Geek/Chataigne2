use super::{outbound::Broadcast, Event, ReplySource};
use crate::app::module::common::streaming::websocket::socket::WireSocket;
use crate::app::module::{
    common::streaming::{
        input::{self, InputPlan},
        native_replies::NativeReply,
        parser::StreamingParser,
    },
    script_api::NativeCallbackPayload,
};
use std::{
    net::Shutdown,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio_tungstenite::tungstenite::{Error, Message};

pub(super) struct Peer {
    pub id: u64,
    label: String,
    socket: WireSocket,
    control_since: Option<Instant>,
    closing: bool,
    parser: StreamingParser,
    input_generation: Option<u64>,
}
pub(super) struct Peers {
    pub entries: Vec<Peer>,
    count: Arc<AtomicUsize>,
    next_id: u64,
    next_read: usize,
}
impl Peers {
    pub fn new(count: Arc<AtomicUsize>) -> Self {
        Self {
            entries: Vec::new(),
            count,
            next_id: 0,
            next_read: 0,
        }
    }
    pub fn reset(&mut self) {
        for peer in &mut self.entries {
            peer.socket.get_mut().reset();
        }
    }
    pub fn add(&mut self, socket: WireSocket, label: String, info: String) -> Event {
        self.next_id = self
            .next_id
            .checked_add(1)
            .expect("WebSocket native client identity exhausted");
        self.entries.push(Peer {
            id: self.next_id,
            label: label.clone(),
            socket,
            control_since: None,
            closing: false,
            parser: StreamingParser::default(),
            input_generation: None,
        });
        self.count.store(self.entries.len(), Ordering::Release);
        let callback = NativeCallbackPayload::prepare(
            "clientConnected",
            vec![serde_json::json!(label), serde_json::json!(info)],
        );
        Event::ClientConnected {
            client_id: label,
            info,
            callback,
        }
    }
    pub fn remove(&mut self, index: usize, reason: Option<String>) -> Event {
        let mut peer = self.entries.remove(index);
        self.count.store(self.entries.len(), Ordering::Release);
        let _ = peer.socket.get_ref().stream.shutdown(Shutdown::Both);
        peer.parser.discard_partial_input();
        let rejected_lines = peer.parser.take_rejected_line_count();
        let callback = NativeCallbackPayload::prepare(
            "clientDisconnected",
            vec![serde_json::json!(peer.label), serde_json::json!(reason)],
        );
        Event::ClientDisconnected {
            client_id: peer.label,
            reason,
            rejected_lines,
            callback,
        }
    }
    pub fn write(&mut self, command: &mut Broadcast, recipient: usize) -> Option<Event> {
        let id = command.id(recipient);
        let Some(index) = self.entries.iter().position(|peer| peer.id == id && !peer.closing) else {
            command.finish(recipient);
            return Some(Event::Error(format!(
                "WebSocket broadcast original peer {id} is unavailable; frame will not be replayed"
            )));
        };
        match command.write_turn(recipient, &mut self.entries[index].socket) {
            Ok(()) => None,
            Err(error) => {
                command.finish(recipient);
                Some(self.remove(index, Some(error)))
            }
        }
    }
    pub fn receive(&mut self, receive_enabled: bool, input: &InputPlan) -> Option<Event> {
        if self.entries.is_empty() {
            return None;
        }
        let index = self.next_read % self.entries.len();
        self.next_read = self.next_read.wrapping_add(1);
        let peer = &mut self.entries[index];
        if let Some(started) = peer.control_since {
            let limit = if peer.closing {
                Duration::from_secs(1)
            } else {
                Duration::from_secs(5)
            };
            match peer.socket.flush() {
                Ok(()) if peer.closing => return Some(self.remove(index, None)),
                Ok(()) => peer.control_since = None,
                Err(Error::ConnectionClosed | Error::AlreadyClosed) => return Some(self.remove(index, None)),
                Err(Error::Io(error))
                    if error.kind() == std::io::ErrorKind::WouldBlock && started.elapsed() < limit =>
                {
                    return None
                }
                Err(error) => {
                    return Some(self.remove(index, Some(format!("WebSocket control flush failed: {error}"))))
                }
            }
        }
        // Control frames must progress even when application input is disabled.
        match peer.socket.read() {
            Ok(message) => match message {
                Message::Text(text) if receive_enabled => Some(peer.prepare(input, text.as_bytes(), true)),
                Message::Binary(bytes) if receive_enabled => Some(peer.prepare(input, &bytes, false)),
                Message::Ping(_) => {
                    peer.control_since.get_or_insert_with(Instant::now);
                    None
                }
                Message::Close(_) => {
                    peer.closing = true;
                    peer.control_since.get_or_insert_with(Instant::now);
                    None
                }
                _ => None,
            },
            Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::WouldBlock => None,
            Err(Error::ConnectionClosed | Error::AlreadyClosed) => Some(self.remove(index, None)),
            Err(error) => Some(self.remove(index, Some(format!("WebSocket input failed: {error}")))),
        }
    }
    pub fn retire_one(&mut self) -> Option<Event> {
        let peer = self.entries.first_mut()?;
        let started = *peer.control_since.get_or_insert_with(Instant::now);
        let blocked =
            matches!(peer.socket.close(None), Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::WouldBlock);
        if blocked && started.elapsed() < Duration::from_secs(1) {
            None
        } else {
            Some(self.remove(0, None))
        }
    }
}
impl Peer {
    fn prepare(&mut self, plan: &InputPlan, bytes: &[u8], text_hint: bool) -> Event {
        let snapshot = plan.snapshot();
        if self.input_generation != Some(snapshot.generation) {
            self.parser.discard_partial_input();
            self.input_generation = Some(snapshot.generation);
        }
        let event = match input::prepare(
            &mut self.parser,
            &snapshot,
            bytes,
            "WebSocket server",
            Some(&self.label),
            text_hint,
        ) {
            Ok(input) => Event::Input(input),
            Err(error) => Event::Error(format!("WebSocket server input was not admitted: {error}")),
        };
        if event.weight() > Event::limit(ReplySource::Input) {
            self.parser.discard_partial_input();
            Event::oversized(ReplySource::Input)
        } else {
            event
        }
    }
}
impl Drop for Peers {
    fn drop(&mut self) {
        self.count.store(0, Ordering::Release);
        for peer in &self.entries {
            let _ = peer.socket.get_ref().stream.shutdown(Shutdown::Both);
        }
    }
}

#[cfg(test)]
mod tests;
