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
mod native;
mod receipts;
#[cfg(test)]
mod tests;
mod worker;

#[cfg(test)]
use native::{normalize_path, output_command};
use receipts::AdmittedCommand;
pub(super) use receipts::{CommandReceipt, CommandStatus};

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
const EVENT_WEIGHT: usize = 256 * 1024;

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
                if incoming { 32 * 1024 * 1024 } else { 4 * 1024 * 1024 },
            )
            .map_err(|error| error.to_string())?,
        );
    }
    Ok(pool.as_ref().unwrap().clone())
}

pub(crate) struct ButtplugTransportHandle {
    worker: BoundedWorker<AdmittedCommand, RetainedPayload<ButtplugWorkerEvent>>,
    connected: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
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
        let connected = Arc::new(AtomicBool::new(false));
        let running = Arc::new(AtomicBool::new(true));
        let ready = connected.clone();
        let live = running.clone();
        let wake = notifier.clone();
        let worker = BoundedWorker::spawn_notified(
            "buttplug-transport",
            slots,
            WorkerLimits {
                command_items: 64,
                command_weight: 256 * 1024,
                event_items: 64,
                event_weight: 4 * 1024 * 1024,
            },
            notifier.clone(),
            move |ctx| worker::run(ctx, config, events, ready, live, wake, open),
        )
        .map_err(|error| error.to_string())?;
        Ok(Self {
            worker,
            connected,
            running,
            commands,
            notifier,
        })
    }
    pub(super) fn send(&self, command: ButtplugWorkerCommand) -> Result<CommandReceipt, String> {
        admission::command(&command)?;
        if !self.connected.load(Ordering::Acquire) || !self.running.load(Ordering::Acquire) {
            return Err("Buttplug transport is not connected".into());
        }
        let source = self
            .commands
            .try_retain(command, COMMAND_WEIGHT)
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
    pub(crate) fn drain_events(&self, output: &mut Vec<RetainedPayload<ButtplugWorkerEvent>>) -> PendingDrain {
        self.worker.drain_events(output, NonZeroUsize::MIN)
    }
    pub(crate) fn has_pending(&self) -> bool {
        self.worker.has_pending()
    }
    pub(crate) fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }
    pub(crate) fn stop(&mut self) {
        self.connected.store(false, Ordering::Release);
        self.worker.cancel();
    }
}
impl Drop for ButtplugTransportHandle {
    fn drop(&mut self) {
        self.stop();
    }
}
