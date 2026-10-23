use crate::app::module::common::keyboard::KeyboardKey;
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

pub(super) const MAX_DEVICES: usize = 64;
pub(super) const MAX_DEVICE_TEXT: usize = 1024;
pub(super) const INPUT_WEIGHT: usize = 4096;
pub(super) const CATALOG_WEIGHT: usize = 2 * 1024 * 1024;
const ACTOR_TURN: usize = 16;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DiscoveredKeyboardDevice {
    pub index: usize,
    pub variant_id: String,
    pub label: String,
    pub details: String,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum KeyboardInputEvent {
    KeyChanged { key: KeyboardKey, pressed: bool },
}
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum KeyboardRuntimeEvent {
    DevicesChanged(Vec<DiscoveredKeyboardDevice>),
    Input { device: String, event: KeyboardInputEvent },
}
pub(super) struct Publication {
    pub(super) event: KeyboardRuntimeEvent,
    pub(super) guards: [RevisionGuard; 2],
}
impl Publication {
    pub(super) fn current(&self) -> bool {
        self.guards.iter().all(RevisionGuard::is_current)
    }
}
pub(super) trait Provider: 'static {
    fn poll(&mut self) -> Result<Vec<KeyboardRuntimeEvent>, String>;
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
    // Termination is observable before native teardown or a hook-thread join can block.
    _stopped: Stopped,
    ctx: Context,
    provider: P,
    _owner: golden_io::RetirementPermit,
}
pub(crate) struct KeyboardInputRuntime {
    worker: BoundedWorker<(), RetainedPayload<Publication>>,
    actor: Option<RevisionSource>,
    shared: Arc<Shared>,
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
            PayloadPool::new("keyboard-input-publications", 128, 32 * 1024 * 1024)
                .map_err(|error| error.to_string())?,
        );
    }
    Ok(pool.as_ref().unwrap().clone())
}
impl KeyboardInputRuntime {
    pub(crate) fn create(owner: &RetirementPool, notifier: WorkerNotifier) -> Result<Self, String> {
        Self::spawn_using(owner, lifetimes(), publications()?, notifier, native::create)
    }
    pub(super) fn spawn_using<P: Provider>(
        owner: &RetirementPool,
        slots: &RetirementPool,
        pool: PayloadPool,
        notifier: WorkerNotifier,
        open: impl FnOnce() -> Result<P, String> + Send + 'static,
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
            "keyboard-input-owner",
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
                run(
                    Owned {
                        _stopped: stopped,
                        ctx,
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
                .unwrap_or_else(|| {
                    "keyboard input worker stopped unexpectedly; pending publications cancelled".into()
                }));
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
impl Drop for KeyboardInputRuntime {
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
fn validate(event: &KeyboardRuntimeEvent) -> Result<usize, String> {
    let valid = |value: &str| value.len() <= MAX_DEVICE_TEXT;
    match event {
        KeyboardRuntimeEvent::DevicesChanged(devices) => {
            if devices.len() > MAX_DEVICES
                || devices
                    .iter()
                    .any(|device| !valid(&device.variant_id) || !valid(&device.label) || !valid(&device.details))
            {
                return Err("keyboard catalog exceeds 64 devices / 1024 bytes per field".into());
            }
            Ok(CATALOG_WEIGHT)
        }
        KeyboardRuntimeEvent::Input { device, .. } if valid(device) => Ok(INPUT_WEIGHT),
        _ => Err("keyboard input device id exceeds 1024 bytes".into()),
    }
}
fn run<P: Provider>(mut owned: Owned<P>, pool: &PayloadPool, guards: &[RevisionGuard; 2]) {
    while !owned.ctx.cancellation().is_cancelled() {
        let events = match owned.provider.poll() {
            Ok(events) if events.len() <= ACTOR_TURN => events,
            Ok(_) => {
                failure(
                    &owned._stopped.shared,
                    "keyboard provider turn exceeds 16 records".into(),
                );
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
            let reservation = loop {
                if owned.ctx.cancellation().is_cancelled() {
                    return;
                }
                match pool.try_reserve(weight) {
                    Ok(reservation) => break reservation,
                    Err(_) => pressure(&owned, true),
                }
                if owned.ctx.cancellation().wait(Duration::from_millis(10)) {
                    return;
                }
            };
            let mut publication = reservation
                .retain(
                    Publication {
                        event,
                        guards: guards.clone(),
                    },
                    weight,
                )
                .unwrap();
            loop {
                match owned.ctx.try_publish_weighted(publication, weight) {
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
fn pressure<P>(owned: &Owned<P>, value: bool) {
    if owned._stopped.shared.pressure.swap(value, Ordering::AcqRel) != value {
        owned._stopped.shared.notifier.notify();
    }
}

#[cfg(test)]
mod tests;
