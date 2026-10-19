use crate::app::module::common::streaming::commands::SendInput;
mod outbound;
#[cfg(test)]
mod tests;
mod worker;

use crate::app::module::common::streaming::input::{InputPlan, PreparedStreamInput, StreamInputConfig};
use crate::app::module::common::streaming::native_replies::NativeReply;
use golden_io::{BoundedWorker, PendingDrain, PendingSendError, RetirementPool, WorkerLimits, WorkerNotifier};
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
const EVENT_BYTES: usize = 32 * 1024 * 1024;
const EVENT_TURN: NonZeroUsize = NonZeroUsize::new(4).unwrap();

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TcpStreamingTransportConfig {
    pub remote_host: String,
    pub remote_port: u16,
    pub receive_enabled: bool,
    pub send_enabled: bool,
    pub input: StreamInputConfig,
}

impl TcpStreamingTransportConfig {
    pub(super) fn same_endpoint(&self, other: &Self) -> bool {
        self.remote_host == other.remote_host
            && self.remote_port == other.remote_port
            && self.receive_enabled == other.receive_enabled
            && self.send_enabled == other.send_enabled
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TcpStreamingConnectionStatus {
    Connected { remote_address: SocketAddr },
    Recovering { message: String },
}

#[derive(Clone, Debug)]
pub(crate) enum StreamingWorkerEvent {
    Input(golden_io::RetainedPayload<PreparedStreamInput>),
    RejectedInput(u64),
    Error(String),
    OutgoingLog(String),
    Status(TcpStreamingConnectionStatus),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
                Self::Error(message) | Self::OutgoingLog(message) => message.capacity(),
                Self::Status(TcpStreamingConnectionStatus::Recovering { message }) => message.capacity(),
                Self::Status(_) => 0,
            }
    }
    fn limit(source: ReplySource) -> usize {
        match source {
            ReplySource::Input => EVENT_BYTES,
            _ => 64 * 1024,
        }
    }
    fn oversized(source: ReplySource) -> Self {
        Self::Error(format!(
            "TCP {source:?} result exceeds its retained-byte limit; details rejected and native work is not replayed"
        ))
    }
}

pub(crate) struct TcpStreamingTransportHandle {
    worker: BoundedWorker<Outbound, StreamingWorkerEvent>,
    connection: Arc<AtomicU64>,
    send_enabled: bool,
    input: InputPlan,
}

fn lifetimes() -> &'static RetirementPool {
    static POOL: OnceLock<RetirementPool> = OnceLock::new();
    POOL.get_or_init(|| RetirementPool::new(NonZeroUsize::new(16).unwrap()))
}

impl TcpStreamingTransportHandle {
    pub(crate) fn spawn(config: TcpStreamingTransportConfig, notifier: WorkerNotifier) -> Result<Self, String> {
        let endpoint = config.clone();
        Self::spawn_with_connector(config, lifetimes(), notifier, move |cancel| {
            worker::connect(&endpoint, cancel)
        })
    }

    fn spawn_with_connector(
        config: TcpStreamingTransportConfig,
        pool: &RetirementPool,
        notifier: WorkerNotifier,
        connector: impl FnMut(&golden_io::WorkerCancellation) -> Result<std::net::TcpStream, String> + Send + 'static,
    ) -> Result<Self, String> {
        if config.remote_host.len() > 1024 || config.remote_host.trim().is_empty() {
            return Err("TCP remote host must contain 1–1024 bytes; endpoint was not admitted".into());
        }
        let connection = Arc::new(AtomicU64::new(0));
        let native_connection = connection.clone();
        let send_enabled = config.send_enabled;
        let input = InputPlan::new(config.input.clone());
        let worker_input = input.clone();
        let worker = BoundedWorker::spawn_notified(
            "streaming-tcp-client",
            pool,
            WorkerLimits {
                command_items: 256,
                command_weight: COMMAND_BYTES,
                event_items: 64,
                event_weight: EVENT_BYTES,
            },
            notifier,
            move |context| worker::run(context, config, worker_input, native_connection, connector),
        )
        .map_err(|error| format!("TCP worker could not start: {error}"))?;
        Ok(Self {
            worker,
            connection,
            send_enabled,
            input,
        })
    }

    pub(crate) fn send<'a>(&self, bytes: impl Into<SendInput<'a>>, log: bool) -> Result<(), String> {
        if !self.worker.is_accepting_commands() {
            return Err("TCP worker is not accepting commands".into());
        }
        if !self.send_enabled {
            return Err("TCP sender is disabled; command was not admitted".into());
        }
        let generation = self.connection.load(Ordering::Acquire);
        if generation == 0 {
            return Err("TCP transport is not connected; command was not admitted".into());
        }
        let command = Outbound::new(bytes, generation, log)?;
        let weight = command.weight();
        self.worker.send_weighted(command, weight).map_err(|error| match error {
            PendingSendError::Full(_) => "TCP command capacity exhausted; command was not admitted".into(),
            PendingSendError::Disconnected(_) => "TCP worker is not accepting commands".into(),
        })
    }

    pub(crate) fn drain_events(&self, events: &mut Vec<StreamingWorkerEvent>) -> PendingDrain {
        self.worker.drain_events(events, EVENT_TURN)
    }
    pub(super) fn update_input(&self, config: StreamInputConfig) {
        self.input.update(config);
    }
    pub(crate) fn has_pending(&self) -> bool {
        self.worker.has_pending()
    }
    #[cfg(test)]
    pub(super) fn event_metrics(&self) -> golden_io::PendingMetricsSnapshot {
        self.worker.event_metrics()
    }
    pub(crate) fn finish_admission(&mut self) {
        self.worker.finish_admission();
    }
    pub(crate) fn stop(&mut self) {
        self.connection.store(0, Ordering::Release);
        self.worker.cancel();
    }
}
