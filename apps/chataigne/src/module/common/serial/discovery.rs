mod catalog;
#[cfg(test)]
mod tests;
mod worker;

use super::DiscoveredSerialPort;
pub(crate) use catalog::SerialCatalog;
use golden_io::{RetainedPayload, RetirementPool, WorkerNotifier};
use std::{
    collections::BTreeMap,
    num::NonZeroUsize,
    sync::{Arc, LazyLock, Mutex},
    time::{Duration, Instant},
};
use worker::SerialDiscoveryWorker;

const MAX_SUBSCRIBERS: usize = 1024;
#[derive(Clone, Debug, Default)]
pub(crate) struct SerialDiscoverySnapshot {
    pub version: u64,
    pub catalog: Option<RetainedPayload<SerialCatalog>>,
    pub error: Option<String>,
}
pub(crate) struct SerialDiscoveryRegistration {
    id: u64,
    service: Arc<Service>,
}
struct State {
    next_id: u64,
    epoch: u64,
    subscribers: BTreeMap<u64, WorkerNotifier>,
    snapshot: SerialDiscoverySnapshot,
    worker: Option<SerialDiscoveryWorker>,
    starting: bool,
    retry_at: Option<Instant>,
}
type Detector = dyn Fn() -> Result<Vec<DiscoveredSerialPort>, String> + Send + Sync;
struct Service {
    state: Mutex<State>,
    pool: RetirementPool,
    detector: Arc<Detector>,
}
static SERVICE: LazyLock<Arc<Service>> = LazyLock::new(|| Service::new(worker::detect_available_serial_ports));
impl Service {
    fn new(detector: impl Fn() -> Result<Vec<DiscoveredSerialPort>, String> + Send + Sync + 'static) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(State {
                next_id: 1,
                epoch: 0,
                subscribers: BTreeMap::new(),
                snapshot: Default::default(),
                worker: None,
                starting: false,
                retry_at: None,
            }),
            pool: RetirementPool::new(NonZeroUsize::MIN),
            detector: Arc::new(detector),
        })
    }
    fn register(self: &Arc<Self>, notifier: WorkerNotifier) -> Result<SerialDiscoveryRegistration, String> {
        let id = {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            if state.subscribers.len() >= MAX_SUBSCRIBERS {
                return Err("Serial discovery subscriber capacity exhausted".into());
            }
            let id = state.next_id;
            state.next_id = id.checked_add(1).ok_or("Serial discovery identity exhausted")?;
            state.subscribers.insert(id, notifier);
            id
        };
        self.ensure();
        Ok(SerialDiscoveryRegistration {
            id,
            service: self.clone(),
        })
    }
    fn ensure(self: &Arc<Self>) {
        let mut changed = false;
        let epoch = {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            if state.subscribers.is_empty() {
                return;
            }
            if state.worker.as_ref().is_some_and(SerialDiscoveryWorker::stopped) {
                state.worker = None;
                state.retry_at = Some(Instant::now() + Duration::from_secs(1));
                changed |= set_error(&mut state, Some("Serial discovery producer stopped; retrying".into()));
            }
            if state.worker.is_none() && !state.starting && state.retry_at.is_none_or(|at| Instant::now() >= at) {
                state.epoch = state.epoch.checked_add(1).expect("serial discovery epoch exhausted");
                state.starting = true;
                Some(state.epoch)
            } else {
                None
            }
        };
        if changed {
            self.notify();
        }
        let Some(epoch) = epoch else { return };
        let detector = self.detector.clone();
        let publish = Arc::downgrade(self);
        let exit = Arc::downgrade(self);
        // Thread startup failure may synchronously notify producer disconnection.
        // Neither startup nor subscriber callbacks may run under the catalog lock.
        let result = SerialDiscoveryWorker::spawn_using(
            &self.pool,
            move || detector(),
            move |ports, error| {
                if let Some(service) = publish.upgrade() {
                    service.publish(epoch, ports, error);
                }
            },
            WorkerNotifier::new(move || {
                if let Some(service) = exit.upgrade() {
                    service.notify_epoch(epoch);
                }
            }),
        );
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if state.epoch != epoch || state.subscribers.is_empty() {
            drop(state);
            drop(result);
            return;
        }
        state.starting = false;
        let changed = match result {
            Ok(worker) => {
                state.worker = Some(worker);
                state.retry_at = None;
                false
            }
            Err(error) => {
                state.retry_at = Some(Instant::now() + Duration::from_secs(1));
                set_error(&mut state, Some(bounded_error(error)))
            }
        };
        drop(state);
        if changed {
            self.notify_epoch(epoch);
        }
    }
    fn publish(&self, epoch: u64, ports: Vec<DiscoveredSerialPort>, error: Option<String>) {
        let previous = {
            let state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            if state.epoch != epoch || state.subscribers.is_empty() {
                return;
            }
            state.snapshot.clone()
        };
        let (catalog, error, same_ports) = if previous.catalog.as_ref().is_some_and(|catalog| catalog.ports == ports) {
            (previous.catalog.clone(), error.map(bounded_error), true)
        } else {
            match SerialCatalog::prepare(ports) {
                Ok(catalog) => (Some(catalog), error.map(bounded_error), false),
                Err(error) => (previous.catalog.clone(), Some(bounded_error(error)), true),
            }
        };
        let changed = {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            if state.epoch != epoch || state.subscribers.is_empty() {
                return;
            }
            if same_ports && state.snapshot.error == error {
                false
            } else {
                state.snapshot.version = state
                    .snapshot
                    .version
                    .checked_add(1)
                    .expect("serial catalog version exhausted");
                state.snapshot.catalog = catalog;
                state.snapshot.error = error;
                true
            }
        };
        if changed {
            self.notify_epoch(epoch);
        }
    }
    fn notify_epoch(&self, epoch: u64) {
        let subscribers = {
            let state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            if state.epoch != epoch {
                return;
            }
            state.subscribers.values().cloned().collect::<Vec<_>>()
        };
        for notifier in subscribers {
            notifier.notify();
        }
    }
    fn notify(&self) {
        let subscribers = self
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .subscribers
            .values()
            .cloned()
            .collect::<Vec<_>>();
        // Notifications may read the catalog again; never invoke them under the service lock.
        for notifier in subscribers {
            notifier.notify();
        }
    }
}
fn bounded_error(error: String) -> String {
    if error.capacity() <= 4096 {
        error
    } else {
        "Serial discovery error details exceed 4096 bytes".into()
    }
}
fn set_error(state: &mut State, error: Option<String>) -> bool {
    if state.snapshot.error == error {
        return false;
    }
    state.snapshot.version = state
        .snapshot
        .version
        .checked_add(1)
        .expect("serial catalog version exhausted");
    state.snapshot.error = error;
    true
}
impl SerialDiscoveryRegistration {
    pub(crate) fn version(&self) -> u64 {
        self.service
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .snapshot
            .version
    }
    pub(crate) fn register(notifier: WorkerNotifier) -> Result<Self, String> {
        SERVICE.register(notifier)
    }
    pub(crate) fn snapshot(&self) -> SerialDiscoverySnapshot {
        self.service.ensure();
        self.service
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .snapshot
            .clone()
    }
    pub(crate) fn retry_after(&self) -> Option<Duration> {
        self.service
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .retry_at
            .map(|at| at.saturating_duration_since(Instant::now()))
    }
}
impl Drop for SerialDiscoveryRegistration {
    fn drop(&mut self) {
        let worker = {
            let mut state = self.service.state.lock().unwrap_or_else(|p| p.into_inner());
            state.subscribers.remove(&self.id);
            if state.subscribers.is_empty() {
                state.epoch = state.epoch.checked_add(1).expect("serial discovery epoch exhausted");
                state.starting = false;
                state.snapshot.catalog = None;
                state.snapshot.error = None;
                state.retry_at = None;
                state.worker.take()
            } else {
                None
            }
        };
        if let Some(mut worker) = worker {
            worker.stop();
        }
    }
}
