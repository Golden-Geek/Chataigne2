use super::{outbound::Broadcast, Event, StreamingSendFrameKind};
use crate::app::module::common::streaming::websocket::socket::WireSocket;
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
        });
        self.count.store(self.entries.len(), Ordering::Release);
        Event::ClientConnected { client_id: label, info }
    }
    pub fn remove(&mut self, index: usize, reason: Option<String>) -> Event {
        let peer = self.entries.remove(index);
        self.count.store(self.entries.len(), Ordering::Release);
        let _ = peer.socket.get_ref().stream.shutdown(Shutdown::Both);
        Event::ClientDisconnected {
            client_id: peer.label,
            reason,
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
    pub fn receive(&mut self, receive_enabled: bool) -> Option<Event> {
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
            Ok(message) => {
                let frame = match message {
                    Message::Text(text) if receive_enabled => {
                        Some((StreamingSendFrameKind::Text, text.as_bytes().to_vec()))
                    }
                    Message::Binary(bytes) if receive_enabled => Some((StreamingSendFrameKind::Binary, bytes.to_vec())),
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
                };
                frame.map(|(frame_kind, bytes)| Event::Bytes {
                    client_id: peer.label.clone(),
                    frame_kind,
                    bytes,
                })
            }
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
