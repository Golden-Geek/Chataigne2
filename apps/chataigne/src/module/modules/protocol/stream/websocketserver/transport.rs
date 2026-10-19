mod handshake;
mod outbound;
mod peers;
#[cfg(test)]
mod tests;
mod worker;

use crate::app::module::common::streaming::{commands::StreamingSendFrameKind, native_replies::NativeReply};
use golden_io::{BoundedWorker, PendingDrain, PendingSendError, RetirementPool, WorkerLimits};
use outbound::Command;
use std::{
    mem::size_of,
    net::{SocketAddr, TcpListener},
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, OnceLock,
    },
};

const COMMAND_BYTES: usize = 1024 * 1024;
const EVENT_BYTES: usize = 32 * 1024 * 1024;
const MAX_CLIENTS: usize = 128;
const TURN: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WebSocketServerTransportConfig {
    pub bind_host: String,
    pub bind_port: u16,
    pub path: String,
    pub receive_enabled: bool,
    pub send_enabled: bool,
}
impl WebSocketServerTransportConfig {
    fn validate(&self) -> Result<(), String> {
        if self.bind_host.capacity() > 1024 || self.bind_host.trim().is_empty() {
            return Err("WebSocket server bind host must contain 1–1024 retained bytes".into());
        }
        if self.path.capacity() > 8192 || (!self.path.is_empty() && !self.path.starts_with('/')) {
            return Err("WebSocket server path must be empty or start with / and fit 8192 retained bytes".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum WebSocketServerWorkerEvent {
    Listening(SocketAddr),
    Recovering(String),
    ClientConnected {
        client_id: String,
        info: String,
    },
    ClientDisconnected {
        client_id: String,
        reason: Option<String>,
    },
    Bytes {
        client_id: String,
        frame_kind: StreamingSendFrameKind,
        bytes: Vec<u8>,
    },
    Error(String),
}
use WebSocketServerWorkerEvent as Event;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReplySource {
    Input,
    Output,
    Status,
}
impl NativeReply for Event {
    type Source = ReplySource;
    const SOURCES: usize = 3;
    fn weight(&self) -> usize {
        size_of::<Self>()
            + match self {
                Self::Listening(_) => 0,
                Self::Recovering(message) | Self::Error(message) => message.capacity(),
                Self::ClientConnected { client_id, info } => client_id.capacity() + info.capacity(),
                Self::ClientDisconnected { client_id, reason } => {
                    client_id.capacity() + reason.as_ref().map_or(0, String::capacity)
                }
                Self::Bytes { client_id, bytes, .. } => client_id.capacity() + bytes.capacity(),
            }
    }
    fn limit(source: ReplySource) -> usize {
        if source == ReplySource::Input {
            EVENT_BYTES
        } else {
            64 * 1024
        }
    }
    fn oversized(_: ReplySource) -> Self {
        Self::Error("WebSocket server result exceeds its retained-byte limit; native work is not replayed".into())
    }
}

pub(crate) struct WebSocketServerTransportHandle {
    worker: BoundedWorker<Command, Event>,
    clients: Arc<AtomicUsize>,
    send_enabled: bool,
}
fn lifetimes() -> &'static RetirementPool {
    static POOL: OnceLock<RetirementPool> = OnceLock::new();
    POOL.get_or_init(|| RetirementPool::new(NonZeroUsize::new(16).unwrap()))
}
impl WebSocketServerTransportHandle {
    pub(crate) fn spawn(config: WebSocketServerTransportConfig) -> Result<Self, String> {
        Self::spawn_using(config, lifetimes(), worker::bind)
    }
    fn spawn_using(
        config: WebSocketServerTransportConfig,
        pool: &RetirementPool,
        binder: impl FnMut(&WebSocketServerTransportConfig) -> Result<TcpListener, String> + Send + 'static,
    ) -> Result<Self, String> {
        config.validate()?;
        let clients = Arc::new(AtomicUsize::new(0));
        let native_clients = clients.clone();
        let send_enabled = config.send_enabled;
        let worker = BoundedWorker::spawn(
            "streaming-websocket-server",
            pool,
            WorkerLimits {
                command_items: 256,
                command_weight: 8 * COMMAND_BYTES,
                event_items: 64,
                event_weight: EVENT_BYTES,
            },
            move |context| worker::run(context, config, native_clients, binder),
        )
        .map_err(|error| format!("WebSocket server worker could not start: {error}"))?;
        Ok(Self {
            worker,
            clients,
            send_enabled,
        })
    }
    pub(crate) fn send(&self, kind: StreamingSendFrameKind, bytes: &[u8]) -> Result<(), String> {
        if !self.worker.is_accepting_commands() {
            return Err("WebSocket server worker is not accepting commands".into());
        }
        if !self.send_enabled || self.clients.load(Ordering::Acquire) == 0 {
            return Err("WebSocket server has no eligible clients; command was not admitted".into());
        }
        let command = Command::new(kind, bytes)?;
        let weight = command.weight();
        self.worker.send_weighted(command, weight).map_err(|error| match error {
            PendingSendError::Full(_) => "WebSocket server command capacity exhausted; command was not admitted".into(),
            PendingSendError::Disconnected(_) => "WebSocket server worker is not accepting commands".into(),
        })
    }
    pub(crate) fn drain_events(&self, events: &mut Vec<Event>) -> PendingDrain {
        self.worker.drain_events(events, NonZeroUsize::new(TURN).unwrap())
    }
    pub(crate) fn has_pending(&self) -> bool {
        self.worker.has_pending()
    }
    pub(crate) fn stop(&mut self) {
        self.clients.store(0, Ordering::Release);
        self.worker.cancel();
    }
}
