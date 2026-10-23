use super::*;
use crate::app::module::common::mouse::MouseButtonKind;
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

mod native;
#[cfg(all(test, windows))]
pub(crate) use native::{
    capture_target_matches_public_selection as capture_target_matches_selection, next_mouse_position_from_raw_input,
    should_swallow_captured_mouse_input,
};
pub(super) mod preparation;
use crate::app::module::script_api::NativeCallbackPreparer;
use preparation::{Draft, Prepared, Preparer};

pub(super) const MAX_DEVICES: usize = 64;
pub(super) const MAX_DEVICE_TEXT: usize = 1024;
pub(super) const INPUT_WEIGHT: usize = 4096;
pub(super) const CATALOG_WEIGHT: usize = 2 * 1024 * 1024;
const RAW_CATALOG_WEIGHT: usize = 512 * 1024;
const ACTOR_TURN: usize = 16;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DiscoveredMouseDevice {
    pub index: usize,
    pub variant_id: String,
    pub label: String,
    pub details: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MouseInputConfig {
    pub capture_os_input: bool,
    pub selection: String,
}

impl Default for MouseInputConfig {
    fn default() -> Self {
        Self {
            capture_os_input: false,
            selection: "auto".to_string(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum MouseInputEvent {
    Moved { x: i32, y: i32, dx: i32, dy: i32 },
    ButtonChanged { button: MouseButtonKind, pressed: bool },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum MouseRuntimeEvent {
    DevicesChanged(Vec<DiscoveredMouseDevice>),
    Input { device: String, event: MouseInputEvent },
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
    fn poll(&mut self) -> Result<Vec<MouseRuntimeEvent>, String>;
}
struct Settings {
    selection: Arc<str>,
    guard: RevisionGuard,
}
struct Shared {
    desired: Mutex<Arc<Settings>>,
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
    // Termination is observable before native teardown or a hook-thread join can block.
    _stopped: Stopped,
    ctx: Context,
    provider: P,
    _owner: golden_io::RetirementPermit,
}
pub(crate) struct MouseInputRuntime {
    worker: BoundedWorker<(), RetainedPayload<Publication>>,
    actor: Option<RevisionSource>,
    shared: Arc<Shared>,
    configuration: RevisionSource,
    native_config: MouseInputConfig,
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
        .unwrap_or_else(|error| error.into_inner());
    if pool.is_none() {
        *pool = Some(
            PayloadPool::new("mouse-input-publications", 128, 32 * 1024 * 1024).map_err(|error| error.to_string())?,
        );
    }
    Ok(pool.as_ref().unwrap().clone())
}
impl MouseInputRuntime {
    pub(crate) fn create(
        owner: &RetirementPool,
        notifier: WorkerNotifier,
        config: MouseInputConfig,
    ) -> Result<Self, String> {
        if config.selection.capacity() > MAX_DEVICE_TEXT {
            return Err("Mouse selection exceeds 1024 bytes".into());
        }
        let native_config = config.clone();
        Self::spawn_guarded_using(
            owner,
            lifetimes(),
            publications()?,
            notifier,
            config,
            None,
            move |guard| native::create(native_config, guard),
        )
    }
    #[cfg(test)]
    pub(super) fn spawn_using<P: Provider>(
        owner: &RetirementPool,
        slots: &RetirementPool,
        pool: PayloadPool,
        notifier: WorkerNotifier,
        config: MouseInputConfig,
        open: impl FnOnce() -> Result<P, String> + Send + 'static,
    ) -> Result<Self, String> {
        Self::spawn_prepared_using(owner, slots, pool, notifier, config, None, open)
    }
    #[cfg(test)]
    pub(super) fn spawn_prepared_using<P: Provider>(
        owner: &RetirementPool,
        slots: &RetirementPool,
        pool: PayloadPool,
        notifier: WorkerNotifier,
        config: MouseInputConfig,
        resources: Option<Resources>,
        open: impl FnOnce() -> Result<P, String> + Send + 'static,
    ) -> Result<Self, String> {
        Self::spawn_guarded_using(owner, slots, pool, notifier, config, resources, |_| open())
    }
    fn spawn_guarded_using<P: Provider>(
        owner: &RetirementPool,
        slots: &RetirementPool,
        pool: PayloadPool,
        notifier: WorkerNotifier,
        config: MouseInputConfig,
        resources: Option<Resources>,
        open: impl FnOnce(RevisionGuard) -> Result<P, String> + Send + 'static,
    ) -> Result<Self, String> {
        let selection = config.selection.as_str();
        if config.selection.capacity() > MAX_DEVICE_TEXT {
            return Err("Mouse selection exceeds 1024 bytes".into());
        }
        let configuration = RevisionSource::default();
        let settings = Arc::new(Settings {
            selection: selection.into(),
            guard: configuration.reader().guard_at(0),
        });
        let permit = owner.try_reserve().map_err(|error| error.to_string())?;
        let actor = RevisionSource::default();
        let native = RevisionSource::default();
        let guards = [actor.reader().guard_at(0), native.reader().guard_at(0)];
        let shared = Arc::new(Shared {
            desired: Mutex::new(settings),
            running: AtomicBool::new(true),
            pressure: AtomicBool::new(false),
            error: Mutex::new(None),
            notifier: notifier.clone(),
        });
        let state = shared.clone();
        let capture_os_input = config.capture_os_input;
        let worker = BoundedWorker::spawn_notified(
            "mouse-input-owner",
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
                let provider = match open(guards[0].clone()) {
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
                    capture_os_input,
                );
            },
        )
        .map_err(|error| error.to_string())?;
        Ok(Self {
            worker,
            actor: Some(actor),
            shared,
            configuration,
            native_config: config,
        })
    }
    pub(super) fn needs_reopen(&self, capture: bool, selection: &str) -> bool {
        self.native_config.capture_os_input != capture || (capture && self.native_config.selection != selection)
    }
    pub(super) fn configure(&mut self, selection: &str) -> Result<(), String> {
        if selection.len() > MAX_DEVICE_TEXT {
            return Err("Mouse selection exceeds 1024 bytes".into());
        }
        let mut desired = self.shared.desired.lock().unwrap_or_else(|error| error.into_inner());
        if desired.selection.as_ref() != selection {
            let revision = self.configuration.advance();
            *desired = Arc::new(Settings {
                selection: selection.into(),
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
    pub(super) fn poll_events(&mut self) -> Result<Vec<RetainedPayload<Publication>>, String> {
        let mut events = Vec::with_capacity(ACTOR_TURN);
        let drain = self
            .worker
            .drain_events(&mut events, NonZeroUsize::new(ACTOR_TURN).unwrap());
        if events.is_empty()
            && (!self.shared.running.load(Ordering::Acquire) || drain.state == PendingDrainState::Disconnected)
        {
            return Err(self
                .shared
                .error
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .clone()
                .unwrap_or_else(|| "mouse input worker stopped unexpectedly; pending publications cancelled".into()));
        }
        Ok(events)
    }
    pub(super) fn pressure(&self) -> bool {
        self.shared.pressure.load(Ordering::Acquire)
    }
    #[cfg(test)]
    pub(super) fn queued(&self) -> usize {
        self.worker.event_metrics().depth
    }
}
impl Drop for MouseInputRuntime {
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
    *shared.error.lock().unwrap_or_else(|error| error.into_inner()) = Some(error[..end].into());
}
fn validate(event: &MouseRuntimeEvent) -> Result<usize, String> {
    let valid = |value: &String| value.capacity() <= MAX_DEVICE_TEXT;
    match event {
        MouseRuntimeEvent::DevicesChanged(devices) => {
            if devices.capacity() > MAX_DEVICES
                || devices
                    .iter()
                    .any(|device| !valid(&device.variant_id) || !valid(&device.label) || !valid(&device.details))
            {
                return Err("mouse catalog exceeds 64 devices / 1024 bytes per field".into());
            }
            Ok(RAW_CATALOG_WEIGHT)
        }
        MouseRuntimeEvent::Input { device, .. } if valid(device) => Ok(INPUT_WEIGHT),
        _ => Err("mouse input device id exceeds 1024 bytes".into()),
    }
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
                PayloadPool::new("mouse-choice-domains", 128, 64 * 1024 * 1024).map_err(|error| error.to_string())?,
            );
        }
        Ok(Self {
            domains: domains.as_ref().unwrap().clone(),
            callbacks: NativeCallbackPreparer::new()?,
        })
    }
}
fn run<P: Provider>(
    mut owned: Owned<P>,
    pool: &PayloadPool,
    guards: &[RevisionGuard; 2],
    resources: &Resources,
    capture: bool,
) {
    let mut preparer = Preparer::new(capture);
    let mut settings: Option<Arc<Settings>> = None;
    while !owned.ctx.cancellation().is_cancelled() {
        let desired = owned
            ._stopped
            .shared
            .desired
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        if !settings.as_ref().is_some_and(|current| Arc::ptr_eq(current, &desired)) {
            pressure(&owned, false);
            let draft = match preparer.configure(&desired.selection) {
                Ok(draft) => draft,
                Err(error) => {
                    failure(&owned._stopped.shared, error);
                    return;
                }
            };
            settings = Some(desired.clone());
            if let Some(draft) = draft {
                if !publish(&owned, pool, guards, &desired, draft, resources) {
                    continue;
                }
            }
        }
        if let Some(draft) = preparer.expire_delta(std::time::Instant::now()) {
            if !publish(&owned, pool, guards, &desired, draft, resources) {
                continue;
            }
        }
        let events = match owned.provider.poll() {
            Ok(events) if events.len() <= ACTOR_TURN => events,
            Ok(_) => {
                failure(&owned._stopped.shared, "mouse provider turn exceeds 16 records".into());
                return;
            }
            Err(error) => {
                failure(&owned._stopped.shared, error);
                return;
            }
        };
        for event in events {
            if !desired.guard.is_current() {
                break;
            }
            if let Err(error) = validate(&event) {
                failure(&owned._stopped.shared, error);
                return;
            }
            let draft = match preparer.event(event, &desired.selection) {
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
    settings: &Settings,
    mut draft: Draft,
    resources: &Resources,
) -> bool {
    let weight = draft.weight();
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
