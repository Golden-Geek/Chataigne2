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
mod model;
pub(crate) use model::*;
mod native;
mod preparation;
pub(super) use preparation::Prepared;
use preparation::Preparer;
pub(super) const EVENT_WEIGHT: usize = 16 * 1024;
pub(super) const ACTOR_TURN: usize = 16;
pub(super) trait Provider: 'static {
    fn poll(&mut self) -> Result<Vec<KinectRuntimePoll>, String>;
}
pub(super) struct Publication {
    pub(super) prepared: Prepared,
    pub(super) guards: [RevisionGuard; 3],
}
impl Publication {
    pub(super) fn current(&self) -> bool {
        self.guards.iter().all(RevisionGuard::is_current)
    }
}
struct Desired {
    space: KinectReferenceSpace,
    guard: RevisionGuard,
}
struct Shared {
    desired: Mutex<Arc<Desired>>,
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
    // Publish termination before provider destruction or a native cleanup can block.
    stopped: Stopped,
    ctx: Context,
    provider: P,
    _owner: golden_io::RetirementPermit,
}
pub(super) struct Kinect2Runtime {
    worker: BoundedWorker<(), RetainedPayload<Publication>>,
    actor: Option<RevisionSource>,
    shared: Arc<Shared>,
    configuration: RevisionSource,
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
        *pool = Some(PayloadPool::new("kinect2-publications", 128, 128 * EVENT_WEIGHT).map_err(|e| e.to_string())?);
    }
    Ok(pool.as_ref().unwrap().clone())
}
impl Kinect2Runtime {
    pub(super) fn create(
        owner: &RetirementPool,
        notifier: WorkerNotifier,
        space: KinectReferenceSpace,
    ) -> Result<Self, String> {
        Self::spawn_using(owner, lifetimes(), sources()?, notifier, space, native::create)
    }
    pub(super) fn spawn_using<P: Provider>(
        owner: &RetirementPool,
        slots: &RetirementPool,
        pool: PayloadPool,
        notifier: WorkerNotifier,
        space: KinectReferenceSpace,
        open: impl FnOnce() -> Result<P, String> + Send + 'static,
    ) -> Result<Self, String> {
        let configuration = RevisionSource::default();
        let desired = Arc::new(Desired {
            space,
            guard: configuration.reader().guard_at(0),
        });
        let permit = owner.try_reserve().map_err(|e| e.to_string())?;
        let actor = RevisionSource::default();
        let native = RevisionSource::default();
        let guards = [actor.reader().guard_at(0), native.reader().guard_at(0)];
        let shared = Arc::new(Shared {
            desired: Mutex::new(desired),
            running: AtomicBool::new(true),
            pressure: AtomicBool::new(false),
            error: Mutex::new(None),
            notifier: notifier.clone(),
        });
        let state = shared.clone();
        let worker = BoundedWorker::spawn_notified(
            "kinect2-owner",
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
            configuration,
        })
    }
    pub(super) fn configure(&mut self, space: KinectReferenceSpace) {
        let mut desired = self.shared.desired.lock().unwrap_or_else(|e| e.into_inner());
        if desired.space != space {
            let revision = self.configuration.advance();
            *desired = Arc::new(Desired {
                space,
                guard: self.configuration.reader().guard_at(revision),
            });
        }
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
        let mut rows = Vec::with_capacity(ACTOR_TURN);
        let drain = self
            .worker
            .drain_events(&mut rows, NonZeroUsize::new(ACTOR_TURN).unwrap());
        if rows.is_empty() && (!self.is_running() || drain.state == PendingDrainState::Disconnected) {
            return Err(self
                .shared
                .error
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone()
                .unwrap_or_else(|| "Kinect owner stopped unexpectedly; pending publications cancelled".into()));
        }
        Ok(rows)
    }
    #[cfg(test)]
    pub(super) fn queued(&self) -> usize {
        self.worker.event_metrics().depth
    }
}
impl Drop for Kinect2Runtime {
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
    let mut preparer = Preparer::new();
    let mut desired_previous: Option<Arc<Desired>> = None;
    let mut latest: Option<KinectRuntimePoll> = None;
    while !owned.ctx.cancellation().is_cancelled() {
        let desired = owned
            .stopped
            .shared
            .desired
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if !desired_previous.as_ref().is_some_and(|old| Arc::ptr_eq(old, &desired)) {
            desired_previous = Some(desired.clone());
            preparer.reset_history();
            pressure(&owned, false);
            if let Some(row) = &latest {
                let mut prepared = match preparer.prepare(row.clone(), desired.space) {
                    Ok(row) => row,
                    Err(error) => {
                        failure(&owned.stopped.shared, error);
                        return;
                    }
                };
                prepared.incoming = false;
                if !publish(&owned, pool, guards, &desired, prepared) {
                    continue;
                }
            }
        }
        let rows = match owned.provider.poll() {
            Ok(rows) if rows.capacity() <= 16 => rows,
            Ok(_) => {
                failure(&owned.stopped.shared, "Kinect provider turn exceeds 16 records".into());
                return;
            }
            Err(error) => {
                failure(&owned.stopped.shared, error);
                return;
            }
        };
        for row in rows {
            if !desired.guard.is_current() {
                break;
            }
            if let Err(error) = preparation::validate(&row) {
                failure(&owned.stopped.shared, error);
                return;
            }
            if row.frame.is_some() || !row.sensor_available || latest.as_ref().is_none_or(|row| !row.sensor_available) {
                latest = Some(row.clone());
            }
            let prepared = match preparer.prepare(row, desired.space) {
                Ok(row) => row,
                Err(error) => {
                    failure(&owned.stopped.shared, error);
                    return;
                }
            };
            if !publish(&owned, pool, guards, &desired, prepared) {
                break;
            }
        }
        if owned.ctx.cancellation().wait(Duration::from_millis(8)) {
            return;
        }
    }
}
fn publish<P>(
    owned: &Owned<P>,
    pool: &PayloadPool,
    guards: &[RevisionGuard; 2],
    desired: &Desired,
    prepared: Prepared,
) -> bool {
    let mut prepared = Some(prepared);
    let mut publication = loop {
        if owned.ctx.cancellation().is_cancelled() || !desired.guard.is_current() {
            return false;
        }
        if let Ok(reservation) = pool.try_reserve(EVENT_WEIGHT) {
            break reservation
                .retain(
                    Publication {
                        prepared: prepared.take().unwrap(),
                        guards: [guards[0].clone(), guards[1].clone(), desired.guard.clone()],
                    },
                    EVENT_WEIGHT,
                )
                .unwrap();
        }
        pressure(owned, true);
        if owned.ctx.cancellation().wait(Duration::from_millis(10)) {
            return false;
        }
    };
    loop {
        if owned.ctx.cancellation().is_cancelled() || !desired.guard.is_current() {
            return false;
        }
        match owned.ctx.try_publish_weighted(publication, EVENT_WEIGHT) {
            Ok(()) => {
                pressure(owned, false);
                return true;
            }
            Err(PendingSendError::Full(value)) => publication = value,
            Err(PendingSendError::Disconnected(_)) => return false,
        }
        pressure(owned, true);
        if owned.ctx.cancellation().wait(Duration::from_millis(10)) {
            return false;
        }
    }
}
#[cfg(test)]
mod tests;
