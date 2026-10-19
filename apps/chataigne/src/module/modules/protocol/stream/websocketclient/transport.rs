use crate::app::module::common::streaming::commands::SendInput;
mod outbound;
mod socket;
#[cfg(test)]
mod tests;
mod worker;

use crate::app::module::common::streaming::{
    commands::StreamingSendFrameKind,
    input::{InputPlan, PreparedStreamInput, StreamInputConfig},
    native_replies::NativeReply,
    websocket::socket::WireSocket,
};
use golden_io::{
    BoundedWorker, PendingDrain, PendingSendError, RetainedPayload, RetirementPool, WorkerCancellation, WorkerLimits,
    WorkerNotifier,
};
use outbound::Outbound;
use std::{
    net::SocketAddr,
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, OnceLock,
    },
};

const COMMAND_BYTES: usize = 1024 * 1024;
const QUEUED_COMMAND_BYTES: usize = 8 * COMMAND_BYTES;
const EVENT_BYTES: usize = 32 * 1024 * 1024;
const EVENT_TURN: NonZeroUsize = NonZeroUsize::new(4).unwrap();

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct WebSocketClientTransportConfig {
    pub remote_host: String,
    pub remote_port: u16,
    pub path: String,
    pub receive_enabled: bool,
    pub send_enabled: bool,
    pub input: StreamInputConfig,
}
impl WebSocketClientTransportConfig {
    pub(super) fn validate(&self) -> Result<(), String> {
        if self.remote_host.trim().is_empty() || self.remote_host.capacity() > 1024 {
            return Err("WebSocket remote host must contain 1–1024 retained bytes".into());
        }
        if self.path.capacity() > 8192 || !self.path.starts_with('/') {
            return Err("WebSocket path must start with / and fit 8192 retained bytes".into());
        }
        self.input.validate()
    }
    pub(super) fn same_endpoint(&self, other: &Self) -> bool {
        self.remote_host == other.remote_host
            && self.remote_port == other.remote_port
            && self.path == other.path
            && self.send_enabled == other.send_enabled
            && self.receive_enabled == other.receive_enabled
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum WebSocketClientConnectionStatus {
    Connected { remote_address: SocketAddr },
    Recovering { message: String },
}
#[derive(Clone, Debug)]
pub(crate) enum StreamingWorkerEvent {
    Input(RetainedPayload<PreparedStreamInput>),
    RejectedInput(u64),
    OutgoingLog(String),
    Error(String),
    Status(WebSocketClientConnectionStatus),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReplySource {
    Input,
    Output,
    Status,
}
impl NativeReply for StreamingWorkerEvent {
    type Source = ReplySource;
    const SOURCES: usize = 3;
    fn weight(&self) -> usize {
        std::mem::size_of::<Self>()
            + match self {
                Self::Input(input) => input.weight,
                Self::RejectedInput(_) => 0,
                Self::OutgoingLog(message)
                | Self::Error(message)
                | Self::Status(WebSocketClientConnectionStatus::Recovering { message }) => message.capacity(),
                Self::Status(_) => 0,
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
        Self::Error(
            "WebSocket result exceeds its retained-byte limit; details rejected, native work is not replayed".into(),
        )
    }
}

pub(crate) struct WebSocketClientTransportHandle {
    worker: BoundedWorker<Outbound, StreamingWorkerEvent>,
    connection: Arc<AtomicU64>,
    send_enabled: bool,
    input: InputPlan,
}
fn lifetimes() -> &'static RetirementPool {
    static POOL: OnceLock<RetirementPool> = OnceLock::new();
    POOL.get_or_init(|| RetirementPool::new(NonZeroUsize::new(16).unwrap()))
}
impl WebSocketClientTransportHandle {
    pub(crate) fn spawn(config: WebSocketClientTransportConfig, notifier: WorkerNotifier) -> Result<Self, String> {
        config.validate()?;
        let endpoint = config.clone();
        Self::spawn_with_connector(config, lifetimes(), notifier, move |cancel, stopped| {
            socket::connect(&endpoint, cancel, stopped)
        })
    }
    fn spawn_with_connector(
        config: WebSocketClientTransportConfig,
        pool: &RetirementPool,
        notifier: WorkerNotifier,
        connector: impl FnMut(&WorkerCancellation, &dyn Fn() -> bool) -> Result<WireSocket, String> + Send + 'static,
    ) -> Result<Self, String> {
        config.validate()?;
        let connection = Arc::new(AtomicU64::new(0));
        let native_connection = connection.clone();
        let send_enabled = config.send_enabled;
        let input = InputPlan::new(config.input.clone());
        let native_input = input.clone();
        let worker = BoundedWorker::spawn_notified(
            "streaming-websocket-client",
            pool,
            WorkerLimits {
                command_items: 256,
                command_weight: QUEUED_COMMAND_BYTES,
                event_items: 64,
                event_weight: EVENT_BYTES,
            },
            notifier,
            move |context| worker::run(context, config, native_connection, native_input, connector),
        )
        .map_err(|error| format!("WebSocket worker could not start: {error}"))?;
        Ok(Self {
            worker,
            connection,
            send_enabled,
            input,
        })
    }
    pub(crate) fn send<'a>(
        &self,
        kind: StreamingSendFrameKind,
        bytes: impl Into<SendInput<'a>>,
        log: bool,
    ) -> Result<(), String> {
        if !self.worker.is_accepting_commands() {
            return Err("WebSocket worker is not accepting commands".into());
        }
        if !self.send_enabled {
            return Err("WebSocket sender is disabled; command was not admitted".into());
        }
        let generation = self.connection.load(Ordering::Acquire);
        if generation == 0 {
            return Err("WebSocket transport is not connected; command was not admitted".into());
        }
        let command = Outbound::new(kind, bytes, generation, log)?;
        let weight = command.weight();
        self.worker.send_weighted(command, weight).map_err(|error| match error {
            PendingSendError::Full(_) => "WebSocket command capacity exhausted; command was not admitted".into(),
            PendingSendError::Disconnected(_) => "WebSocket worker is not accepting commands".into(),
        })
    }
    pub(crate) fn drain_events(&self, events: &mut Vec<StreamingWorkerEvent>) -> PendingDrain {
        self.worker.drain_events(events, EVENT_TURN)
    }
    pub(crate) fn has_pending(&self) -> bool {
        self.worker.has_pending()
    }
    pub(crate) fn finish_admission(&mut self) {
        self.worker.finish_admission();
    }
    pub(crate) fn update_input(&self, config: StreamInputConfig) -> Result<(), String> {
        config.validate()?;
        self.input.update(config);
        Ok(())
    }
    pub(crate) fn stop(&mut self) {
        self.connection.store(0, Ordering::Release);
        self.worker.cancel();
    }
}
