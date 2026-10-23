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
pub(super) const MAX_DEVICES: usize = 64;
pub(super) const MAX_DEVICE_TEXT: usize = 1024;
const ACTOR_TURN: usize = 16;
const INPUT_WEIGHT: usize = 4096;
const CATALOG_WEIGHT: usize = 512 * 1024;
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
    pub(super) event: MouseRuntimeEvent,
    guards: [RevisionGuard; 2],
}
impl Publication {
    pub(super) fn current(&self) -> bool {
        self.guards.iter().all(RevisionGuard::is_current)
    }
}
pub(super) trait Provider: 'static {
    fn poll(&mut self) -> Result<Vec<MouseRuntimeEvent>, String>;
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
    _stopped: Stopped,
    context: Context,
    provider: P,
    _owner: golden_io::RetirementPermit,
}
pub(crate) struct MouseInputRuntime {
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
    let mut slot = POOL
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if slot.is_none() {
        *slot =
            Some(PayloadPool::new("mouse-input-sources", 128, 32 * 1024 * 1024).map_err(|error| error.to_string())?);
    }
    Ok(slot.as_ref().unwrap().clone())
}
impl MouseInputRuntime {
    pub(crate) fn create(
        owner: &RetirementPool,
        notifier: WorkerNotifier,
        config: MouseInputConfig,
    ) -> Result<Self, String> {
        if config.selection.capacity() > MAX_DEVICE_TEXT {
            return Err("mouse selection exceeds 1024 bytes".into());
        }
        Self::spawn_guarded_using(owner, lifetimes(), sources()?, notifier, move |guard| {
            native::create(config, guard)
        })
    }
    #[cfg(test)]
    pub(super) fn spawn_using<P: Provider>(
        owner: &RetirementPool,
        slots: &RetirementPool,
        pool: PayloadPool,
        notifier: WorkerNotifier,
        open: impl FnOnce() -> Result<P, String> + Send + 'static,
    ) -> Result<Self, String> {
        Self::spawn_guarded_using(owner, slots, pool, notifier, |_| open())
    }
    pub(super) fn spawn_guarded_using<P: Provider>(
        owner: &RetirementPool,
        slots: &RetirementPool,
        pool: PayloadPool,
        notifier: WorkerNotifier,
        open: impl FnOnce(RevisionGuard) -> Result<P, String> + Send + 'static,
    ) -> Result<Self, String> {
        let permit = owner.try_reserve().map_err(|error| error.to_string())?;
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
            "mouse-input-owner",
            slots,
            WorkerLimits {
                command_items: 1,
                command_weight: 1,
                event_items: 32,
                event_weight: 8 * 1024 * 1024,
            },
            notifier,
            move |context| {
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
                run(
                    Owned {
                        _stopped: stopped,
                        context,
                        provider,
                        _owner: permit,
                    },
                    &pool,
                    &guards,
                );
            },
        )
        .map_err(|error| error.to_string())?;
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
                .unwrap_or_else(|error| error.into_inner())
                .clone()
                .unwrap_or_else(|| "mouse input worker stopped unexpectedly; pending input cancelled".into()));
        }
        Ok(events)
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
            Ok(CATALOG_WEIGHT)
        }
        MouseRuntimeEvent::Input { device, .. } if valid(device) => Ok(INPUT_WEIGHT),
        _ => Err("mouse input device id exceeds 1024 bytes".into()),
    }
}
fn run<P: Provider>(mut owned: Owned<P>, pool: &PayloadPool, guards: &[RevisionGuard; 2]) {
    while !owned.context.cancellation().is_cancelled() {
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
            let weight = match validate(&event) {
                Ok(weight) => weight,
                Err(error) => {
                    failure(&owned._stopped.shared, error);
                    return;
                }
            };
            let mut pending = Some(event);
            let mut retained = loop {
                if owned.context.cancellation().is_cancelled() {
                    return;
                }
                match pool.try_reserve(weight) {
                    Ok(reservation) => {
                        break reservation
                            .retain(
                                Publication {
                                    event: pending.take().unwrap(),
                                    guards: guards.clone(),
                                },
                                weight,
                            )
                            .unwrap()
                    }
                    Err(_) => owned._stopped.shared.pressure.store(true, Ordering::Release),
                }
                if owned.context.cancellation().wait(Duration::from_millis(10)) {
                    return;
                }
            };
            loop {
                if owned.context.cancellation().is_cancelled() {
                    return;
                }
                match owned.context.try_publish_weighted(retained, weight) {
                    Ok(()) => {
                        owned._stopped.shared.pressure.store(false, Ordering::Release);
                        break;
                    }
                    Err(PendingSendError::Full(value)) => retained = value,
                    Err(PendingSendError::Disconnected(_)) => return,
                }
                owned._stopped.shared.pressure.store(true, Ordering::Release);
                if owned.context.cancellation().wait(Duration::from_millis(10)) {
                    return;
                }
            }
        }
        if owned.context.cancellation().wait(Duration::from_millis(8)) {
            return;
        }
    }
}
#[cfg(test)]
mod tests;
