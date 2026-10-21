pub(super) mod admission;
#[cfg(test)]
mod tests;
pub(super) mod worker;

use golden_io::{BoundedWorker, PendingDrain, PendingSendError, RetirementPool, WorkerLimits, WorkerNotifier};
use mio::Waker;
use std::{
    net::{SocketAddr, ToSocketAddrs},
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
};

use super::osc_input::{InputMode, InputPlan, PreparedInput};
#[cfg(test)]
use super::osc_message::OscDecodedMessage;
#[cfg(test)]
use super::osc_message::OscValuePayload;
use super::osc_outbound::{OutboundRequest, QueueError, CAPTURE_WEIGHT};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OscTransportConfig {
    pub bind_interface_host: String,
    pub bind_port: u16,
    pub receive_enabled: bool,
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OscOutboundMessage {
    pub address: Arc<str>,
    pub payload: OscValuePayload,
    pub remote_address: SocketAddr,
}

#[derive(Debug)]
pub(super) enum OscWorkerCommand {
    Captured(OutboundRequest),
    #[cfg(test)]
    Direct(OscOutboundMessage),
}
impl OscWorkerCommand {
    pub(super) fn description(&self) -> String {
        match self {
            Self::Captured(request) => request.description(),
            #[cfg(test)]
            Self::Direct(message) => message.address.to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum OscWorkerEvent {
    Ready(SocketAddr),
    #[cfg(test)]
    Message(OscDecodedMessage),
    Input(golden_io::RetainedPayload<PreparedInput>),
    Error(String),
    Failed(String),
    Sent(Option<String>),
}

pub(super) const OSC_EVENT_DRAIN_BUDGET: NonZeroUsize = NonZeroUsize::new(64).unwrap();
const OSC_COMMAND_TURN: NonZeroUsize = NonZeroUsize::new(16).unwrap();
const OSC_SOCKET_SERVICE_BUDGET: usize = 16;
const OSC_EVENT_BYTES: usize = 2 * 1024 * 1024;
const OSC_MAX_DATAGRAM: usize = 65_507;
const OSC_MAX_ARGUMENTS: usize = 1024;
const OSC_MAX_ADDRESS: usize = 4096;
const OSC_MAX_COMMAND_BYTES: usize = 128 * 1024;

fn limits() -> WorkerLimits {
    WorkerLimits {
        command_items: 256,
        command_weight: 4 * 1024 * 1024,
        event_items: 128,
        event_weight: OSC_EVENT_BYTES,
    }
}

fn lifetimes() -> &'static RetirementPool {
    static POOL: OnceLock<RetirementPool> = OnceLock::new();
    POOL.get_or_init(|| RetirementPool::new(NonZeroUsize::new(16).unwrap()))
}

#[derive(Clone, Default)]
struct NativeWake {
    source: Arc<Mutex<Option<Arc<Waker>>>>,
    failed: Arc<AtomicBool>,
}

impl NativeWake {
    fn wake(&self) {
        let source = self.source.lock().unwrap_or_else(|error| error.into_inner());
        if source.as_ref().is_some_and(|source| source.wake().is_err()) {
            // Admission already succeeded. Do not report rejection or invite a
            // duplicate send; the native loop's coarse deadline still services it.
            self.failed.store(true, Ordering::Release);
        }
    }

    fn clear(&self) {
        let source = self.source.lock().unwrap_or_else(|error| error.into_inner()).take();
        // Final native wake-handle disposal belongs to the worker too. Callers
        // borrow under the short lock and never retain a native strong reference.
        drop(source);
    }
}

pub(crate) struct OscTransportHandle {
    worker: BoundedWorker<OscWorkerCommand, OscWorkerEvent>,
    wake: NativeWake,
    ready: Arc<AtomicBool>,
    input: InputMode,
}

impl OscTransportHandle {
    pub(crate) fn spawn(
        config: OscTransportConfig,
        input: InputPlan,
        notifier: WorkerNotifier,
    ) -> Result<Self, String> {
        Self::spawn_with_input(
            config,
            lifetimes(),
            notifier,
            InputMode::Prepared(input),
            worker::NativeSocket::open,
        )
    }

    #[cfg(test)]
    fn spawn_using<S: worker::OscSocket>(
        config: OscTransportConfig,
        lifetimes: &RetirementPool,
        notifier: WorkerNotifier,
        setup: impl FnOnce(&OscTransportConfig, &NativeWake) -> Result<S, String> + Send + 'static,
    ) -> Result<Self, String> {
        Self::spawn_with_input(config, lifetimes, notifier, InputMode::Decoded, setup)
    }

    fn spawn_with_input<S: worker::OscSocket>(
        config: OscTransportConfig,
        lifetimes: &RetirementPool,
        notifier: WorkerNotifier,
        input: InputMode,
        setup: impl FnOnce(&OscTransportConfig, &NativeWake) -> Result<S, String> + Send + 'static,
    ) -> Result<Self, String> {
        if config.bind_interface_host.len() > 1024 || config.bind_interface_host.capacity() > 1024 {
            return Err("OSC bind host exceeds 1024 bytes".into());
        }
        let wake = NativeWake::default();
        let worker_wake = wake.clone();
        let ready = Arc::new(AtomicBool::new(false));
        let worker_ready = ready.clone();
        let worker_input = input.clone();
        let status_notifier = notifier.clone();
        let worker = BoundedWorker::spawn_notified("osc-transport", lifetimes, limits(), notifier, move |context| {
            worker::run(
                context,
                config,
                worker_wake,
                worker_ready,
                worker_input,
                status_notifier,
                setup,
            );
        })
        .map_err(|error| format!("failed to start OSC worker: {error}"))?;
        Ok(Self {
            worker,
            wake,
            ready,
            input,
        })
    }

    pub(crate) fn log_incoming(&self, enabled: bool) {
        match &self.input {
            InputMode::Prepared(plan) => plan.set_log(enabled),
            #[cfg(test)]
            InputMode::Decoded => {}
        }
    }

    pub(crate) fn drain_event(&self, events: &mut Vec<OscWorkerEvent>) -> PendingDrain {
        let turn = self.worker.drain_events(events, NonZeroUsize::MIN);
        if turn.received != 0 {
            self.wake.wake();
        }
        turn
    }

    pub(crate) fn enqueue(&self, request: OutboundRequest) -> Result<(), QueueError> {
        self.worker
            .send_weighted(OscWorkerCommand::Captured(request), CAPTURE_WEIGHT)
            .map_err(|error| match error {
                PendingSendError::Full(_) => QueueError::Full,
                PendingSendError::Disconnected(_) => QueueError::Closed,
            })?;
        self.wake.wake();
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn send(&self, message: OscOutboundMessage) -> Result<(), String> {
        let weight = admission::outbound_weight(&message)?;
        self.worker
            .send_weighted(OscWorkerCommand::Direct(message), weight)
            .map_err(|error| match error {
                PendingSendError::Full(_) => "OSC output item/byte capacity exhausted; message rejected".to_string(),
                PendingSendError::Disconnected(_) => "OSC worker is no longer accepting commands".to_string(),
            })?;
        self.wake.wake();
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn drain_events(&self, events: &mut Vec<OscWorkerEvent>) -> PendingDrain {
        let turn = self.worker.drain_events(events, OSC_EVENT_DRAIN_BUDGET);
        if turn.received != 0 {
            self.wake.wake();
        }
        turn
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.worker.has_pending()
    }

    pub(crate) fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }

    pub(crate) fn finish_admission(&mut self) {
        self.worker.finish_admission();
        self.wake.wake();
    }

    pub(crate) fn stop(&mut self) {
        self.worker.cancel();
        self.wake.wake();
    }
}

impl Drop for OscTransportHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

fn should_ignore_receive_error(error: &std::io::Error) -> bool {
    error.raw_os_error() == Some(10054) || (cfg!(windows) && error.kind() == std::io::ErrorKind::ConnectionReset)
}

pub(crate) fn resolve_socket_addr(host: &str, port: u16) -> Result<SocketAddr, String> {
    (host.trim(), port)
        .to_socket_addrs()
        .map_err(|error| format!("failed to resolve OSC endpoint: {error}"))?
        .next()
        .ok_or_else(|| "OSC endpoint resolved to no address".into())
}
