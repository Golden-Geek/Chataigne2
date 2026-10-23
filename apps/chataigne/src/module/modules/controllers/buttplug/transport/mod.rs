use crate::app::module::common::buttplug::ButtplugSetOutputRequest;
use golden_io::{
    BoundedWorker, PayloadPool, PendingDrain, RetainedPayload, RetirementPool, WorkerLimits, WorkerNotifier,
};
use std::{
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, OnceLock,
    },
};

mod admission;
mod input;
mod native;
mod publication;
mod receipts;
#[cfg(test)]
mod tests;
mod worker;

use golden_io::{RevisionGuard, RevisionReader, RevisionSource};
pub(super) use input::{Input, Source};
#[cfg(test)]
use native::{normalize_path, output_command};
pub(super) use publication::Publication;
use receipts::AdmittedCommand;
pub(super) use receipts::{CommandReceipt, CommandStatus};

struct Shared {
    connected: AtomicBool,
    running: AtomicBool,
    notifier: WorkerNotifier,
    actor: RevisionGuard,
    native: RevisionReader,
    session: RevisionReader,
    selection: std::sync::Mutex<String>,
    pressure: AtomicBool,
}
struct NativeLife {
    native: RevisionSource,
    session: RevisionSource,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ButtplugTransportConfig {
    pub remote_host: String,
    pub remote_port: u16,
    pub path: String,
    pub secure: bool,
    pub bypass_certificate_verification: bool,
    pub client_name: String,
    pub auto_scan: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ButtplugDeviceInfo {
    pub index: u32,
    pub variant_id: String,
    pub name: String,
    pub display_name: Option<String>,
    pub outputs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ButtplugConnectionStatus {
    Connected { server_name: String },
    Recovering { message: String },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ButtplugWorkerEvent {
    Status(ButtplugConnectionStatus),
    Devices(Vec<ButtplugDeviceInfo>),
    DeviceAdded(ButtplugDeviceInfo),
    DeviceRemoved(ButtplugDeviceInfo),
    Scanning(bool),
    ScanningFinished,
    Error(String),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ButtplugWorkerCommand {
    StartScanning,
    StopScanning,
    StopAllDevices,
    StopDevice { target: String },
    SetOutput(ButtplugSetOutputRequest),
}

const COMMAND_WEIGHT: usize = 8192;
const EVENT_WEIGHT: usize = 1024 * 1024;

fn lifetimes() -> &'static RetirementPool {
    static POOL: OnceLock<RetirementPool> = OnceLock::new();
    POOL.get_or_init(|| RetirementPool::new(NonZeroUsize::new(16).unwrap()))
}
fn payloads(incoming: bool) -> Result<PayloadPool, String> {
    static INPUT: OnceLock<std::sync::Mutex<Option<PayloadPool>>> = OnceLock::new();
    static OUTPUT: OnceLock<std::sync::Mutex<Option<PayloadPool>>> = OnceLock::new();
    let registry = if incoming { &INPUT } else { &OUTPUT };
    let mut pool = registry
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if pool.is_none() {
        *pool = Some(
            PayloadPool::new(
                if incoming {
                    "buttplug-input-disposer"
                } else {
                    "buttplug-output-disposer"
                },
                if incoming { 128 } else { 512 },
                if incoming { 128 * 1024 * 1024 } else { 32 * 1024 * 1024 },
            )
            .map_err(|error| error.to_string())?,
        );
    }
    Ok(pool.as_ref().unwrap().clone())
}

pub(crate) struct ButtplugTransportHandle {
    worker: BoundedWorker<AdmittedCommand, RetainedPayload<Publication>>,
    shared: Arc<Shared>,
    actor: Option<RevisionSource>,
    commands: PayloadPool,
    notifier: WorkerNotifier,
}
impl ButtplugTransportHandle {
    pub(crate) fn spawn(
        config: ButtplugTransportConfig,
        notifier: WorkerNotifier,
        owner: &RetirementPool,
    ) -> Result<Self, String> {
        admission::config(&config)?;
        let permit = owner
            .try_reserve()
            .map_err(|_| "Previous Buttplug connection is still retiring")?;
        Self::spawn_using(
            config,
            notifier,
            lifetimes(),
            payloads(false)?,
            payloads(true)?,
            move || native::Native::open().map(|provider| worker::Exclusive::new(provider, permit)),
        )
    }
    fn spawn_using<P: worker::Provider>(
        config: ButtplugTransportConfig,
        notifier: WorkerNotifier,
        slots: &RetirementPool,
        commands: PayloadPool,
        events: PayloadPool,
        open: impl FnOnce() -> Result<P, String> + Send + 'static,
    ) -> Result<Self, String> {
        admission::config(&config)?;
        let config = commands
            .try_retain(config, COMMAND_WEIGHT)
            .map_err(|_| "Buttplug configuration lifetime capacity exhausted")?;
        let actor = RevisionSource::default();
        let life = NativeLife {
            native: RevisionSource::default(),
            session: RevisionSource::default(),
        };
        let shared = Arc::new(Shared {
            connected: AtomicBool::new(false),
            running: AtomicBool::new(true),
            notifier: notifier.clone(),
            actor: actor.reader().guard_at(0),
            native: life.native.reader(),
            session: life.session.reader(),
            selection: std::sync::Mutex::new("all".into()),
            pressure: AtomicBool::new(false),
        });
        let state = shared.clone();
        let worker = BoundedWorker::spawn_notified(
            "buttplug-transport",
            slots,
            WorkerLimits {
                command_items: 64,
                command_weight: 4 * 1024 * 1024,
                event_items: 64,
                event_weight: 4 * 1024 * 1024,
            },
            notifier.clone(),
            move |ctx| worker::run(ctx, config, events, state, life, open),
        )
        .map_err(|error| error.to_string())?;
        Ok(Self {
            worker,
            shared,
            actor: Some(actor),
            commands,
            notifier,
        })
    }
    #[cfg(test)]
    fn send(&self, command: ButtplugWorkerCommand) -> Result<CommandReceipt, String> {
        admission::command(&command)?;
        if !self.shared.connected.load(Ordering::Acquire) || !self.shared.running.load(Ordering::Acquire) {
            return Err("Buttplug transport is not connected".into());
        }
        let source = self
            .commands
            .try_retain(
                Source {
                    input: Input::Prepared(command),
                    origin: input::Origin::Fixture,
                    time: golden_core::engine::EngineTime {
                        tick: 0,
                        micro: 0,
                        seq: 0,
                    },
                },
                COMMAND_WEIGHT,
            )
            .map_err(|_| "Buttplug command lifetime capacity exhausted")?;
        let (mut command, receipt) = AdmittedCommand::new(source, self.notifier.clone());
        command.arm();
        self.worker.send_weighted(command, COMMAND_WEIGHT).map_err(|error| {
            let (mut command, message) = match error {
                golden_io::PendingSendError::Full(command) => (command, "Buttplug command queue is full"),
                golden_io::PendingSendError::Disconnected(command) => (command, "Buttplug worker is no longer running"),
            };
            command.disarm();
            message
        })?;
        Ok(receipt)
    }
    pub(super) fn capture(
        &self,
        input: Input,
        producer: golden_core::engine::NodeWaker,
        time: golden_core::engine::EngineTime,
        permit: golden_io::RetirementPermit,
    ) -> Result<RetainedPayload<Source>, String> {
        if !self.shared.connected.load(Ordering::Acquire) {
            return Err("Buttplug connection is opening or stopped; request was not admitted".into());
        }
        let source = Source {
            input,
            time,
            origin: input::Origin::Engine {
                producer,
                _permit: permit,
                guards: [
                    self.shared.actor.clone(),
                    self.shared.native.guard_at(0),
                    self.shared.session.guard_at(self.shared.session.current()),
                ],
            },
        };
        let weight = source.weight();
        if weight > 4 * 1024 * 1024 {
            return Err("Buttplug captured input exceeds 4 MiB native mailbox capacity".into());
        }
        self.commands
            .try_retain(source, weight)
            .map_err(|_| "Buttplug source lifetime capacity exhausted".into())
    }
    pub(super) fn submit(&self, source: RetainedPayload<Source>) -> Result<CommandReceipt, RetainedPayload<Source>> {
        let weight = source.weight();
        let (mut command, receipt) = AdmittedCommand::new(source.clone(), self.notifier.clone());
        command.arm();
        self.worker.send_weighted(command, weight).map_err(|error| {
            let (golden_io::PendingSendError::Full(mut command)
            | golden_io::PendingSendError::Disconnected(mut command)) = error;
            command.disarm();
            source
        })?;
        Ok(receipt)
    }
    pub(super) fn select(&self, selection: &str) -> Result<(), String> {
        if selection.len() > 1024 {
            return Err("Buttplug selection exceeds 1024 bytes".into());
        }
        let mut current = self.shared.selection.lock().unwrap_or_else(|error| error.into_inner());
        if current.as_str() != selection {
            *current = selection.to_owned();
        }
        Ok(())
    }
    pub(super) fn pressure(&self) -> bool {
        self.shared.pressure.load(Ordering::Acquire)
    }
    pub(super) fn queued_events(&self) -> usize {
        self.worker.event_metrics().depth
    }
    pub(super) fn drain_events(&self, output: &mut Vec<RetainedPayload<Publication>>) -> PendingDrain {
        self.worker.drain_events(output, NonZeroUsize::MIN)
    }
    pub(crate) fn has_pending(&self) -> bool {
        self.worker.has_pending()
    }
    pub(crate) fn is_running(&self) -> bool {
        self.shared.running.load(Ordering::Acquire)
    }
    pub(crate) fn stop(&mut self) {
        self.actor = None;
        self.shared.connected.store(false, Ordering::Release);
        self.worker.cancel();
    }
}
impl Drop for ButtplugTransportHandle {
    fn drop(&mut self) {
        self.stop();
    }
}
