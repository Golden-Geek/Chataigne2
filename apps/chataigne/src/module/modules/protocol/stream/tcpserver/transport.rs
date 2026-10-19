mod outbound;
#[cfg(test)]
mod tests;
mod worker;

use crate::app::module::common::streaming::native_replies::NativeReply;
use golden_io::{BoundedWorker, PendingDrain, PendingSendError, RetirementPool, WorkerLimits};
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
const QUEUED_COMMAND_BYTES: usize = 8 * COMMAND_BYTES;
const EVENT_BYTES: usize = 1024 * 1024;
const EVENT_TURN: NonZeroUsize = NonZeroUsize::new(4).unwrap();
const MAX_CLIENTS: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TcpServerTransportConfig {
    pub bind_host: String,
    pub bind_port: u16,
    pub receive_enabled: bool,
    pub send_enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TcpServerWorkerEvent {
    Listening(SocketAddr),
    Recovering(String),
    ClientConnected { client_id: String, info: String },
    ClientDisconnected { client_id: String, reason: Option<String> },
    Bytes { client_id: String, bytes: Vec<u8> },
    Error(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReplySource {
    Input,
    Output,
    Status,
}

impl NativeReply for TcpServerWorkerEvent {
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
                Self::Bytes { client_id, bytes } => client_id.capacity() + bytes.capacity(),
            }
    }
    fn limit(_: ReplySource) -> usize {
        16 * 1024
    }
    fn oversized(_: ReplySource) -> Self {
        Self::Error("TCP server native result exceeds 16 KiB; details rejected, native work is not replayed".into())
    }
}

pub(crate) struct TcpServerTransportHandle {
    worker: BoundedWorker<Vec<u8>, TcpServerWorkerEvent>,
    clients: Arc<AtomicUsize>,
    send_enabled: bool,
}

fn lifetimes() -> &'static RetirementPool {
    static POOL: OnceLock<RetirementPool> = OnceLock::new();
    POOL.get_or_init(|| RetirementPool::new(NonZeroUsize::new(16).unwrap()))
}

impl TcpServerTransportHandle {
    pub(crate) fn spawn(config: TcpServerTransportConfig) -> Result<Self, String> {
        Self::spawn_using(config, lifetimes(), worker::bind)
    }
    fn spawn_using(
        config: TcpServerTransportConfig,
        pool: &RetirementPool,
        binder: impl FnMut(&TcpServerTransportConfig) -> Result<TcpListener, String> + Send + 'static,
    ) -> Result<Self, String> {
        if config.bind_host.trim().is_empty() || config.bind_host.capacity() > 1024 {
            return Err("TCP server bind host must contain 1–1024 retained bytes; endpoint was not admitted".into());
        }
        let clients = Arc::new(AtomicUsize::new(0));
        let native_clients = clients.clone();
        let send_enabled = config.send_enabled;
        let worker = BoundedWorker::spawn(
            "streaming-tcp-server",
            pool,
            WorkerLimits {
                command_items: 256,
                command_weight: QUEUED_COMMAND_BYTES,
                event_items: 64,
                event_weight: EVENT_BYTES,
            },
            move |context| worker::run(context, config, native_clients, binder),
        )
        .map_err(|error| format!("TCP server worker could not start: {error}"))?;
        Ok(Self {
            worker,
            clients,
            send_enabled,
        })
    }
    pub(crate) fn send(&self, bytes: &[u8]) -> Result<(), String> {
        if !self.send_enabled || self.clients.load(Ordering::Acquire) == 0 {
            return Err("TCP server has no eligible clients; command was not admitted".into());
        }
        let weight = size_of::<Vec<u8>>().saturating_add(bytes.len());
        if weight > COMMAND_BYTES {
            return Err("TCP server command exceeds 1 MiB; command was not admitted".into());
        }
        self.worker
            .send_weighted(bytes.to_vec(), weight)
            .map_err(|error| match error {
                PendingSendError::Full(_) => "TCP server command capacity exhausted; command was not admitted".into(),
                PendingSendError::Disconnected(_) => "TCP server worker is not accepting commands".into(),
            })
    }
    pub(crate) fn drain_events(&self, events: &mut Vec<TcpServerWorkerEvent>) -> PendingDrain {
        self.worker.drain_events(events, EVENT_TURN)
    }
    pub(crate) fn has_pending(&self) -> bool {
        self.worker.has_pending()
    }
    pub(crate) fn stop(&mut self) {
        self.clients.store(0, Ordering::Release);
        self.worker.cancel();
    }
}
