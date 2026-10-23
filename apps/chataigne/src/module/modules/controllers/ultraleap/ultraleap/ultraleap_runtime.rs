use golden_io::{
    BoundedWorker, PayloadPool, PendingDrainState, PendingSendError, RetainedPayload, RetirementPool, RevisionGuard,
    RevisionSource, WorkerContext, WorkerLimits, WorkerNotifier,
};
use std::{
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::Duration,
};
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
mod native;
mod preparation;
pub(super) use preparation::Prepared;
use preparation::Preparer;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UltraleapVec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl UltraleapVec3 {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);

    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn distance(self, other: Self) -> f64 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        let dz = other.z - self.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UltraleapHandSnapshot {
    pub active: bool,
    pub grab_strength: f64,
    pub pinch_strength: f64,
    pub pinch_distance: f64,
    pub thumb_extended: bool,
    pub index_extended: bool,
    pub middle_extended: bool,
    pub ring_extended: bool,
    pub pinky_extended: bool,
    pub palm_position: UltraleapVec3,
    pub palm_stabilized_position: UltraleapVec3,
    pub palm_velocity: UltraleapVec3,
    pub palm_direction: UltraleapVec3,
    pub palm_normal: UltraleapVec3,
}

impl Default for UltraleapHandSnapshot {
    fn default() -> Self {
        Self {
            active: false,
            grab_strength: 0.0,
            pinch_strength: 0.0,
            pinch_distance: 0.0,
            thumb_extended: false,
            index_extended: false,
            middle_extended: false,
            ring_extended: false,
            pinky_extended: false,
            palm_position: UltraleapVec3::ZERO,
            palm_stabilized_position: UltraleapVec3::ZERO,
            palm_velocity: UltraleapVec3::ZERO,
            palm_direction: UltraleapVec3::ZERO,
            palm_normal: UltraleapVec3::ZERO,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UltraleapFrameSnapshot {
    pub hand_count: usize,
    pub left: UltraleapHandSnapshot,
    pub right: UltraleapHandSnapshot,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UltraleapRuntimePoll {
    pub service_connected: bool,
    pub connected_devices: usize,
    pub frame: Option<UltraleapFrameSnapshot>,
    pub last_event: Option<String>,
}

pub(super) const EVENT_WEIGHT: usize = 16 * 1024;
pub(super) const ACTOR_TURN: usize = 16;
pub(super) trait Provider: 'static {
    fn poll(&mut self) -> Result<Vec<UltraleapRuntimePoll>, String>;
}
pub(super) struct Publication {
    pub(super) prepared: Prepared,
    pub(super) guards: [RevisionGuard; 2],
}
impl Publication {
    pub(super) fn current(&self) -> bool {
        self.guards.iter().all(RevisionGuard::is_current)
    }
}
struct Shared {
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
struct Owned<P> {
    // Failure and publication retirement become visible before SDK cleanup can block.
    stopped: Stopped,
    ctx: Context,
    provider: P,
    _owner: golden_io::RetirementPermit,
}
pub(super) struct UltraleapRuntime {
    worker: BoundedWorker<(), RetainedPayload<Publication>>,
    actor: Option<RevisionSource>,
    shared: Arc<Shared>,
}
fn lifetimes() -> &'static RetirementPool {
    static POOL: OnceLock<RetirementPool> = OnceLock::new();
    POOL.get_or_init(|| RetirementPool::new(NonZeroUsize::new(4).unwrap()))
}
fn sources() -> Result<PayloadPool, String> {
    static POOL: OnceLock<Mutex<Option<PayloadPool>>> = OnceLock::new();
    let mut pool = POOL
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if pool.is_none() {
        *pool = Some(PayloadPool::new("ultraleap-publications", 128, 2 * 1024 * 1024).map_err(|e| e.to_string())?);
    }
    Ok(pool.as_ref().unwrap().clone())
}
impl UltraleapRuntime {
    pub(super) fn create(owner: &RetirementPool, notifier: WorkerNotifier) -> Result<Self, String> {
        #[cfg(any(windows, target_os = "linux", target_os = "macos"))]
        {
            Self::spawn_using(owner, lifetimes(), sources()?, notifier, native::create)
        }
        #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
        {
            let _ = (owner, notifier);
            Err("Ultraleap not supported on this OS".into())
        }
    }
    pub(super) fn spawn_using<P: Provider>(
        owner: &RetirementPool,
        slots: &RetirementPool,
        pool: PayloadPool,
        notifier: WorkerNotifier,
        open: impl FnOnce() -> Result<P, String> + Send + 'static,
    ) -> Result<Self, String> {
        let permit = owner.try_reserve().map_err(|e| e.to_string())?;
        let actor = RevisionSource::default();
        let native = RevisionSource::default();
        let guards = [actor.reader().guard_at(0), native.reader().guard_at(0)];
        let shared = Arc::new(Shared {
            running: AtomicBool::new(true),
            pressure: AtomicBool::new(false),
            error: Mutex::new(None),
            notifier: notifier.clone(),
        });
        let state = shared.clone();
        let worker = BoundedWorker::spawn_notified(
            "ultraleap-owner",
            slots,
            WorkerLimits {
                command_items: 1,
                command_weight: 1,
                event_items: 32,
                event_weight: 32 * EVENT_WEIGHT,
            },
            notifier,
            move |ctx| {
                let stopped = Stopped {
                    shared: state,
                    native: Some(native),
                };
                let provider = match open() {
                    Ok(provider) => provider,
                    Err(error) => {
                        failure(&stopped.shared, error);
                        return;
                    }
                };
                run(
                    Owned {
                        stopped,
                        ctx,
                        provider,
                        _owner: permit,
                    },
                    &pool,
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
    pub(super) fn has_pending(&self) -> bool {
        self.worker.event_metrics().depth > 0
    }
    pub(super) fn is_running(&self) -> bool {
        self.shared.running.load(Ordering::Acquire)
    }
    pub(super) fn pressure(&self) -> bool {
        self.shared.pressure.load(Ordering::Acquire)
    }
    pub(super) fn poll(&mut self) -> Result<Vec<RetainedPayload<Publication>>, String> {
        let mut events = Vec::with_capacity(ACTOR_TURN);
        let drain = self
            .worker
            .drain_events(&mut events, NonZeroUsize::new(ACTOR_TURN).unwrap());
        if events.is_empty() && (!self.is_running() || drain.state == PendingDrainState::Disconnected) {
            return Err(self
                .shared
                .error
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone()
                .unwrap_or_else(|| "Ultraleap owner stopped unexpectedly; pending publications cancelled".into()));
        }
        Ok(events)
    }
    #[cfg(test)]
    pub(super) fn queued(&self) -> usize {
        self.worker.event_metrics().depth
    }
}
impl Drop for UltraleapRuntime {
    fn drop(&mut self) {
        self.actor.take();
        self.worker.cancel();
    }
}
fn failure(shared: &Shared, error: String) {
    let mut end = error.len().min(1024);
    while !error.is_char_boundary(end) {
        end -= 1;
    }
    *shared.error.lock().unwrap_or_else(|e| e.into_inner()) = Some(error[..end].into());
}
fn pressure<P>(owned: &Owned<P>, value: bool) {
    if owned.stopped.shared.pressure.swap(value, Ordering::AcqRel) != value {
        owned.stopped.shared.notifier.notify();
    }
}
fn run<P: Provider>(mut owned: Owned<P>, pool: &PayloadPool, guards: &[RevisionGuard; 2]) {
    let mut preparer = Preparer::default();
    while !owned.ctx.cancellation().is_cancelled() {
        let rows = match owned.provider.poll().and_then(preparation::coalesce) {
            Ok(rows) => rows,
            Err(error) => {
                failure(&owned.stopped.shared, error);
                return;
            }
        };
        for row in rows {
            let prepared = match preparer.prepare(row) {
                Ok(prepared) => prepared,
                Err(error) => {
                    failure(&owned.stopped.shared, error);
                    return;
                }
            };
            let mut prepared = Some(prepared);
            let mut publication = loop {
                if owned.ctx.cancellation().is_cancelled() {
                    return;
                }
                if let Ok(reservation) = pool.try_reserve(EVENT_WEIGHT) {
                    break reservation
                        .retain(
                            Publication {
                                prepared: prepared.take().unwrap(),
                                guards: guards.clone(),
                            },
                            EVENT_WEIGHT,
                        )
                        .unwrap();
                }
                pressure(&owned, true);
                if owned.ctx.cancellation().wait(Duration::from_millis(10)) {
                    return;
                }
            };
            loop {
                if owned.ctx.cancellation().is_cancelled() {
                    return;
                }
                match owned.ctx.try_publish_weighted(publication, EVENT_WEIGHT) {
                    Ok(()) => {
                        pressure(&owned, false);
                        break;
                    }
                    Err(PendingSendError::Full(value)) => publication = value,
                    Err(PendingSendError::Disconnected(_)) => return,
                }
                pressure(&owned, true);
                if owned.ctx.cancellation().wait(Duration::from_millis(10)) {
                    return;
                }
            }
        }
        if owned.ctx.cancellation().wait(Duration::from_millis(8)) {
            return;
        }
    }
}
#[cfg(test)]
mod tests;
