use super::*;
use golden_io::{PayloadPool, PendingSendError, WorkerReceive};
use std::sync::Mutex;
mod catalog;
pub(crate) use catalog::{PortCatalog, PreparedCatalog, Selections};
#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CatalogEvent {
    pub generation: u64,
    pub result: Result<PreparedCatalog, String>,
}
impl CatalogEvent {
    fn weight(&self) -> usize {
        std::mem::size_of::<Self>()
            + self
                .result
                .as_ref()
                .map_or_else(String::capacity, PreparedCatalog::weight)
    }
}
#[derive(Clone)]
struct Profile {
    generation: u64,
    selections: Selections,
}
pub(crate) struct MidiDiscovery {
    worker: BoundedWorker<(), CatalogEvent>,
    profile: Arc<Mutex<Profile>>,
}
impl MidiDiscovery {
    pub(crate) fn spawn(input: &str, output: &str, notifier: WorkerNotifier) -> Result<Self, String> {
        Self::spawn_using(lifetimes(), notifier, input, output, ports::available_midi_port_options)
    }
    pub(super) fn spawn_using(
        pool: &RetirementPool,
        notifier: WorkerNotifier,
        input: &str,
        output: &str,
        mut detect: impl FnMut() -> Result<MidiPortOptions, String> + Send + 'static,
    ) -> Result<Self, String> {
        let profile = Arc::new(Mutex::new(Profile {
            generation: 0,
            selections: Selections::new(input, output)?,
        }));
        let source = profile.clone();
        let worker = BoundedWorker::spawn_notified(
            "midi-discovery",
            pool,
            WorkerLimits {
                command_items: 1,
                command_weight: 1,
                event_items: 1,
                event_weight: 1024 * 1024,
            },
            notifier,
            move |ctx| {
                let pool = match catalog_pool() {
                    Ok(pool) => pool,
                    Err(error) => {
                        let event = CatalogEvent {
                            generation: 0,
                            result: Err(diagnostic(error)),
                        };
                        let weight = event.weight();
                        let _ = ctx.publish_weighted(event, weight);
                        return;
                    }
                };
                let mut previous = None;
                let mut observed: Option<CatalogEvent> = None;
                let mut pending = None;
                let mut refresh = true;
                let mut deadline = Instant::now();
                loop {
                    if ctx.cancellation().is_cancelled() {
                        return;
                    }
                    if refresh {
                        let profile = source.lock().unwrap_or_else(|p| p.into_inner()).clone();
                        let raw = detect();
                        if ctx.cancellation().is_cancelled() {
                            return;
                        }
                        let result = raw
                            .and_then(|raw| {
                                PreparedCatalog::prepare(raw, &profile.selections, previous.as_ref(), &pool)
                            })
                            .map_err(diagnostic);
                        if let Ok(catalog) = &result {
                            previous = Some(catalog.clone());
                        }
                        let event = CatalogEvent {
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
                        match ctx.try_publish_weighted(event, weight) {
                            Ok(()) => {}
                            Err(PendingSendError::Full(event)) => pending = Some(event),
                            Err(PendingSendError::Disconnected(_)) => return,
                        }
                    }
                    match ctx.receive_until(deadline) {
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
        .map_err(|error| format!("failed to start MIDI discovery: {error}"))?;
        Ok(Self { worker, profile })
    }
    pub(crate) fn select(&self, input: &str, output: &str) -> Result<(), String> {
        let mut profile = self.profile.lock().unwrap_or_else(|p| p.into_inner());
        if profile.selections.input == input && profile.selections.output == output {
            return Ok(());
        }
        let selections = Selections::new(input, output)?;
        profile.generation = profile
            .generation
            .checked_add(1)
            .ok_or("MIDI catalog generation exhausted")?;
        profile.selections = selections;
        drop(profile);
        let _ = self.worker.send_weighted((), 1);
        Ok(())
    }
    pub(crate) fn generation(&self) -> u64 {
        self.profile.lock().unwrap_or_else(|p| p.into_inner()).generation
    }
    pub(crate) fn has_pending(&self) -> bool {
        self.worker.has_pending()
    }
    pub(crate) fn take(&self) -> (Option<CatalogEvent>, bool) {
        let mut events = Vec::with_capacity(1);
        let drain = self.worker.drain_events(&mut events, NonZeroUsize::MIN);
        if drain.received != 0 {
            let _ = self.worker.send_weighted((), 1);
        }
        (events.pop(), drain.state == PendingDrainState::Disconnected)
    }
}

fn catalog_pool() -> Result<PayloadPool, String> {
    static POOL: OnceLock<Mutex<Option<PayloadPool>>> = OnceLock::new();
    let mut pool = POOL
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if pool.is_none() {
        *pool = Some(PayloadPool::new("midi-catalogs", 128, 32 * 1024 * 1024).map_err(|error| error.to_string())?);
    }
    Ok(pool.as_ref().expect("installed MIDI catalog disposer").clone())
}
