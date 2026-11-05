//! App policy and independent background process owners. Explicitly start only for a live project.
//! Matrix capture/renderer integration supplies complete committed demand, including final-use holds.
mod agent;
mod capture;
mod grouping;
pub(crate) use capture::PreparedApplications;
use grouping::{OwnerKey, OwnerTarget};
pub(crate) mod callbacks;
pub use callbacks::{ApplicationCallbackSubscription, ApplicationCallbacks};
mod observation;
pub use observation::{ApplicationObservations, ApplicationSubscription};
mod readiness;
mod retention;
pub(crate) mod supervision;
mod worker;
pub use retention::PlanRetainer;

use super::configuration::ApplicationRuntimeKey;
use super::{demand::ApplicationDemand, protocol::*};
use crate::app::{AppEngine, AppNode};
use golden_io::{BoundedWorker, RetirementPool, RevisionGuard, WorkerLimits, WorkerNotifier};
use std::{
    collections::{HashMap, HashSet},
    num::NonZeroUsize,
    sync::{Arc, Mutex, OnceLock},
    time::Instant,
};

const MAX_APPLICATIONS: usize = 64;
const MAX_CONSUMERS: usize = 4096;
#[derive(Clone)]
struct Target {
    retain_existing: bool,
    definition: Option<Arc<ApplicationRuntimeKey>>,
    guard: Option<RevisionGuard>,
    admission: Arc<[RevisionGuard]>,
    definition_revision: Option<u64>,
    definition_instance: Option<uuid::Uuid>,
}
#[derive(Clone, Default)]
struct Request {
    targets: HashMap<ApplicationId, Target>,
    owners: HashMap<ApplicationRuntimeId, OwnerTarget>,
    bindings: HashMap<ApplicationId, ApplicationRuntimeId>,
}
struct Shared {
    project: uuid::Uuid,
    guard: RevisionGuard,
    origin: Instant,
    desired: Mutex<Request>,
    readiness: Mutex<HashMap<ApplicationRuntimeId, ReadinessEvidence>>,
    final_use: Mutex<HashMap<ApplicationRuntimeId, Vec<golden_video::runtime::PlanLifetimeObserver>>>,
}

/// Evidence must come from the actual sender/first-frame observer, never process-running inference.
#[derive(Clone)]
pub struct ReadinessEvidence {
    pub project: uuid::Uuid,
    pub application: ApplicationId,
    pub launch: uuid::Uuid,
    pub sender: String,
    pub first_frame: bool,
}

type Statuses = HashMap<ApplicationRuntimeId, (uuid::Uuid, ApplicationProcessStatus)>;

struct ConsumerObservations {
    requested: bool,
    entries: Arc<[ApplicationRuntimeConsumer]>,
}
fn owns_process(status: &ApplicationProcessStatus) -> bool {
    status.process_id.is_some()
        || matches!(
            status.phase,
            ApplicationPhase::Starting
                | ApplicationPhase::WaitingForSender
                | ApplicationPhase::Ready
                | ApplicationPhase::Stopping
                | ApplicationPhase::RetirementUnverified
        )
}

#[derive(Clone, Default)]
pub(crate) struct RuntimeView {
    supervision: Arc<Mutex<supervision::State>>,
    callbacks: Arc<callbacks::Journal>,
    claimed: Arc<std::sync::atomic::AtomicBool>,
    origin: Arc<OnceLock<Instant>>,
    statuses: Arc<Mutex<Statuses>>,
    consumers: Arc<Mutex<HashMap<ApplicationRuntimeId, ConsumerObservations>>>,
    revision: Arc<std::sync::atomic::AtomicU64>,
    observer: Arc<Mutex<Option<(uuid::Uuid, WorkerNotifier)>>>,
}
impl RuntimeView {
    pub(crate) fn callback_state(&self) -> ApplicationCallbackState {
        self.callbacks
            .state
            .lock()
            .expect("application callback journal lock poisoned")
            .snapshot()
    }
    pub(crate) fn snapshot(&self, project: uuid::Uuid) -> ApplicationRuntimeSnapshot {
        let statuses = self.statuses.lock().expect("application observation lock poisoned");
        let callbacks = self
            .callbacks
            .state
            .lock()
            .expect("application callback journal lock poisoned");
        let callback_state = callbacks.snapshot();
        let supervision = self.supervision.lock().expect("application supervision lock poisoned");
        let mut supervision_state = supervision.snapshot.clone();
        if let Some(message) = &callback_state.fault {
            supervision_state.callbacks = ApplicationServiceStatus::Halted {
                message: message.clone(),
            };
        }
        let revision = self.revision.load(std::sync::atomic::Ordering::Acquire);
        let consumers = self.consumers.lock().expect("application consumers lock poisoned");
        let mut applications = statuses
            .iter()
            .map(|(id, (_, process))| ApplicationRuntimeStatus {
                process: process.clone(),
                consumers: consumers
                    .get(id)
                    .filter(|entry| entry.requested || owns_process(process))
                    .map(|entry| entry.entries.clone())
                    .unwrap_or_else(|| Arc::from([])),
            })
            .collect::<Vec<_>>();
        drop(consumers);
        drop(statuses);
        drop(callbacks);
        drop(supervision);
        applications.sort_by_key(|entry| entry.runtime.0);
        let elapsed_ms = self.origin.get().map_or(0, |origin| {
            u64::try_from(origin.elapsed().as_millis()).unwrap_or(u64::MAX)
        });
        ApplicationRuntimeSnapshot {
            supervision: supervision_state,
            callbacks: callback_state,
            project,
            revision,
            elapsed_ms,
            applications,
        }
    }
    fn changed(&self) {
        self.revision
            .fetch_update(
                std::sync::atomic::Ordering::Release,
                std::sync::atomic::Ordering::Relaxed,
                |revision| revision.checked_add(1),
            )
            .expect("application observation revision exhausted");
        if let Some((_, notifier)) = &*self.observer.lock().expect("application observer lock poisoned") {
            notifier.notify();
        }
        self.callbacks.notify();
    }
    fn publish(
        &self,
        id: ApplicationRuntimeId,
        owner: uuid::Uuid,
        status: &ApplicationProcessStatus,
        lifecycle: Option<&mut callbacks::Reservation>,
        project: uuid::Uuid,
        elapsed_ms: u64,
    ) {
        let mut statuses = self.statuses.lock().expect("application observation lock poisoned");
        if let Some((registered, observed)) = statuses.get_mut(&id)
            && *registered == owner
            && *observed != *status
        {
            if observed.phase != status.phase || observed.launch != status.launch {
                self.callbacks.append(lifecycle, project, elapsed_ms, status.clone());
            }
            *observed = status.clone();
            if !owns_process(status) {
                let mut consumers = self.consumers.lock().expect("application consumers lock poisoned");
                if consumers.get(&id).is_some_and(|entry| !entry.requested) {
                    consumers.remove(&id);
                }
            }
            self.changed();
        }
    }
}
struct Handle {
    key: OwnerKey,
    owner: uuid::Uuid,
    worker: BoundedWorker<(), ()>,
    view: Arc<Mutex<ApplicationProcessStatus>>,
}

/// A sole controller for one live Medias. Drop cancels without joining native cleanup.
pub struct ApplicationRuntime {
    shared: Arc<Shared>,
    view: RuntimeView,
    handles: HashMap<ApplicationRuntimeId, Handle>,
    pool: RetirementPool,
    notifier: Option<WorkerNotifier>,
}
fn global_pool() -> RetirementPool {
    static POOL: OnceLock<RetirementPool> = OnceLock::new();
    POOL.get_or_init(|| RetirementPool::new(NonZeroUsize::new(MAX_APPLICATIONS).unwrap()))
        .clone()
}
impl ApplicationRuntime {
    /// Called by a host/runtime owner after project activation, never candidate validation/Undo.
    pub fn start(engine: &AppEngine, notifier: Option<WorkerNotifier>) -> Result<Self, String> {
        Self::start_context(capture::Context::capture(engine)?, notifier)
    }
    fn start_context(context: capture::Context, notifier: Option<WorkerNotifier>) -> Result<Self, String> {
        if !context.guard.is_current() {
            return Err("application runtime project is retired".into());
        }
        use std::sync::atomic::Ordering;
        context
            .view
            .claimed
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "application runtime is already owned for this project")?;
        let origin = *context.view.origin.get_or_init(Instant::now);
        Ok(Self {
            shared: Arc::new(Shared {
                project: context.project,
                guard: context.guard,
                origin,
                desired: Mutex::new(Request::default()),
                readiness: Mutex::new(HashMap::new()),
                final_use: Mutex::new(HashMap::new()),
            }),
            view: context.view,
            handles: HashMap::new(),
            pool: global_pool(),
            notifier,
        })
    }
    /// Atomically publish a whole aggregate set. All native work stays on independent workers.
    /// Retiring/blocked owners count toward the 64 global slots, including across project replacement.
    pub fn reconcile(&mut self, engine: &AppEngine, demand: &[ApplicationDemand]) -> Result<(), String> {
        self.reconcile_guarded(engine, demand, Arc::from([]))
    }
    pub(crate) fn reconcile_guarded(
        &mut self,
        engine: &AppEngine,
        demand: &[ApplicationDemand],
        admission: Arc<[RevisionGuard]>,
    ) -> Result<(), String> {
        self.reconcile_prepared(PreparedApplications::capture(engine, demand, admission)?)
    }
    pub(crate) fn start_prepared(
        capture: &PreparedApplications,
        notifier: Option<WorkerNotifier>,
    ) -> Result<Self, String> {
        if !capture.is_current() {
            return Err("application demand capture is stale".into());
        }
        Self::start_context(capture.context.clone(), notifier)
    }
    pub(crate) fn reconcile_prepared(&mut self, capture: PreparedApplications) -> Result<(), String> {
        if !self.shared.guard.is_current() || !capture.is_current() {
            return Err("application demand capture is stale".into());
        }
        if capture.context.project != self.shared.project {
            return Err("application demand belongs to another project".into());
        }
        let old = self
            .shared
            .desired
            .lock()
            .expect("application demand lock poisoned")
            .clone();
        let request = grouping::request(
            capture.targets,
            &old,
            self.handles.iter().map(|(id, handle)| (*id, handle.key.clone())),
        );
        // Only an already-idle, unrequested owner can release its global worker slot.
        self.handles.retain(|id, handle| {
            let prune = !request.owners.contains_key(id)
                && !old.owners.contains_key(id)
                && handle.view.lock().expect("application status lock poisoned").phase == ApplicationPhase::Idle;
            if prune {
                self.view
                    .statuses
                    .lock()
                    .expect("application observation lock poisoned")
                    .remove(id);
                self.view.changed();
            }
            !prune
        });
        if self.handles.len()
            + request
                .owners
                .keys()
                .filter(|id| !self.handles.contains_key(id))
                .count()
            > MAX_APPLICATIONS
        {
            return Err("application runtime admits at most 64 active/retiring normalized keys".into());
        }
        let mut added = Vec::new();
        for &id in request.owners.keys().filter(|id| !self.handles.contains_key(id)) {
            let owner = uuid::Uuid::new_v4();
            let key = request.owners[&id].key.clone();
            let status = agent::Agent::new(id, agent::Native).status;
            let view = Arc::new(Mutex::new(status));
            let shared = self.shared.clone();
            let output = view.clone();
            let observations = self.view.clone();
            let notifier = self.notifier.clone();
            let worker = BoundedWorker::spawn(
                "mapgyver-application",
                &self.pool,
                WorkerLimits {
                    command_items: 1,
                    command_weight: 1,
                    event_items: 1,
                    event_weight: 1,
                },
                move |context| worker::run(id, owner, shared, output, observations, notifier, context),
            )
            .map_err(|e| e.to_string())?;
            added.push((
                id,
                Handle {
                    key,
                    owner,
                    worker,
                    view,
                },
            ));
        }
        let registered = added
            .iter()
            .map(|(id, handle)| {
                (
                    *id,
                    (
                        handle.owner,
                        handle.view.lock().expect("application status lock poisoned").clone(),
                    ),
                )
            })
            .collect::<Vec<_>>();
        if !registered.is_empty() {
            let mut statuses = self
                .view
                .statuses
                .lock()
                .expect("application observation lock poisoned");
            statuses.extend(registered);
            self.view.changed();
        }
        self.handles.extend(added);
        let consumers = request
            .owners
            .iter()
            .map(|(id, owner)| {
                (
                    *id,
                    ConsumerObservations {
                        requested: true,
                        entries: owner.observations.clone(),
                    },
                )
            })
            .collect::<HashMap<_, _>>();
        *self.shared.desired.lock().expect("application demand lock poisoned") = request;
        {
            let mut observed = self.view.consumers.lock().expect("application consumers lock poisoned");
            observed.retain(|id, _| self.handles.contains_key(id));
            for (id, entry) in observed.iter_mut() {
                if entry.requested && !consumers.contains_key(id) {
                    entry.requested = false;
                    entry.entries = entry
                        .entries
                        .iter()
                        .cloned()
                        .map(|mut consumer| {
                            consumer.configuration = ApplicationConfigurationState::Retained;
                            consumer
                        })
                        .collect::<Vec<_>>()
                        .into();
                }
            }
            observed.extend(consumers);
        }
        self.view.changed();
        for handle in self.handles.values() {
            handle.worker.wake_notifier().notify();
        }
        Ok(())
    }
    pub fn snapshot(&self) -> ApplicationRuntimeSnapshot {
        self.view.snapshot(self.shared.project)
    }
    pub fn observations(&self) -> ApplicationObservations {
        ApplicationObservations::new(self.shared.project, self.shared.guard.clone(), self.view.clone())
    }
    pub fn callbacks(&self) -> ApplicationCallbacks {
        ApplicationCallbacks::new(self.shared.project, self.shared.guard.clone(), self.view.clone())
    }
    pub fn readiness(&self, evidence: ReadinessEvidence) -> Result<(), String> {
        if evidence.project != self.shared.project
            || !self.shared.guard.is_current()
            || evidence.sender.is_empty()
            || evidence.sender.len() > 255
            || evidence.sender.contains('\0')
        {
            return Err("readiness belongs to a stale project or exceeds sender bounds".into());
        }
        let runtime = {
            let desired = self.shared.desired.lock().expect("application demand lock poisoned");
            let runtime = *desired
                .bindings
                .get(&evidence.application)
                .ok_or("readiness consumer is not admitted")?;
            let target = desired
                .targets
                .get(&evidence.application)
                .ok_or("readiness consumer is missing")?;
            if target.definition.is_none()
                || !target.guard.as_ref().is_some_and(RevisionGuard::is_current)
                || !target.admission.iter().all(RevisionGuard::is_current)
            {
                return Err("readiness consumer is stale or unavailable".into());
            }
            runtime
        };
        let handle = self.handles.get(&runtime).ok_or("readiness runtime is not admitted")?;
        let status = handle.view.lock().expect("application status lock poisoned");
        if status.launch != Some(evidence.launch)
            || !matches!(
                status.phase,
                ApplicationPhase::WaitingForSender | ApplicationPhase::Ready
            )
        {
            return Err("readiness belongs to a stale launch".into());
        }
        drop(status);
        self.shared
            .readiness
            .lock()
            .expect("application readiness lock poisoned")
            .insert(runtime, evidence);
        handle.worker.wake_notifier().notify();
        Ok(())
    }
    pub fn retirement(&self) -> RetirementPool {
        self.pool.clone()
    }
    pub fn plan_retainer(&self) -> PlanRetainer {
        PlanRetainer(self.shared.clone())
    }
    pub fn close(&mut self) {
        *self.shared.desired.lock().expect("application demand lock poisoned") = Request::default();
        self.view
            .consumers
            .lock()
            .expect("application consumers lock poisoned")
            .clear();
        self.view.changed();
        self.shared
            .readiness
            .lock()
            .expect("application readiness lock poisoned")
            .clear();
        for handle in self.handles.values() {
            handle.worker.wake_notifier().notify();
        }
    }
}

impl Drop for ApplicationRuntime {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests;
