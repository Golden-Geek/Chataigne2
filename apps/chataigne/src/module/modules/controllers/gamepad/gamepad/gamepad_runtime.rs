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
mod native;
pub(crate) use model::*;
mod preparation;
use crate::app::module::script_api::NativeCallbackPreparer;
use preparation::{Draft, Prepared, Preparer};
pub(super) const CATALOG_WEIGHT: usize = 2 * 1024 * 1024;
pub(super) const ACTOR_TURN: usize = 16;
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Settings {
    pub(super) requested: String,
    pub(super) axes: [AxisProcessing; 8],
}
impl Settings {
    pub(super) fn selection(&self) -> &str {
        if self.requested.trim().is_empty() {
            "auto"
        } else {
            &self.requested
        }
    }
    fn validate(&self) -> Result<(), String> {
        if self.requested.capacity() > 1024
            || self.axes.iter().any(|a| {
                !a.dead_zone.is_finite()
                    || !a.offset.is_finite()
                    || !(0.0..=1.0).contains(&a.dead_zone)
                    || !(-1.0..=1.0).contains(&a.offset)
            })
        {
            return Err("Gamepad settings exceed selection/calibration bounds".into());
        }
        Ok(())
    }
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            requested: "auto".into(),
            axes: [AxisProcessing {
                dead_zone: 0.0,
                offset: 0.0,
            }; 8],
        }
    }
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
pub(super) trait Provider: 'static {
    fn poll(&mut self) -> Result<Vec<Record>, String>;
    fn state(&mut self, selection: &str) -> Result<Option<GamepadStateSnapshot>, String>;
}
struct Desired {
    settings: Settings,
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
    _stopped: Stopped,
    ctx: Context,
    provider: P,
    _owner: golden_io::RetirementPermit,
}
pub(super) struct GamepadRuntime {
    worker: BoundedWorker<(), RetainedPayload<Publication>>,
    actor: Option<RevisionSource>,
    shared: Arc<Shared>,
    configuration: RevisionSource,
}
fn lifetimes() -> &'static RetirementPool {
    static POOL: OnceLock<RetirementPool> = OnceLock::new();
    POOL.get_or_init(|| RetirementPool::new(NonZeroUsize::new(4).unwrap()))
}
fn publications() -> Result<PayloadPool, String> {
    static POOL: OnceLock<Mutex<Option<PayloadPool>>> = OnceLock::new();
    let mut pool = POOL
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if pool.is_none() {
        *pool = Some(PayloadPool::new("gamepad-input-publications", 128, 32 * 1024 * 1024).map_err(|e| e.to_string())?);
    }
    Ok(pool.as_ref().unwrap().clone())
}
impl GamepadRuntime {
    pub(super) fn create(owner: &RetirementPool, notifier: WorkerNotifier, settings: Settings) -> Result<Self, String> {
        Self::spawn_using(
            owner,
            lifetimes(),
            publications()?,
            notifier,
            settings,
            None,
            native::create,
        )
    }
    pub(super) fn spawn_using<P: Provider>(
        owner: &RetirementPool,
        slots: &RetirementPool,
        pool: PayloadPool,
        notifier: WorkerNotifier,
        settings: Settings,
        resources: Option<Resources>,
        open: impl FnOnce() -> Result<P, String> + Send + 'static,
    ) -> Result<Self, String> {
        settings.validate()?;
        let configuration = RevisionSource::default();
        let desired = Arc::new(Desired {
            settings,
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
            "gamepad-input-owner",
            slots,
            WorkerLimits {
                command_items: 1,
                command_weight: 1,
                event_items: 32,
                event_weight: 8 * 1024 * 1024,
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
                let resources = match resources.map_or_else(Resources::new, Ok) {
                    Ok(resources) => resources,
                    Err(error) => {
                        failure(&stopped.shared, error);
                        return;
                    }
                };
                run(
                    Owned {
                        _stopped: stopped,
                        ctx,
                        provider,
                        _owner: permit,
                    },
                    &pool,
                    &guards,
                    &resources,
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
    pub(super) fn configure(&mut self, selection: &str, axes: [AxisProcessing; 8]) -> Result<(), String> {
        if selection.len() > 1024 {
            return Err("Gamepad selection exceeds 1024 bytes".into());
        }
        let mut desired = self.shared.desired.lock().unwrap_or_else(|e| e.into_inner());
        if desired.settings.requested != selection || desired.settings.axes != axes {
            let settings = Settings {
                requested: selection.into(),
                axes,
            };
            settings.validate()?;
            let revision = self.configuration.advance();
            *desired = Arc::new(Desired {
                settings,
                guard: self.configuration.reader().guard_at(revision),
            });
        }
        Ok(())
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
    pub(super) fn poll_events(&mut self) -> Result<Vec<RetainedPayload<Publication>>, String> {
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
                .unwrap_or_else(|| "Gamepad input owner stopped unexpectedly; pending publications cancelled".into()));
        }
        Ok(events)
    }
    #[cfg(test)]
    pub(super) fn queued(&self) -> usize {
        self.worker.event_metrics().depth
    }
}
impl Drop for GamepadRuntime {
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
pub(super) struct Resources {
    pub(super) domains: PayloadPool,
    pub(super) callbacks: NativeCallbackPreparer,
}
impl Resources {
    fn new() -> Result<Self, String> {
        static DOMAINS: OnceLock<Mutex<Option<PayloadPool>>> = OnceLock::new();
        let mut domains = DOMAINS
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if domains.is_none() {
            *domains = Some(
                PayloadPool::new("gamepad-choice-domains", 128, 64 * 1024 * 1024).map_err(|error| error.to_string())?,
            );
        }
        Ok(Self {
            domains: domains.as_ref().unwrap().clone(),
            callbacks: NativeCallbackPreparer::new()?,
        })
    }
}
fn run<P: Provider>(mut owned: Owned<P>, pool: &PayloadPool, guards: &[RevisionGuard; 2], resources: &Resources) {
    let mut preparer = Preparer::new();
    let mut settings: Option<Arc<Desired>> = None;
    while !owned.ctx.cancellation().is_cancelled() {
        let desired = owned
            ._stopped
            .shared
            .desired
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if !settings.as_ref().is_some_and(|s| Arc::ptr_eq(s, &desired)) {
            settings = Some(desired.clone());
            pressure(&owned, false);
            if let Some(catalog) = &preparer.catalog {
                let devices = catalog.devices.clone();
                let state = match owned.provider.state(desired.settings.selection()) {
                    Ok(state) => state,
                    Err(error) => {
                        failure(&owned._stopped.shared, error);
                        return;
                    }
                };
                let draft = match preparer.catalog(devices, &desired.settings, state) {
                    Ok(draft) => draft,
                    Err(error) => {
                        failure(&owned._stopped.shared, error);
                        return;
                    }
                };
                if !publish(&owned, pool, guards, &desired, draft, resources) {
                    continue;
                }
            }
        }
        let rows = match owned.provider.poll() {
            Ok(rows) if rows.capacity() <= 16 => rows,
            Ok(_) => {
                failure(
                    &owned._stopped.shared,
                    "Gamepad provider turn exceeds 16 records".into(),
                );
                return;
            }
            Err(error) => {
                failure(&owned._stopped.shared, error);
                return;
            }
        };
        for row in rows {
            if !desired.guard.is_current() {
                break;
            }
            let changes_catalog = matches!(
                row,
                Record::Catalog(_)
                    | Record::Event(GamepadRuntimeEvent::Connected { .. } | GamepadRuntimeEvent::Disconnected { .. })
            );
            let state = if changes_catalog {
                match owned.provider.state(desired.settings.selection()) {
                    Ok(state) => state,
                    Err(error) => {
                        failure(&owned._stopped.shared, error);
                        return;
                    }
                }
            } else {
                None
            };
            let draft = match row {
                Record::Catalog(devices) => preparer.catalog(devices, &desired.settings, state).map(Some),
                Record::Event(event) => preparer.event(event, &desired.settings, state),
            };
            let draft = match draft {
                Ok(Some(draft)) => draft,
                Ok(None) => continue,
                Err(error) => {
                    failure(&owned._stopped.shared, error);
                    return;
                }
            };
            if !publish(&owned, pool, guards, &desired, draft, resources) {
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
    settings: &Desired,
    mut draft: Draft,
    resources: &Resources,
) -> bool {
    let weight = preparation::FRAME_WEIGHT;
    let reservation = loop {
        if owned.ctx.cancellation().is_cancelled() || !settings.guard.is_current() {
            return false;
        }
        match pool.try_reserve(weight) {
            Ok(reservation) => break reservation,
            Err(_) => pressure(owned, true),
        }
        if owned.ctx.cancellation().wait(Duration::from_millis(10)) {
            return false;
        }
    };
    let prepared = loop {
        if owned.ctx.cancellation().is_cancelled() || !settings.guard.is_current() {
            return false;
        }
        if let Some(prepared) = draft.try_finish(pool, &resources.domains, &resources.callbacks) {
            break prepared;
        }
        pressure(owned, true);
        if owned.ctx.cancellation().wait(Duration::from_millis(10)) {
            return false;
        }
    };
    let mut publication = reservation
        .retain(
            Publication {
                prepared,
                guards: [guards[0].clone(), guards[1].clone(), settings.guard.clone()],
            },
            weight,
        )
        .unwrap();
    loop {
        if owned.ctx.cancellation().is_cancelled() || !settings.guard.is_current() {
            return false;
        }
        match owned.ctx.try_publish_weighted(publication, weight) {
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
fn pressure<P>(owned: &Owned<P>, value: bool) {
    if owned._stopped.shared.pressure.swap(value, Ordering::AcqRel) != value {
        owned._stopped.shared.notifier.notify();
    }
}

#[cfg(test)]
mod tests;
