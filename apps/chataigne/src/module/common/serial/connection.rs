mod outbound;
#[cfg(test)]
mod tests;
mod worker;

use crate::app::module::common::streaming::{
    input::{InputPlan, PreparedStreamInput, StreamInputConfig},
    native_replies::NativeReply,
};
use golden_io::{
    BoundedWorker, PendingDrain, PendingSendError, RetainedPayload, RetirementPool, WorkerLimits, WorkerNotifier,
};
use outbound::Outbound;
use std::{
    io::{Read, Write},
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, OnceLock,
    },
};

const COMMAND_BYTES: usize = 256 * 1024;
const QUEUED_COMMAND_BYTES: usize = 256 * 1024;
const EVENT_BYTES: usize = 32 * 1024 * 1024;
const TURN: usize = 4;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SerialConnectionConfig {
    pub port_name: String,
    pub baud_rate: u32,
    pub dtr: bool,
    pub rts: bool,
    pub input: StreamInputConfig,
}
impl SerialConnectionConfig {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.port_name.capacity() > 1024 || self.port_name.trim().is_empty() || self.port_name.contains('\0') {
            return Err("Serial port name must contain 1–1024 retained bytes without NUL".into());
        }
        if self.baud_rate == 0 {
            return Err("Serial baud rate must be positive".into());
        }
        self.input.validate()
    }
    pub(crate) fn same_endpoint(&self, other: &Self) -> bool {
        self.port_name == other.port_name
            && self.baud_rate == other.baud_rate
            && self.dtr == other.dtr
            && self.rts == other.rts
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SerialConnectionStatus {
    Connected { port_name: String },
    Recovering { port_name: String, message: String },
}
#[derive(Clone, Debug)]
pub(crate) enum SerialConnectionEvent {
    Input(RetainedPayload<PreparedStreamInput>),
    RejectedInput(u64),
    Warning(String),
    OutgoingLog(String),
    Status(SerialConnectionStatus),
}
use SerialConnectionEvent as Event;
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
        std::mem::size_of::<Self>()
            + match self {
                Self::Input(input) => input.weight,
                Self::RejectedInput(_) => 0,
                Self::Warning(message) | Self::OutgoingLog(message) => message.capacity(),
                Self::Status(SerialConnectionStatus::Connected { port_name }) => port_name.capacity(),
                Self::Status(SerialConnectionStatus::Recovering { port_name, message }) => {
                    port_name.capacity() + message.capacity()
                }
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
        Self::Warning("Serial native result exceeds its retained-byte limit; native work is not replayed".into())
    }
}
pub(crate) struct SerialConnectionHandle {
    worker: BoundedWorker<Outbound, Event>,
    connection: Arc<AtomicU64>,
    input: InputPlan,
}
fn lifetimes() -> &'static RetirementPool {
    static POOL: OnceLock<RetirementPool> = OnceLock::new();
    POOL.get_or_init(|| RetirementPool::new(NonZeroUsize::new(16).unwrap()))
}
impl SerialConnectionHandle {
    pub(crate) fn spawn(config: SerialConnectionConfig, notifier: WorkerNotifier) -> Result<Self, String> {
        Self::spawn_using(config, lifetimes(), worker::open, notifier)
    }
    fn spawn_using<P: Read + Write + Send + 'static>(
        config: SerialConnectionConfig,
        pool: &RetirementPool,
        opener: impl FnMut(&SerialConnectionConfig) -> Result<P, String> + Send + 'static,
        notifier: WorkerNotifier,
    ) -> Result<Self, String> {
        config.validate()?;
        let connection = Arc::new(AtomicU64::new(0));
        let native = connection.clone();
        let input = InputPlan::new(config.input.clone());
        let native_input = input.clone();
        let worker = BoundedWorker::spawn_notified(
            "serial-connection",
            pool,
            WorkerLimits {
                command_items: 256,
                command_weight: QUEUED_COMMAND_BYTES,
                event_items: 64,
                event_weight: EVENT_BYTES,
            },
            notifier,
            move |context| worker::run(context, config, native, native_input, opener),
        )
        .map_err(|error| format!("Serial worker could not start: {error}"))?;
        Ok(Self {
            worker,
            connection,
            input,
        })
    }
    pub(crate) fn send(&self, bytes: &[u8], log: bool) -> Result<(), String> {
        if !self.worker.is_accepting_commands() {
            return Err("Serial worker is not accepting commands".into());
        }
        let generation = self.connection.load(Ordering::Acquire);
        if generation == 0 {
            return Err("Serial port is not connected; command was not admitted".into());
        }
        let command = Outbound::new(bytes, generation, log)?;
        let weight = command.weight();
        self.worker.send_weighted(command, weight).map_err(|error| match error {
            PendingSendError::Full(_) => "Serial command capacity exhausted; command was not admitted".into(),
            PendingSendError::Disconnected(_) => "Serial worker is not accepting commands".into(),
        })
    }
    pub(crate) fn drain_events(&self, events: &mut Vec<Event>) -> PendingDrain {
        self.worker.drain_events(events, NonZeroUsize::new(TURN).unwrap())
    }
    pub(crate) fn has_pending(&self) -> bool {
        self.worker.has_pending()
    }
    pub(crate) fn stop(&mut self) {
        self.connection.store(0, Ordering::Release);
        self.worker.cancel();
    }
    pub(crate) fn finish_admission(&mut self) {
        self.worker.finish_admission();
    }
    pub(crate) fn update_input(&self, input: StreamInputConfig) -> Result<(), String> {
        input.validate()?;
        self.input.update(input);
        Ok(())
    }
}
impl Drop for SerialConnectionHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
pub(crate) fn spawn_for_test<P: Read + Write + Send + 'static>(
    config: SerialConnectionConfig,
    pool: &RetirementPool,
    opener: impl FnMut(&SerialConnectionConfig) -> Result<P, String> + Send + 'static,
    notifier: WorkerNotifier,
) -> Result<SerialConnectionHandle, String> {
    SerialConnectionHandle::spawn_using(config, pool, opener, notifier)
}
