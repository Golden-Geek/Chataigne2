use super::frame::{DMX_SLOT_COUNT, DmxFrame};
use golden_io::{
    BoundedWorker, PayloadPool, RetainedPayload, RetirementPool, RevisionSource, WorkerLimits, WorkerNotifier,
};
use std::{
    num::NonZeroUsize,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
mod input;
mod native;
mod output;
mod receipts;
#[cfg(test)]
mod tests;
mod types;
mod worker;
use receipts::Output;
pub(crate) use receipts::{SendReceipt, SendStatus};
pub(crate) use types::*;

const WORKER_COMMAND_CAPACITY: usize = 64;
const FRAME_WEIGHT: usize = 16 * 1024;
struct Shared {
    latest: Mutex<Option<RetainedPayload<ReceivedFrame>>>,
    replaced: AtomicU64,
    ready: AtomicBool,
    running: AtomicBool,
    wake: WorkerNotifier,
}
pub(crate) struct DmxTransportHandle {
    worker: BoundedWorker<Output, DmxWorkerEvent>,
    shared: Arc<Shared>,
    output: PayloadPool,
    config: DmxTransportConfig,
    source: Option<RevisionSource>,
}
fn lifetimes() -> &'static RetirementPool {
    static POOL: OnceLock<RetirementPool> = OnceLock::new();
    POOL.get_or_init(|| RetirementPool::new(NonZeroUsize::new(16).unwrap()))
}
fn payloads(incoming: bool) -> Result<PayloadPool, String> {
    static INPUT: OnceLock<Result<PayloadPool, String>> = OnceLock::new();
    static OUTPUT: OnceLock<Result<PayloadPool, String>> = OnceLock::new();
    let pool = if incoming { &INPUT } else { &OUTPUT };
    pool.get_or_init(|| {
        PayloadPool::new(if incoming { "dmx-input" } else { "dmx-output" }, 512, 8 * 1024 * 1024)
            .map_err(|error| error.to_string())
    })
    .clone()
}
impl DmxTransportHandle {
    pub(crate) fn spawn(config: DmxTransportConfig, wake: WorkerNotifier) -> Result<Self, String> {
        Self::spawn_using(
            config,
            wake,
            lifetimes(),
            payloads(false)?,
            payloads(true)?,
            native::WorkerEndpoint::open,
        )
    }
    fn spawn_using<E: worker::Endpoint>(
        config: DmxTransportConfig,
        wake: WorkerNotifier,
        lifetimes: &RetirementPool,
        output: PayloadPool,
        input: PayloadPool,
        open: impl FnOnce(&DmxTransportConfig) -> Result<E, String> + Send + 'static,
    ) -> Result<Self, String> {
        validate_config(&config)?;
        let shared = Arc::new(Shared {
            latest: Mutex::new(None),
            replaced: AtomicU64::new(0),
            ready: AtomicBool::new(false),
            running: AtomicBool::new(true),
            wake: wake.clone(),
        });
        let source = RevisionSource::default();
        let guard = source.reader().guard_at(0);
        let state = shared.clone();
        let native = config.clone();
        let worker = BoundedWorker::spawn_notified(
            "dmx-transport",
            lifetimes,
            WorkerLimits {
                command_items: WORKER_COMMAND_CAPACITY,
                command_weight: WORKER_COMMAND_CAPACITY * FRAME_WEIGHT,
                event_items: 4,
                event_weight: 8 * 1024,
            },
            wake,
            move |ctx| worker::run(ctx, native, state, input, guard, open),
        )
        .map_err(|error| error.to_string())?;
        Ok(Self {
            worker,
            shared,
            output,
            config,
            source: Some(source),
        })
    }
    #[cfg(test)]
    pub(crate) fn send(&self, frame: DmxFrame) -> Result<SendReceipt, String> {
        self.send_inner(frame, None)
    }
    pub(super) fn send_prepared(
        &self,
        frame: DmxFrame,
        source: RetainedPayload<super::outgoing::Prepared>,
    ) -> Result<SendReceipt, String> {
        self.send_inner(frame, Some(source))
    }
    fn send_inner(
        &self,
        frame: DmxFrame,
        source: Option<RetainedPayload<super::outgoing::Prepared>>,
    ) -> Result<SendReceipt, String> {
        validate_frame(&frame, self.config.protocol)?;
        if frame.universe != self.config.universe {
            return Err("DMX frame belongs to another configured universe".into());
        }
        if !self.is_ready() {
            return Err("DMX endpoint is opening or stopped; frame was not admitted".into());
        }
        let frame = self
            .output
            .try_retain(frame, FRAME_WEIGHT)
            .map_err(|_| "DMX output payload capacity exhausted")?;
        let (output, receipt) = Output::new(frame, source, self.shared.wake.clone());
        self.worker
            .send_weighted(output, FRAME_WEIGHT)
            .map_err(|error| match error {
                golden_io::PendingSendError::Full(output) => {
                    output.unadmitted();
                    "DMX output queue is full (64 frames)"
                }
                golden_io::PendingSendError::Disconnected(output) => {
                    output.unadmitted();
                    "DMX worker is not running"
                }
            })?;
        Ok(receipt)
    }
    pub(super) fn source_guard(&self) -> Option<golden_io::RevisionGuard> {
        Some(self.source.as_ref()?.reader().guard_at(0))
    }
    pub(crate) fn is_ready(&self) -> bool {
        self.shared.ready.load(Ordering::Acquire) && self.is_running()
    }
    pub(crate) fn is_running(&self) -> bool {
        self.shared.running.load(Ordering::Acquire)
    }
    pub(crate) fn has_frame(&self) -> bool {
        self.shared
            .latest
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .is_some()
    }
    pub(crate) fn has_pending(&self) -> bool {
        self.has_frame() || self.worker.has_pending() || !self.is_running()
    }
    pub(crate) fn take_status(&self) -> Option<DmxWorkerEvent> {
        let mut events = Vec::with_capacity(1);
        self.worker.drain_events(&mut events, NonZeroUsize::MIN);
        events.pop()
    }
    pub(crate) fn take_frame(&self) -> Option<RetainedPayload<ReceivedFrame>> {
        self.shared
            .latest
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .take()
    }
    #[cfg(test)]
    pub(crate) fn take_latest_event(&self) -> Option<DmxWorkerEvent> {
        self.take_status()
            .or_else(|| self.take_frame().map(DmxWorkerEvent::Frame))
    }
    pub(crate) fn take_replaced_frames(&self) -> u64 {
        self.shared.replaced.swap(0, Ordering::AcqRel)
    }
    pub(crate) fn stop(&mut self) {
        self.source = None;
        self.shared.ready.store(false, Ordering::Release);
        self.worker.cancel();
    }
}
impl Drop for DmxTransportHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

fn validate_config(config: &DmxTransportConfig) -> Result<(), String> {
    if config.universe == 0 || config.universe > config.protocol.maximum_universe() {
        return Err("DMX configured universe is out of range".into());
    }
    if config.receive_enabled && config.listen_port == 0 {
        return Err("DMX input port must be non-zero".into());
    }
    if config.protocol == DmxProtocol::ArtNet && config.destination.is_none() {
        return Err("Art-Net output requires a destination address".into());
    }
    Ok(())
}
fn validate_frame(frame: &DmxFrame, protocol: DmxProtocol) -> Result<(), String> {
    if frame.universe == 0
        || frame.universe > protocol.maximum_universe()
        || frame.priority > 200
        || frame.slots.len() > DMX_SLOT_COUNT
        || frame.slots.capacity() > DMX_SLOT_COUNT
    {
        return Err("DMX frame exceeds its universe/priority/512-slot storage bounds".into());
    }
    Ok(())
}
fn diagnostic(message: String) -> String {
    let mut end = message.len().min(1024);
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    message[..end].to_owned()
}
