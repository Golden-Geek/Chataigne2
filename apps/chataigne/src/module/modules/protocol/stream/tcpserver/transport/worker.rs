use super::{outbound::Broadcast, ReplySource, TcpServerTransportConfig, TcpServerWorkerEvent as Event, MAX_CLIENTS};
use crate::app::module::common::streaming::native_replies::NativeReplies;
use golden_io::{PendingDrainState, ReconnectBackoff, WorkerContext};
use std::{
    io::Read,
    net::{Shutdown, TcpListener, TcpStream, ToSocketAddrs},
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

const POLL: Duration = Duration::from_millis(5);
const IO_TURN: usize = 4;
type Context = WorkerContext<Vec<u8>, Event>;

struct Client {
    id: u64,
    label: String,
    stream: TcpStream,
}
struct Clients {
    entries: Vec<Client>,
    count: Arc<AtomicUsize>,
    next_id: u64,
    read_index: usize,
}
impl Clients {
    fn publish_count(&self) {
        self.count.store(self.entries.len(), Ordering::Release);
    }
    fn remove(&mut self, index: usize, reason: Option<String>) -> Event {
        let client = self.entries.remove(index);
        let _ = client.stream.shutdown(Shutdown::Both);
        self.publish_count();
        Event::ClientDisconnected {
            client_id: client.label,
            reason,
        }
    }
    fn accept(&mut self, stream: TcpStream, address: std::net::SocketAddr) -> Option<Event> {
        if self.entries.len() >= MAX_CLIENTS {
            let _ = stream.shutdown(Shutdown::Both);
            return None;
        }
        if let Err(error) = stream.set_nonblocking(true) {
            return Some(Event::Error(format!("TCP server client setup failed: {error}")));
        }
        let Some(id) = self.next_id.checked_add(1) else {
            let _ = stream.shutdown(Shutdown::Both);
            return Some(Event::Error("TCP server connection identity exhausted".into()));
        };
        self.next_id = id;
        let client_id = address.to_string();
        let info = format!("Remote address: {address}");
        self.entries.push(Client {
            id,
            label: client_id.clone(),
            stream,
        });
        self.publish_count();
        Some(Event::ClientConnected { client_id, info })
    }
    fn receive(&mut self) -> Option<Event> {
        if self.entries.is_empty() {
            return None;
        }
        let index = self.read_index % self.entries.len();
        self.read_index = index + 1;
        let mut buffer = [0; 8192];
        match self.entries[index].stream.read(&mut buffer) {
            Ok(0) => Some(self.remove(index, None)),
            Ok(length) => Some(Event::Bytes {
                client_id: self.entries[index].label.clone(),
                bytes: buffer[..length].to_vec(),
            }),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) =>
            {
                None
            }
            Err(error) => Some(self.remove(index, Some(format!("TCP server receive failed: {error}")))),
        }
    }
    fn write(&mut self, command: &mut Broadcast, recipient: usize) -> Option<Event> {
        let Some(index) = self
            .entries
            .iter()
            .position(|client| client.id == command.id(recipient))
        else {
            command.finish(recipient);
            return Some(Event::Error(
                command.failure(recipient, "the original connection is unavailable"),
            ));
        };
        match command.write_turn(recipient, &mut self.entries[index].stream) {
            Ok(()) => None,
            Err(error) => {
                let reason = command.failure(recipient, &error.to_string());
                command.finish(recipient);
                Some(self.remove(index, Some(reason)))
            }
        }
    }
}
impl Drop for Clients {
    fn drop(&mut self) {
        self.count.store(0, Ordering::Release);
        for client in &self.entries {
            let _ = client.stream.shutdown(Shutdown::Both);
        }
    }
}

pub(super) fn bind(config: &TcpServerTransportConfig) -> Result<TcpListener, String> {
    let address = (config.bind_host.trim(), config.bind_port)
        .to_socket_addrs()
        .map_err(|_| "TCP server bind address could not be resolved".to_string())?
        .next()
        .ok_or("TCP server bind address resolved to no endpoints")?;
    let listener = TcpListener::bind(address).map_err(|error| format!("TCP server bind failed: {error}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("TCP server listener setup failed: {error}"))?;
    Ok(listener)
}

pub(super) fn run(
    context: Context,
    config: TcpServerTransportConfig,
    count: Arc<AtomicUsize>,
    mut binder: impl FnMut(&TcpServerTransportConfig) -> Result<TcpListener, String>,
) {
    let mut clients = Clients {
        entries: Vec::new(),
        count,
        next_id: 0,
        read_index: 0,
    };
    let mut listener = None;
    let mut retry_at = Instant::now();
    let mut backoff = ReconnectBackoff::new(Duration::from_millis(250), Duration::from_secs(5));
    let mut replies = NativeReplies::<Event>::default();
    let mut current: Option<Broadcast> = None;
    let mut closed = false;
    loop {
        if context.cancellation().is_cancelled() || !replies.flush(&context) {
            return;
        }
        // Reply congestion pauses only the producer whose previous result is retained.
        for _ in 0..IO_TURN {
            if !replies.permits(ReplySource::Output) {
                break;
            }
            if current.is_none() {
                let mut commands = Vec::with_capacity(1);
                let turn = context.drain_commands(&mut commands, NonZeroUsize::MIN);
                closed = turn.state == PendingDrainState::Disconnected;
                if let Some(bytes) = commands.pop() {
                    if clients.entries.is_empty() {
                        let event =
                            Event::Error("TCP server command has no remaining clients; it will not be replayed".into());
                        if !replies.offer(&context, ReplySource::Output, event) {
                            return;
                        }
                        continue;
                    }
                    current = Some(Broadcast::new(bytes, clients.entries.iter().map(|client| client.id)));
                }
            }
            let Some(command) = current.as_mut() else { break };
            let Some(recipient) = command.next_recipient() else {
                current = None;
                continue;
            };
            if let Some(event) = clients.write(command, recipient) {
                if !replies.offer(&context, ReplySource::Output, event) {
                    return;
                }
            }
        }
        if closed && current.is_none() {
            replies.finish(&context);
            return;
        }
        if !context.command_admission_closed() {
            if listener.is_none() && Instant::now() >= retry_at && replies.permits(ReplySource::Status) {
                let bound = binder(&config);
                if context.cancellation().is_cancelled() {
                    return;
                }
                if context.command_admission_closed() {
                    continue;
                }
                let event = match bound.and_then(|socket| {
                    socket
                        .local_addr()
                        .map(|address| (socket, address))
                        .map_err(|error| error.to_string())
                }) {
                    Ok((socket, address)) => {
                        listener = Some(socket);
                        backoff.reset();
                        Event::Listening(address)
                    }
                    Err(error) => {
                        retry_at = backoff.schedule(Instant::now());
                        Event::Recovering(bounded_error(error))
                    }
                };
                if !replies.offer(&context, ReplySource::Status, event) {
                    return;
                }
            }
            if let Some(socket) = &listener {
                for _ in 0..IO_TURN {
                    if !replies.permits(ReplySource::Status) {
                        break;
                    }
                    match socket.accept() {
                        Ok((stream, address)) => {
                            if let Some(event) = clients.accept(stream, address) {
                                if !replies.offer(&context, ReplySource::Status, event) {
                                    return;
                                }
                            }
                        }
                        Err(error)
                            if matches!(
                                error.kind(),
                                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                            ) =>
                        {
                            break
                        }
                        Err(error) => {
                            listener = None;
                            retry_at = backoff.schedule(Instant::now());
                            if !replies.offer(
                                &context,
                                ReplySource::Status,
                                Event::Recovering(format!("TCP server accept failed: {error}")),
                            ) {
                                return;
                            }
                            break;
                        }
                    }
                }
            }
            if config.receive_enabled {
                for _ in 0..IO_TURN {
                    if !replies.permits(ReplySource::Input) {
                        break;
                    }
                    if let Some(event) = clients.receive() {
                        if !replies.offer(&context, ReplySource::Input, event) {
                            return;
                        }
                    }
                }
            }
        }
        context.cancellation().wait(POLL);
    }
}

fn bounded_error(error: String) -> String {
    if error.capacity() <= 4096 {
        error
    } else {
        "TCP server native error details exceed 4096 bytes".into()
    }
}
