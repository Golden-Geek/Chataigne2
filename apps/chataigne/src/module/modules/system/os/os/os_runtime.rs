use crate::app::module::{common::os::WakeOnLanRequest, script_api::NativeCallbackPayload};
use golden_io::{
    BoundedWorker, PayloadPool, RetainedPayload, RetirementPool, RevisionGuard, RevisionSource, WorkerLimits,
    WorkerNotifier,
};
use std::{
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
};
mod input;
mod native;
mod outcomes;
#[cfg(test)]
mod tests;
mod worker;
use input::Request;
pub(super) use input::{Input, Method, Source};
#[cfg(test)]
pub(crate) use native::{build_magic_packet, parse_mac_address};
pub(crate) use native::{host_architecture, host_os_type};
use outcomes::Work;
pub(super) use outcomes::{Outcome, Receipt};
const WEIGHT: usize = 24 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostControlAction {
    Shutdown,
    Reboot,
    Logout,
}

impl HostControlAction {
    pub(crate) fn as_human_label(self) -> &'static str {
        match self {
            Self::Shutdown => "shutdown",
            Self::Reboot => "reboot",
            Self::Logout => "logout",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct OsMetricsSnapshot {
    pub(crate) architecture: String,
    pub(crate) os_version: String,
    pub(crate) global_cpu_ratio: f64,
    pub(crate) app_cpu_ratio: f64,
    pub(crate) system_used_mb: f64,
    pub(crate) system_total_mb: f64,
    pub(crate) app_used_mb: f64,
    pub(crate) app_virtual_mb: f64,
    pub(crate) received_mb_per_sec: f64,
    pub(crate) transmitted_mb_per_sec: f64,
    pub(crate) total_received_mb: f64,
    pub(crate) total_transmitted_mb: f64,
    pub(crate) system_uptime_seconds: f64,
    pub(crate) app_uptime_seconds: f64,
}

#[derive(Clone)]
pub(super) struct Fence {
    actor: RevisionGuard,
    native: RevisionGuard,
}
impl Fence {
    pub(super) fn current(&self) -> bool {
        self.actor.is_current() && self.native.is_current()
    }
    pub(super) fn guards(&self) -> Vec<RevisionGuard> {
        vec![self.actor.clone(), self.native.clone()]
    }
}
pub(super) struct Metrics {
    pub(super) snapshot: OsMetricsSnapshot,
    pub(super) callback: NativeCallbackPayload,
    pub(super) fence: Fence,
}
pub(super) enum Status {
    Ready,
    Warning(String),
    Error(String),
}
struct Shared {
    latest: Mutex<Option<RetainedPayload<Metrics>>>,
    running: AtomicBool,
    wake: WorkerNotifier,
}
pub(super) trait Provider: Send + 'static {
    fn sample(&mut self) -> Result<OsMetricsSnapshot, String>;
    fn execute(&mut self, request: &Request) -> Result<(), String>;
}
pub(super) struct OsRuntime {
    worker: BoundedWorker<Work, Status>,
    shared: Arc<Shared>,
    source: Option<RevisionSource>,
    fence: Fence,
    commands: PayloadPool,
}
fn slots() -> &'static RetirementPool {
    static POOL: OnceLock<RetirementPool> = OnceLock::new();
    POOL.get_or_init(|| RetirementPool::new(NonZeroUsize::new(16).unwrap()))
}
fn pool(output: bool) -> Result<PayloadPool, String> {
    static INPUT: OnceLock<Mutex<Option<PayloadPool>>> = OnceLock::new();
    static OUTPUT: OnceLock<Mutex<Option<PayloadPool>>> = OnceLock::new();
    let cell = (if output { &OUTPUT } else { &INPUT }).get_or_init(|| Mutex::new(None));
    initialize_pool(cell, || {
        PayloadPool::new(
            if output { "os-command-lifetimes" } else { "os-metrics" },
            512,
            16 * 1024 * 1024,
        )
        .map_err(|error| error.to_string())
    })
}
fn initialize_pool(
    cell: &Mutex<Option<PayloadPool>>,
    create: impl FnOnce() -> Result<PayloadPool, String>,
) -> Result<PayloadPool, String> {
    let mut pool = cell.lock().unwrap_or_else(|poison| poison.into_inner());
    if pool.is_none() {
        // Thread admission can fail transiently. Keep the cell retryable rather
        // than caching the failure for the remainder of the host lifetime.
        *pool = Some(create()?);
    }
    Ok(pool.as_ref().unwrap().clone())
}
impl OsRuntime {
    pub(super) fn spawn(wake: WorkerNotifier) -> Result<Self, String> {
        Self::spawn_using(wake, slots(), pool(false)?, pool(true)?, native::Native::open)
    }
    fn spawn_using<P: Provider>(
        wake: WorkerNotifier,
        slots: &RetirementPool,
        input: PayloadPool,
        commands: PayloadPool,
        open: impl FnOnce() -> Result<P, String> + Send + 'static,
    ) -> Result<Self, String> {
        let shared = Arc::new(Shared {
            latest: Mutex::new(None),
            running: AtomicBool::new(true),
            wake: wake.clone(),
        });
        let actor = RevisionSource::default();
        let native = RevisionSource::default();
        let fence = Fence {
            actor: actor.reader().guard_at(0),
            native: native.reader().guard_at(0),
        };
        let state = shared.clone();
        let native_fence = fence.clone();
        let output = commands.clone();
        let worker = BoundedWorker::spawn_notified(
            "os-runtime",
            slots,
            WorkerLimits {
                command_items: 64,
                command_weight: 4 * 1024 * 1024,
                event_items: 4,
                event_weight: 8192,
            },
            wake,
            move |ctx| worker::run(ctx, state, native, native_fence, input, output, open),
        )
        .map_err(|error| error.to_string())?;
        Ok(Self {
            worker,
            shared,
            source: Some(actor),
            fence,
            commands,
        })
    }
    pub(super) fn capture(
        &self,
        input: Input,
        producer: golden_core::engine::NodeWaker,
        method: Method,
        time: golden_core::engine::EngineTime,
        admission: Option<golden_io::RetirementPermit>,
    ) -> Result<RetainedPayload<Source>, String> {
        if !self.is_running() || self.source.is_none() {
            return Err("OS runtime is stopped; request was not admitted".into());
        }
        let source = Source {
            time,
            _admission: admission,
            input,
            producer,
            method,
            fence: self.fence.clone(),
        };
        let weight = source.weight();
        let reservation = self
            .commands
            .try_reserve(weight)
            .map_err(|_| "OS command item/byte capacity exhausted")?;
        let source = reservation
            .retain(source, weight)
            .map_err(|_| "OS command reservation failed")?;
        Ok(source)
    }
    pub(super) fn submit(&self, source: RetainedPayload<Source>) -> Result<Receipt, RetainedPayload<Source>> {
        let weight = source.weight();
        let (work, receipt) = Work::new(source.clone(), self.shared.wake.clone());
        self.worker.send_weighted(work, weight).map_err(|error| {
            let (golden_io::PendingSendError::Full(work) | golden_io::PendingSendError::Disconnected(work)) = error;
            work.unadmitted();
            source
        })?;
        Ok(receipt)
    }
    #[cfg(test)]
    fn send(&self, input: Input, producer: golden_core::engine::NodeWaker, method: Method) -> Result<Receipt, String> {
        let source = self.capture(
            input,
            producer,
            method,
            golden_core::engine::EngineTime {
                tick: 0,
                micro: 0,
                seq: 0,
            },
            None,
        )?;
        self.submit(source)
            .map_err(|_| "OS command mailbox is full or closed".into())
    }
    pub(super) fn is_running(&self) -> bool {
        self.shared.running.load(Ordering::Acquire)
    }
    pub(super) fn has_metrics(&self) -> bool {
        self.shared
            .latest
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .is_some()
    }
    pub(super) fn take_metrics(&self) -> Option<RetainedPayload<Metrics>> {
        self.shared
            .latest
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .take()
    }
    pub(super) fn take_status(&self) -> Option<Status> {
        let mut events = Vec::with_capacity(1);
        self.worker.drain_events(&mut events, NonZeroUsize::MIN);
        events.pop()
    }
    pub(super) fn pending(&self) -> bool {
        self.worker.has_pending() || self.has_metrics() || !self.is_running()
    }
    pub(super) fn stop(&mut self) {
        self.source = None;
        self.worker.cancel();
    }
}
impl Drop for OsRuntime {
    fn drop(&mut self) {
        self.stop();
    }
}
fn diagnostic(value: String) -> String {
    let mut end = value.len().min(1024);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_owned()
}
