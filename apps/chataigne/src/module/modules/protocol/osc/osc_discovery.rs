mod catalog;
#[cfg(test)]
mod tests;
use crate::app::module::common::network_interfaces::bounded_interface_options;
pub(super) use catalog::{Catalog, Selection};
use golden_core::parameter::ParameterEnumOption;
use golden_io::{
    BoundedWorker, PayloadPool, PendingDrainState, PendingSendError, RetirementPool, WorkerLimits, WorkerNotifier,
    WorkerReceive,
};
use std::{
    num::NonZeroUsize,
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct DiscoveryEvent {
    pub generation: u64,
    pub result: Result<Catalog, String>,
}
impl DiscoveryEvent {
    fn weight(&self) -> usize {
        std::mem::size_of::<Self>() + self.result.as_ref().map_or_else(String::capacity, Catalog::weight)
    }
}

#[derive(Clone, Copy)]
struct Profile {
    generation: u64,
    selection: Selection,
}
pub(super) struct Discovery {
    worker: BoundedWorker<(), DiscoveryEvent>,
    profile: Arc<Mutex<Profile>>,
}
impl Discovery {
    pub(super) fn spawn(selection: Selection, notifier: WorkerNotifier) -> Result<Self, String> {
        Self::spawn_using(selection, notifier, lifetimes(), bounded_interface_options)
    }
    #[cfg(test)]
    pub(super) fn spawn_fake(
        selection: Selection,
        notifier: WorkerNotifier,
        detect: impl FnMut() -> Result<Vec<ParameterEnumOption>, String> + Send + 'static,
    ) -> Result<Self, String> {
        Self::spawn_using(selection, notifier, lifetimes(), detect)
    }
    fn spawn_using(
        selection: Selection,
        notifier: WorkerNotifier,
        pool: &RetirementPool,
        mut detect: impl FnMut() -> Result<Vec<ParameterEnumOption>, String> + Send + 'static,
    ) -> Result<Self, String> {
        let profile = Arc::new(Mutex::new(Profile {
            generation: 0,
            selection,
        }));
        let source = profile.clone();
        let worker = BoundedWorker::spawn_notified(
            "osc-interface-discovery",
            pool,
            WorkerLimits {
                command_items: 1,
                command_weight: 1,
                event_items: 1,
                event_weight: 128 * 1024,
            },
            notifier,
            move |context| {
                let pool = match catalog_pool() {
                    Ok(pool) => pool,
                    Err(error) => {
                        let event = DiscoveryEvent {
                            generation: 0,
                            result: Err(error),
                        };
                        let weight = event.weight();
                        let _ = context.publish_weighted(event, weight);
                        return;
                    }
                };
                let mut previous: Option<Catalog> = None;
                let mut observed: Option<DiscoveryEvent> = None;
                let mut pending = None;
                let mut refresh = true;
                let mut deadline = Instant::now();
                loop {
                    if context.cancellation().is_cancelled() {
                        return;
                    }
                    if refresh {
                        let profile = *source.lock().unwrap_or_else(|p| p.into_inner());
                        let rows = detect();
                        if context.cancellation().is_cancelled() {
                            return;
                        }
                        let result = rows
                            .and_then(|rows| Catalog::prepare(rows, profile.selection, previous.as_ref(), &pool))
                            .map_err(|error| {
                                if error.capacity() > 4096 {
                                    "OSC interface discovery diagnostic exceeds 4096 bytes".into()
                                } else {
                                    error
                                }
                            });
                        if let Ok(catalog) = &result {
                            previous = Some(catalog.clone());
                        }
                        let event = DiscoveryEvent {
                            generation: profile.generation,
                            result,
                        };
                        if observed.as_ref() != Some(&event) {
                            pending = Some(event.clone());
                            observed = Some(event);
                        }
                        refresh = false;
                        deadline = Instant::now() + Duration::from_secs(1);
                    }
                    if let Some(event) = pending.take() {
                        let weight = event.weight();
                        match context.try_publish_weighted(event, weight) {
                            Ok(()) => {}
                            Err(PendingSendError::Full(event)) => pending = Some(event),
                            Err(PendingSendError::Disconnected(_)) => return,
                        }
                    }
                    match context.receive_until(deadline) {
                        WorkerReceive::Disconnected => return,
                        WorkerReceive::Deadline => refresh = true,
                        WorkerReceive::Command(()) => {
                            refresh |= observed.as_ref().is_none_or(|event| {
                                event.generation != source.lock().unwrap_or_else(|p| p.into_inner()).generation
                            });
                        }
                    }
                }
            },
        )
        .map_err(|error| format!("failed to start OSC interface discovery: {error}"))?;
        Ok(Self { worker, profile })
    }
    pub(super) fn select(&self, selection: Selection) -> Result<(), String> {
        let mut profile = self.profile.lock().unwrap_or_else(|p| p.into_inner());
        if profile.selection == selection {
            return Ok(());
        }
        profile.generation = profile
            .generation
            .checked_add(1)
            .ok_or("OSC discovery generation exhausted")?;
        profile.selection = selection;
        drop(profile);
        let _ = self.worker.send_weighted((), 1); // A full one-slot wake already represents the latest profile.
        Ok(())
    }
    pub(super) fn generation(&self) -> u64 {
        self.profile.lock().unwrap_or_else(|p| p.into_inner()).generation
    }
    pub(super) fn has_pending(&self) -> bool {
        self.worker.has_pending()
    }
    pub(super) fn take(&self) -> (Option<DiscoveryEvent>, bool) {
        let mut events = Vec::with_capacity(1);
        let turn = self.worker.drain_events(&mut events, NonZeroUsize::MIN);
        if turn.received != 0 {
            let _ = self.worker.send_weighted((), 1);
        }
        (events.pop(), turn.state == PendingDrainState::Disconnected)
    }
}

fn lifetimes() -> &'static RetirementPool {
    static POOL: OnceLock<RetirementPool> = OnceLock::new();
    POOL.get_or_init(|| RetirementPool::new(NonZeroUsize::new(16).unwrap()))
}

fn catalog_pool() -> Result<PayloadPool, String> {
    static POOL: OnceLock<Mutex<Option<PayloadPool>>> = OnceLock::new();
    let mut pool = POOL
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if pool.is_none() {
        *pool =
            Some(PayloadPool::new("osc-interface-catalogs", 128, 8 * 1024 * 1024).map_err(|error| error.to_string())?);
    }
    Ok(pool.as_ref().expect("installed catalog disposer").clone())
}
