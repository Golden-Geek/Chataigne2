use std::{
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, OnceLock,
    },
};

use crate::app::module::common::mqtt::MqttPublishRequest;
use golden_core::node::NodeId;
use golden_io::{
    BoundedWorker, PayloadPool, PendingDrain, RetainedPayload, RetirementPool, WorkerLimits, WorkerNotifier,
};

mod admission;
mod publications;
#[cfg(test)]
mod tests;
mod types;
mod worker;

use publications::AdmittedPublish;
pub(crate) use publications::{PublishReceipt, PublishStatus};
pub(crate) use types::*;

const MAX_PACKET: usize = 1024 * 1024;
const COMMAND_ITEMS: usize = 100;
const EVENT_ITEMS: usize = 128;
pub(super) const EVENT_TURN: usize = 16;
pub(super) const MAX_HOST_BYTES: usize = 1024;
pub(super) const MAX_CLIENT_BYTES: usize = 256;
pub(super) const MAX_CREDENTIAL_BYTES: usize = 4096;
pub(super) const MAX_FILTER_BYTES: usize = 1024;
pub(super) const MAX_SUBSCRIPTIONS: usize = 128;

pub(super) enum ConfigAdmissionError {
    Invalid(String),
    Unavailable(String),
}

pub(super) fn retain_configuration(
    config: MqttTransportConfig,
) -> Result<RetainedPayload<MqttTransportConfig>, ConfigAdmissionError> {
    let bytes = admission::config_bytes(&config).map_err(ConfigAdmissionError::Invalid)?;
    payload_pool(false)
        .map_err(ConfigAdmissionError::Unavailable)?
        .try_retain(config, bytes)
        .map_err(|_| ConfigAdmissionError::Unavailable("MQTT configuration lifetime capacity exhausted".into()))
}

fn lifetimes() -> &'static RetirementPool {
    static POOL: OnceLock<RetirementPool> = OnceLock::new();
    POOL.get_or_init(|| RetirementPool::new(NonZeroUsize::new(16).unwrap()))
}

fn payload_pool(incoming: bool) -> Result<PayloadPool, String> {
    static INPUT: OnceLock<Result<PayloadPool, String>> = OnceLock::new();
    static OUTPUT: OnceLock<Result<PayloadPool, String>> = OnceLock::new();
    let pool = if incoming { &INPUT } else { &OUTPUT };
    pool.get_or_init(|| {
        PayloadPool::new(
            if incoming {
                "mqtt-input-disposer"
            } else {
                "mqtt-output-disposer"
            },
            512,
            64 * 1024 * 1024,
        )
        .map_err(|error| error.to_string())
    })
    .clone()
}

pub(crate) struct MqttTransportHandle {
    worker: BoundedWorker<AdmittedPublish, MqttWorkerEvent>,
    connected: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    commands: PayloadPool,
    notifier: WorkerNotifier,
    origin: NodeId,
}

impl MqttTransportHandle {
    pub(super) fn spawn(
        config: RetainedPayload<MqttTransportConfig>,
        origin: NodeId,
        notifier: WorkerNotifier,
    ) -> Result<Self, String> {
        Self::spawn_shared_using(
            config,
            origin,
            notifier,
            lifetimes(),
            payload_pool(false)?,
            payload_pool(true)?,
            worker::NativeBroker::open,
        )
    }

    #[cfg(test)]
    fn spawn_using<B: worker::Broker>(
        config: MqttTransportConfig,
        origin: NodeId,
        notifier: WorkerNotifier,
        lifetimes: &RetirementPool,
        commands: PayloadPool,
        input: PayloadPool,
        open: impl FnOnce(&MqttTransportConfig) -> B + Send + 'static,
    ) -> Result<Self, String> {
        let bytes = admission::config_bytes(&config)?;
        let config = commands
            .try_retain(config, bytes)
            .map_err(|_| "MQTT configuration lifetime capacity exhausted")?;
        Self::spawn_shared_using(config, origin, notifier, lifetimes, commands, input, open)
    }

    fn spawn_shared_using<B: worker::Broker>(
        config: RetainedPayload<MqttTransportConfig>,
        origin: NodeId,
        notifier: WorkerNotifier,
        lifetimes: &RetirementPool,
        commands: PayloadPool,
        input: PayloadPool,
        open: impl FnOnce(&MqttTransportConfig) -> B + Send + 'static,
    ) -> Result<Self, String> {
        let connected = Arc::new(AtomicBool::new(false));
        let ready = connected.clone();
        let running = Arc::new(AtomicBool::new(true));
        let live = running.clone();
        let observer = notifier.clone();
        let worker = BoundedWorker::spawn_notified(
            "mqtt-transport",
            lifetimes,
            WorkerLimits {
                command_items: COMMAND_ITEMS,
                command_weight: 16 * 1024 * 1024,
                event_items: EVENT_ITEMS,
                event_weight: 4 * 1024 * 1024,
            },
            notifier.clone(),
            move |ctx| worker::run(ctx, config, ready, live, input, observer, open),
        )
        .map_err(|error| error.to_string())?;
        Ok(Self {
            worker,
            connected,
            running,
            commands,
            notifier,
            origin,
        })
    }

    #[cfg(test)]
    pub(crate) fn send(&self, request: MqttPublishRequest, log: bool) -> Result<PublishReceipt, String> {
        let bytes = admission::publish_bytes(&request)?;
        if !self.connected.load(Ordering::Acquire) {
            return Err("MQTT transport is not connected".into());
        }
        let request = self
            .commands
            .try_retain(request, bytes)
            .map_err(|_| "MQTT publish lifetime capacity exhausted")?;
        let (command, receipt) = AdmittedPublish::new(request, self.notifier.clone(), self.origin, log);
        self.worker.send_weighted(command, bytes).map_err(|error| match error {
            golden_io::PendingSendError::Full(_) => "MQTT request queue is full",
            golden_io::PendingSendError::Disconnected(_) => "MQTT worker is no longer running",
        })?;
        Ok(receipt)
    }

    pub(super) fn send_output(&self, request: super::output::Request, log: bool) -> Result<PublishReceipt, String> {
        if !self.connected.load(Ordering::Acquire) {
            return Err("MQTT transport is not connected".into());
        }
        let bytes = request.reservation_weight();
        let source = self
            .commands
            .try_retain(request, bytes)
            .map_err(|_| "MQTT output lifetime capacity exhausted")?;
        let (command, receipt) = AdmittedPublish::captured(source, self.notifier.clone(), self.origin, log);
        self.worker.send_weighted(command, bytes).map_err(|error| match error {
            golden_io::PendingSendError::Full(_) => "MQTT request queue is full",
            golden_io::PendingSendError::Disconnected(_) => "MQTT worker is no longer running",
        })?;
        Ok(receipt)
    }

    pub(crate) fn drain_events(&self, events: &mut Vec<MqttWorkerEvent>) -> PendingDrain {
        self.worker.drain_events(events, NonZeroUsize::MIN)
    }

    pub(crate) fn stop(&mut self) {
        self.connected.store(false, Ordering::Release);
        self.worker.cancel();
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.worker.has_pending()
    }

    pub(crate) fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Acquire)
    }

    pub(crate) fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    #[cfg(test)]
    pub(crate) fn retain_for_test(message: MqttReceivedPublish) -> golden_io::RetainedPayload<MqttReceivedPublish> {
        let bytes = admission::incoming_bytes(message.topic.capacity(), message.payload.capacity()).unwrap();
        payload_pool(true).unwrap().try_retain(message, bytes).unwrap()
    }
}

impl Drop for MqttTransportHandle {
    fn drop(&mut self) {
        self.stop();
    }
}
