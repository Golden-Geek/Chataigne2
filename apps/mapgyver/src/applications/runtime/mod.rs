//! App policy and independent background process owners. Explicitly start only for a live project.
//! Matrix capture/renderer integration supplies complete committed demand, including final-use holds.
mod agent;
mod worker;

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
#[derive(Clone)]
struct Target {
    epoch: uuid::Uuid,
    retain_existing: bool,
    definition: Option<Arc<ApplicationDefinition>>,
    guard: Option<RevisionGuard>,
}
struct Request {
    targets: HashMap<ApplicationId, Target>,
}
struct Shared {
    project: uuid::Uuid,
    guard: RevisionGuard,
    origin: Instant,
    desired: Mutex<Request>,
    readiness: Mutex<HashMap<ApplicationId, ReadinessEvidence>>,
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

type Statuses = HashMap<ApplicationId, (uuid::Uuid, ApplicationRuntimeStatus)>;

#[derive(Clone, Default)]
pub(crate) struct RuntimeView {
    claimed: Arc<std::sync::atomic::AtomicBool>,
    origin: Arc<OnceLock<Instant>>,
    statuses: Arc<Mutex<Statuses>>,
}
impl RuntimeView {
    pub(crate) fn snapshot(&self, project: uuid::Uuid) -> ApplicationRuntimeSnapshot {
        let mut applications = self
            .statuses
            .lock()
            .expect("application observation lock poisoned")
            .values()
            .map(|(_, status)| status.clone())
            .collect::<Vec<_>>();
        applications.sort_by_key(|entry| entry.application.0.0);
        let elapsed_ms = self.origin.get().map_or(0, |origin| {
            u64::try_from(origin.elapsed().as_millis()).unwrap_or(u64::MAX)
        });
        ApplicationRuntimeSnapshot {
            project,
            elapsed_ms,
            applications,
        }
    }
}
struct Handle {
    owner: uuid::Uuid,
    worker: BoundedWorker<(), ()>,
    view: Arc<Mutex<ApplicationRuntimeStatus>>,
}

/// A sole controller for one live Media Manager. Drop cancels without joining native cleanup.
pub struct ApplicationRuntime {
    shared: Arc<Shared>,
    view: RuntimeView,
    handles: HashMap<ApplicationId, Handle>,
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
        let id = crate::media::authoring::manager(engine)?;
        let Some(AppNode::MediaManager(manager)) = engine.nodes.get(id) else {
            unreachable!()
        };
        use std::sync::atomic::Ordering;
        manager
            .applications
            .claimed
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "application runtime is already owned for this project")?;
        let origin = *manager.applications.origin.get_or_init(Instant::now);
        Ok(Self {
            shared: Arc::new(Shared {
                project: manager.instance,
                guard: manager.project_guard(),
                origin,
                desired: Mutex::new(Request {
                    targets: HashMap::new(),
                }),
                readiness: Mutex::new(HashMap::new()),
            }),
            view: manager.applications.clone(),
            handles: HashMap::new(),
            pool: global_pool(),
            notifier,
        })
    }
    /// Atomically publish a whole aggregate set. All native work stays on independent workers.
    /// Retiring/blocked owners count toward the 64 global slots, including across project replacement.
    pub fn reconcile(&mut self, engine: &AppEngine, demand: &[ApplicationDemand]) -> Result<(), String> {
        if !self.shared.guard.is_current() {
            return Err("application runtime project is retired".into());
        }
        let manager = crate::media::authoring::manager(engine)?;
        if !matches!(engine.nodes.get(manager), Some(AppNode::MediaManager(node)) if node.instance == self.shared.project)
        {
            return Err("application demand belongs to another project".into());
        }
        if demand.len() > MAX_APPLICATIONS {
            return Err("application runtime admits at most 64 active/retiring definitions".into());
        }
        let old = self
            .shared
            .desired
            .lock()
            .expect("application demand lock poisoned")
            .targets
            .clone();
        let mut targets = HashMap::new();
        let mut identities = HashSet::new();
        for entry in demand {
            if entry.reasons.is_empty() {
                return Err("application demand requires a lease reason".into());
            }
            if !identities.insert(entry.application) {
                return Err("duplicate application demand identity".into());
            }
            let mut target = match engine
                .node_id_by_uuid(entry.application.0)
                .and_then(|id| engine.nodes.get(id))
            {
                Some(AppNode::Application(node)) => {
                    let snapshot = node.snapshot();
                    let definition = if snapshot.enabled && snapshot.definition.executable.is_some() {
                        snapshot.definition.validate()?;
                        Some(Arc::new(snapshot.definition))
                    } else {
                        None
                    };
                    Target {
                        epoch: uuid::Uuid::new_v4(),
                        retain_existing: false,
                        definition,
                        guard: Some(node.guard()),
                    }
                }
                _ => Target {
                    epoch: uuid::Uuid::new_v4(),
                    retain_existing: false,
                    definition: None,
                    guard: None,
                },
            };
            target.retain_existing = entry.reasons.iter().any(|reason| {
                matches!(
                    reason,
                    super::demand::DemandReason::Outgoing | super::demand::DemandReason::FinalUse
                )
            });
            if target.definition.is_none()
                && entry
                    .reasons
                    .iter()
                    .all(|reason| *reason == super::demand::DemandReason::Neighbor)
            {
                continue;
            }
            if let Some(previous) = old.get(&entry.application)
                && previous.definition == target.definition
                && previous.guard.as_ref().is_some_and(RevisionGuard::is_current)
            {
                target.epoch = previous.epoch;
            }
            targets.insert(entry.application, target);
        }
        // Prune only owners already idle under the old set; cancellation fences an in-flight create.
        self.handles.retain(|id, handle| {
            let prune = !targets.contains_key(id)
                && !old.contains_key(id)
                && handle.view.lock().expect("application status lock poisoned").phase == ApplicationPhase::Idle;
            if prune {
                self.view
                    .statuses
                    .lock()
                    .expect("application observation lock poisoned")
                    .remove(id);
            }
            !prune
        });
        if self.handles.len() + targets.keys().filter(|id| !self.handles.contains_key(id)).count() > MAX_APPLICATIONS {
            return Err("application runtime capacity remains held by retiring owners".into());
        }
        let mut added = Vec::new();
        for &id in targets.keys().filter(|id| !self.handles.contains_key(id)) {
            let owner = uuid::Uuid::new_v4();
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
            added.push((id, Handle { owner, worker, view }));
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
        self.view
            .statuses
            .lock()
            .expect("application observation lock poisoned")
            .extend(registered);
        self.handles.extend(added);
        *self.shared.desired.lock().expect("application demand lock poisoned") = Request { targets };
        for handle in self.handles.values() {
            handle.worker.wake_notifier().notify();
        }
        Ok(())
    }
    pub fn snapshot(&self) -> ApplicationRuntimeSnapshot {
        self.view.snapshot(self.shared.project)
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
        let handle = self
            .handles
            .get(&evidence.application)
            .ok_or("readiness application is not admitted")?;
        let status = handle.view.lock().expect("application status lock poisoned");
        if status.launch != Some(evidence.launch)
            || !matches!(
                status.phase,
                ApplicationPhase::WaitingForSender | ApplicationPhase::Ready
            )
            || !self
                .shared
                .desired
                .lock()
                .expect("application demand lock poisoned")
                .targets
                .contains_key(&evidence.application)
        {
            return Err("readiness belongs to a stale launch".into());
        }
        drop(status);
        self.shared
            .readiness
            .lock()
            .expect("application readiness lock poisoned")
            .insert(evidence.application, evidence);
        handle.worker.wake_notifier().notify();
        Ok(())
    }
    pub fn retirement(&self) -> RetirementPool {
        self.pool.clone()
    }
    pub fn close(&mut self) {
        self.shared
            .desired
            .lock()
            .expect("application demand lock poisoned")
            .targets
            .clear();
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

#[cfg(test)]
mod tests;
