use std::{
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc, Condvar, Mutex, OnceLock,
    },
    thread,
};

use golden_audio::{AudioEngine, AudioEventReceiver};
use golden_io::{
    bounded_pending_channel, PendingReceiver, PendingSendError, PendingSender, RetirementPermit, RetirementPool,
};

use super::super::prepared_event::{AudioEventDraft, EventPreparer, PreparedAudioEvent};
use super::observation::{ObservationCapture, ObservationCollector, ObservationView};
use super::RuntimeWakeSender;

type WakeSlot = Arc<Mutex<Option<RuntimeWakeSender>>>;
pub(super) const EVENT_ITEMS: usize = 64;
pub(in super::super) const EVENT_BYTES: usize = 8 * 1024 * 1024;
pub(in super::super) const EVENT_SERVICE_ITEMS: usize = 8;

#[derive(Default)]
struct Capacity {
    stopped: AtomicBool,
    lock: Mutex<()>,
    changed: Condvar,
    #[cfg(test)]
    preparation_waits: AtomicU32,
}
impl Capacity {
    fn notify(&self) {
        let guard = self.lock.lock().unwrap_or_else(|p| p.into_inner());
        drop(guard);
        self.changed.notify_all();
    }
}

pub(super) fn reserve_lifetime() -> Result<RetirementPermit, String> {
    static POOL: OnceLock<RetirementPool> = OnceLock::new();
    POOL.get_or_init(|| RetirementPool::new(NonZeroUsize::new(16).unwrap()))
        .try_reserve()
        .map_err(|error| format!("Sound Card native runtime capacity exhausted: {error}"))
}

/// The actor holds controls and observations, never the native engine's final owner.
pub(super) struct SoundCardRuntimeEvents {
    pending: PendingReceiver<PreparedAudioEvent>,
    refresh_rate_hz: Arc<AtomicU32>,
    capacity: Arc<Capacity>,
    wake: WakeSlot,
    completion: RetirementPool,
    observation: ObservationCapture,
    reader: golden_io::RetainedPayload<golden_audio::AudioObservationReader>,
}

struct NativeOwner<F: FnOnce(AudioEngine)> {
    engine: Option<AudioEngine>,
    events: Option<AudioEventReceiver>,
    pending: Option<PendingSender<PreparedAudioEvent>>,
    wake: WakeSlot,
    retire: Option<F>,
    _lifetime: RetirementPermit,
    _completion: RetirementPermit,
}

impl<F: FnOnce(AudioEngine)> Drop for NativeOwner<F> {
    fn drop(&mut self) {
        // Announce producer exit before potentially blocking native cleanup. The
        // lifetime remains charged until cleanup and all field destruction finish.
        drop(self.pending.take());
        notify(&self.wake);
        drop(self.events.take());
        if let (Some(engine), Some(retire)) = (self.engine.take(), self.retire.take()) {
            retire(engine);
        }
    }
}

fn notify(slot: &WakeSlot) {
    let wake = slot.lock().unwrap_or_else(|p| p.into_inner()).clone();
    if let Some(wake) = wake {
        wake.wake();
    }
}

impl std::fmt::Debug for SoundCardRuntimeEvents {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SoundCardRuntimeEvents")
            .field("refresh_rate_hz", &self.refresh_rate_hz.load(Ordering::Acquire))
            .field("producer_connected", &!self.pending.is_disconnected())
            .finish_non_exhaustive()
    }
}

impl SoundCardRuntimeEvents {
    pub(super) fn spawn(
        engine: AudioEngine,
        events: AudioEventReceiver,
        lifetime: RetirementPermit,
        refresh_rate_hz: u32,
    ) -> Result<Self, String> {
        Self::spawn_using(engine, events, lifetime, refresh_rate_hz, |mut engine| {
            let _ = engine.shutdown();
        })
    }

    pub(super) fn spawn_using<F>(
        engine: AudioEngine,
        events: AudioEventReceiver,
        lifetime: RetirementPermit,
        refresh_rate_hz: u32,
        retire: F,
    ) -> Result<Self, String>
    where
        F: FnOnce(AudioEngine) + Send + 'static,
    {
        Self::spawn_with_preparer(engine, events, lifetime, refresh_rate_hz, EventPreparer::new, retire)
    }

    fn spawn_with_preparer<F>(
        engine: AudioEngine,
        events: AudioEventReceiver,
        lifetime: RetirementPermit,
        refresh_rate_hz: u32,
        prepare: impl FnOnce() -> Result<EventPreparer, String>,
        retire: F,
    ) -> Result<Self, String>
    where
        F: FnOnce(AudioEngine) + Send + 'static,
    {
        let (pending_sender, pending) = bounded_pending_channel(EVENT_ITEMS, EVENT_BYTES);
        let refresh_rate_hz = Arc::new(AtomicU32::new(super::bounded_levels_update_rate_hz(refresh_rate_hz)));
        let worker_refresh_rate_hz = refresh_rate_hz.clone();
        let capacity = Arc::new(Capacity::default());
        let worker_capacity = capacity.clone();
        let wake = Arc::new(Mutex::new(None));
        let completion = RetirementPool::new(NonZeroUsize::MIN);
        let owner = NativeOwner {
            engine: Some(engine),
            events: Some(events),
            pending: Some(pending_sender),
            wake: wake.clone(),
            retire: Some(retire),
            _lifetime: lifetime,
            _completion: completion.try_reserve().expect("fresh native completion"),
        };
        // Install native ownership before fallible preparation setup. Every
        // constructor failure retires the engine while its lifetime stays charged.
        let preparer = prepare()?;
        let (observation, mut collector) =
            ObservationCollector::new(owner.engine.as_ref().expect("live engine").observations())?;
        let reader = super::observation::retain_reader(owner.engine.as_ref().expect("live engine").observations())?;
        thread::Builder::new()
            .name("chataigne-sound-card-native".to_owned())
            .spawn(move || {
                let owner = owner;
                while !worker_capacity.stopped.load(Ordering::Acquire) {
                    let interval = super::levels_update_interval(worker_refresh_rate_hz.load(Ordering::Acquire));
                    match owner
                        .events
                        .as_ref()
                        .expect("live audio receiver")
                        .recv_timeout(interval)
                    {
                        Ok(Some(event)) => {
                            collector.observe(&event);
                            collector.refresh();
                            let mut draft = AudioEventDraft::new(event);
                            let mut guard = worker_capacity.lock.lock().unwrap_or_else(|p| p.into_inner());
                            let mut event = loop {
                                if worker_capacity.stopped.load(Ordering::Acquire) {
                                    return;
                                }
                                if let Some(event) = preparer.try_prepare(&mut draft) {
                                    break event;
                                }
                                #[cfg(test)]
                                worker_capacity.preparation_waits.fetch_add(1, Ordering::AcqRel);
                                // Shared lifetime capacity is released by background
                                // disposers. A coarse native wait avoids actor polling.
                                guard = worker_capacity
                                    .changed
                                    .wait_timeout(guard, std::time::Duration::from_millis(100))
                                    .unwrap_or_else(|p| p.into_inner())
                                    .0;
                                if worker_capacity.stopped.load(Ordering::Acquire) {
                                    return;
                                }
                                drop(guard);
                                collector.refresh();
                                notify(&owner.wake);
                                guard = worker_capacity.lock.lock().unwrap_or_else(|p| p.into_inner());
                            };
                            let weight = event.retained_bytes;
                            loop {
                                if worker_capacity.stopped.load(Ordering::Acquire) {
                                    return;
                                }
                                match owner
                                    .pending
                                    .as_ref()
                                    .expect("live event sender")
                                    .send_weighted(event, weight)
                                {
                                    Ok(()) => break,
                                    Err(PendingSendError::Disconnected(_)) => return,
                                    Err(PendingSendError::Full(returned)) => {
                                        event = returned;
                                        guard = worker_capacity
                                            .changed
                                            .wait_timeout(guard, interval)
                                            .unwrap_or_else(|p| p.into_inner())
                                            .0;
                                        if worker_capacity.stopped.load(Ordering::Acquire) {
                                            return;
                                        }
                                        drop(guard);
                                        collector.refresh();
                                        notify(&owner.wake);
                                        guard = worker_capacity.lock.lock().unwrap_or_else(|p| p.into_inner());
                                    }
                                }
                            }
                            drop(guard);
                            notify(&owner.wake);
                        }
                        Ok(None) => {
                            collector.refresh();
                            notify(&owner.wake);
                        }
                        Err(_) => break,
                    }
                }
            })
            .map_err(|error| format!("failed to start Sound Card native owner: {error}"))?;
        Ok(Self {
            pending,
            refresh_rate_hz,
            capacity,
            wake,
            completion,
            observation,
            reader,
        })
    }

    pub(super) fn enable_notifications(&self, wake: RuntimeWakeSender) -> Result<(), String> {
        *self.wake.lock().unwrap_or_else(|p| p.into_inner()) = Some(wake);
        if self.pending.has_pending() || self.pending.is_disconnected() {
            notify(&self.wake);
        }
        if self.pending.is_disconnected() {
            return Err("Sound Card native owner stopped before activation".into());
        }
        Ok(())
    }

    pub(super) fn observation(&self) -> ObservationView {
        self.observation.latest()
    }

    pub(super) fn continue_service(&self) {
        notify(&self.wake);
    }

    pub(super) fn inspector_state(&self) -> golden_audio::AudioDeviceInspectorState {
        self.reader.inspect(|snapshot| snapshot.device.clone())
    }

    pub(super) fn configuration_reader(&self) -> golden_io::RetainedPayload<golden_audio::AudioObservationReader> {
        self.reader.clone()
    }

    pub(super) fn device_revision(&self) -> u64 {
        self.reader.device_revision()
    }

    pub(super) fn set_refresh_rate_hz(&self, refresh_rate_hz: u32) {
        self.refresh_rate_hz
            .store(super::bounded_levels_update_rate_hz(refresh_rate_hz), Ordering::Release);
    }

    pub(super) fn drain(&self) -> Vec<PreparedAudioEvent> {
        let mut events = Vec::new();
        self.pending
            .drain_into(&mut events, NonZeroUsize::new(EVENT_SERVICE_ITEMS).unwrap());
        if !events.is_empty() {
            self.capacity.notify();
        }
        if self.pending.has_pending() || self.pending.is_disconnected() {
            notify(&self.wake);
        }
        events
    }

    pub(super) fn stop(&self) {
        self.capacity.stopped.store(true, Ordering::Release);
        self.capacity.notify();
    }

    pub(super) fn completion(&self) -> RetirementPool {
        self.completion.clone()
    }
    pub(super) fn is_disconnected(&self) -> bool {
        self.pending.is_disconnected()
    }
    pub(super) fn has_pending(&self) -> bool {
        self.pending.has_pending()
    }
}

impl Drop for SoundCardRuntimeEvents {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests;
