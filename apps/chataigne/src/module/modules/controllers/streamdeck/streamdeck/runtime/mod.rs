use super::input::{Draft, PreparedInput, Route};
use crate::app::module::script_api::NativeCallbackPreparer;
use golden_core::parameter::ParameterEnumOptions;
use golden_io::{
    BoundedWorker, PayloadPool, PendingDrainState, RetainedPayload, RetirementPool, RevisionGuard, RevisionSource,
    WorkerContext, WorkerLimits, WorkerNotifier,
};
use std::{
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
};
mod model;
pub(super) use model::*;
#[cfg(feature = "streamdeck-hid")]
mod native;
mod worker;
pub(super) const INPUT_WEIGHT: usize = 8 * 1024;
pub(super) const FRAME_WEIGHT: usize = 512 * 1024;
const STATUS_WEIGHT: usize = 256 * 1024;
pub(super) struct Resources {
    sources: PayloadPool,
    domains: PayloadPool,
    callbacks: NativeCallbackPreparer,
}
impl Resources {
    fn new(sources: PayloadPool, domains: PayloadPool) -> Result<Self, String> {
        Ok(Self {
            sources,
            domains,
            callbacks: NativeCallbackPreparer::new()?,
        })
    }
}
pub(super) struct Frame {
    pub(super) selection: String,
    pub(super) selection_guard: RevisionGuard,
    pub(super) draw_guard: RevisionGuard,
    pub(super) brightness: u8,
    pub(super) visuals: Vec<KeyVisual>,
    pub(super) routes: Vec<Route>,
}
pub(super) struct Status {
    pub(super) selection: String,
    pub(super) options: ParameterEnumOptions,
    pub(super) connected: bool,
    pub(super) message: Option<String>,
}
pub(super) enum Event {
    Input(PreparedInput),
    Status(RetainedPayload<Status>),
}
pub(super) struct Publication {
    pub(super) event: Event,
    pub(super) guards: Vec<RevisionGuard>,
    pub(super) selection: String,
}
impl Publication {
    pub(super) fn current(&self) -> bool {
        self.guards.iter().all(RevisionGuard::is_current)
    }
}
struct Shared {
    desired: Mutex<RetainedPayload<Frame>>,
    running: AtomicBool,
    pressure: AtomicBool,
    error: Mutex<Option<String>>,
    notifier: WorkerNotifier,
}
struct Stopped {
    shared: Arc<Shared>,
    native: Option<RevisionSource>,
}
impl Drop for Stopped {
    fn drop(&mut self) {
        self.native.take();
        self.shared.running.store(false, Ordering::Release);
        self.shared.notifier.notify();
    }
}
type Context = WorkerContext<(), RetainedPayload<Publication>>;
struct Owned<D> {
    // Invalidate and wake before the SDK's destruction can block. Keep the driver here,
    // rather than as a run-local whose destructor would precede this notification.
    stopped: Stopped,
    ctx: Context,
    driver: D,
    _owner: golden_io::RetirementPermit,
}
pub(super) struct Runtime {
    worker: BoundedWorker<(), RetainedPayload<Publication>>,
    actor: Option<RevisionSource>,
    shared: Arc<Shared>,
}
fn lifetimes() -> &'static RetirementPool {
    static POOL: OnceLock<RetirementPool> = OnceLock::new();
    POOL.get_or_init(|| RetirementPool::new(NonZeroUsize::new(4).unwrap()))
}
fn shared_pool(
    cell: &'static OnceLock<Mutex<Option<PayloadPool>>>,
    name: &'static str,
    items: usize,
    bytes: usize,
) -> Result<PayloadPool, String> {
    let mut pool = cell
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if pool.is_none() {
        *pool = Some(PayloadPool::new(name, items, bytes).map_err(|e| e.to_string())?);
    }
    Ok(pool.as_ref().unwrap().clone())
}
pub(super) fn sources() -> Result<PayloadPool, String> {
    static POOL: OnceLock<Mutex<Option<PayloadPool>>> = OnceLock::new();
    shared_pool(&POOL, "streamdeck-publications", 128, 32 * 1024 * 1024)
}
pub(super) fn frames() -> Result<PayloadPool, String> {
    static POOL: OnceLock<Mutex<Option<PayloadPool>>> = OnceLock::new();
    shared_pool(&POOL, "streamdeck-frames", 64, 64 * FRAME_WEIGHT)
}
fn domains() -> Result<PayloadPool, String> {
    static POOL: OnceLock<Mutex<Option<PayloadPool>>> = OnceLock::new();
    shared_pool(&POOL, "streamdeck-domains", 128, 64 * 1024 * 1024)
}
impl Runtime {
    #[cfg(feature = "streamdeck-hid")]
    pub(super) fn create(
        owner: &RetirementPool,
        notifier: WorkerNotifier,
        frame: RetainedPayload<Frame>,
    ) -> Result<Self, String> {
        Self::spawn_using(
            owner,
            lifetimes(),
            Resources::new(sources()?, domains()?)?,
            notifier,
            frame,
            native::create,
        )
    }
    pub(super) fn spawn_using<D: Driver>(
        owner: &RetirementPool,
        slots: &RetirementPool,
        resources: Resources,
        notifier: WorkerNotifier,
        frame: RetainedPayload<Frame>,
        open: impl FnOnce() -> Result<D, String> + Send + 'static,
    ) -> Result<Self, String> {
        let permit = owner.try_reserve().map_err(|e| e.to_string())?;
        let actor = RevisionSource::default();
        let native = RevisionSource::default();
        let guards = [actor.reader().guard_at(0), native.reader().guard_at(0)];
        let shared = Arc::new(Shared {
            desired: Mutex::new(frame),
            running: AtomicBool::new(true),
            pressure: AtomicBool::new(false),
            error: Mutex::new(None),
            notifier: notifier.clone(),
        });
        let state = shared.clone();
        let worker = BoundedWorker::spawn_notified(
            "streamdeck-owner",
            slots,
            WorkerLimits {
                command_items: 1,
                command_weight: 1,
                event_items: 32,
                event_weight: 32 * INPUT_WEIGHT,
            },
            notifier,
            move |ctx| {
                let stopped = Stopped {
                    shared: state,
                    native: Some(native),
                };
                let driver = match open() {
                    Ok(driver) => driver,
                    Err(error) => {
                        failure(&stopped.shared, error);
                        return;
                    }
                };
                worker::run(
                    Owned {
                        stopped,
                        ctx,
                        driver,
                        _owner: permit,
                    },
                    &resources,
                    &guards,
                );
            },
        )
        .map_err(|e| e.to_string())?;
        Ok(Self {
            worker,
            actor: Some(actor),
            shared,
        })
    }
    pub(super) fn configure(&mut self, frame: RetainedPayload<Frame>) {
        *self.shared.desired.lock().unwrap_or_else(|e| e.into_inner()) = frame;
    }
    pub(super) fn running(&self) -> bool {
        self.shared.running.load(Ordering::Acquire)
    }
    pub(super) fn pressure(&self) -> bool {
        self.shared.pressure.load(Ordering::Acquire)
    }
    pub(super) fn pending(&self) -> bool {
        self.worker.event_metrics().depth > 0
    }
    pub(super) fn poll(&mut self) -> Result<Vec<RetainedPayload<Publication>>, String> {
        let mut rows = Vec::with_capacity(16);
        let drain = self.worker.drain_events(&mut rows, NonZeroUsize::new(16).unwrap());
        if rows.is_empty() && (!self.running() || drain.state == PendingDrainState::Disconnected) {
            return Err(self
                .shared
                .error
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone()
                .unwrap_or_else(|| "Stream Deck worker stopped; pending input cancelled".into()));
        }
        Ok(rows)
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        self.actor.take();
        self.worker.cancel();
    }
}
fn bounded_error(error: String) -> String {
    let mut end = error.len().min(1024);
    while !error.is_char_boundary(end) {
        end -= 1;
    }
    error[..end].into()
}
fn failure(shared: &Shared, error: String) {
    *shared.error.lock().unwrap_or_else(|e| e.into_inner()) = Some(bounded_error(error));
}
#[cfg(test)]
pub(super) mod tests;
