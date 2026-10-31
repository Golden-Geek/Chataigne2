use super::{
    demand::ApplicationDemand,
    logical::{LogicalApplicationCapture, capture_logical_applications},
    runtime::{ApplicationRuntime, PlanRetainer},
};
use crate::app::{AppEngine, AppNode};
use golden_io::{RevisionGuard, WorkerNotifier};
use std::sync::Arc;

/// Host-owned application policy, independent of native renderer/topology incarnations.
#[derive(Default)]
pub struct ApplicationDriver {
    owner: Option<ApplicationRuntime>,
    project: Option<uuid::Uuid>,
    demand: Vec<ApplicationDemand>,
    admission: Arc<[RevisionGuard]>,
    logical: Option<LogicalApplicationCapture>,
}
/// Immutable actor capture. Applying it on the host owner may admit background workers.
pub struct PreparedApplicationService {
    logical: LogicalApplicationCapture,
    rendered: Vec<ApplicationDemand>,
    admission: Arc<[RevisionGuard]>,
    runtime: super::runtime::PreparedApplications,
    has_demand: bool,
}
impl PreparedApplicationService {
    pub fn project(&self) -> uuid::Uuid {
        self.logical.project
    }
}
impl ApplicationDriver {
    /// Check on an IO/host owner; unchanged projects require no actor capture.
    pub fn needs_service(&self) -> bool {
        self.logical.as_ref().is_none_or(|capture| !capture.is_current())
            || (!self.demand.is_empty() && !self.admission.iter().all(RevisionGuard::is_current))
    }
    /// Services committed project-wide demand without a renderer, device, window or video clock.
    pub fn service(&mut self, engine: &AppEngine, notifier: WorkerNotifier) -> Result<Option<PlanRetainer>, String> {
        let prepared = self.capture_service(engine)?;
        self.apply_prepared(prepared, notifier)
    }
    /// Pure metadata capture for the authoritative actor; no worker creation or process IO.
    pub fn capture_service(&self, engine: &AppEngine) -> Result<PreparedApplicationService, String> {
        self.service_capture()(engine)
    }
    /// Owned metadata request; a failed actor call cannot drop the live process controller.
    pub fn service_capture(
        &self,
    ) -> impl FnOnce(&AppEngine) -> Result<PreparedApplicationService, String> + Send + 'static {
        let project = self.project;
        let demand = self.demand.clone();
        let admission = self.admission.clone();
        move |engine| {
            let capture = capture_logical_applications(engine)?;
            let same = project == Some(capture.project) && admission.iter().all(RevisionGuard::is_current);
            let demand = if same { demand } else { Vec::new() };
            let admission = if same { admission } else { Arc::from([]) };
            Self::prepare(engine, capture, demand, admission)
        }
    }
    pub fn snapshot(&self) -> Option<super::protocol::ApplicationRuntimeSnapshot> {
        self.owner.as_ref().map(ApplicationRuntime::snapshot)
    }
    pub fn retirement(&self) -> Option<golden_io::RetirementPool> {
        self.owner.as_ref().map(ApplicationRuntime::retirement)
    }
    pub fn observations(&self) -> Option<super::runtime::ApplicationObservations> {
        self.owner.as_ref().map(ApplicationRuntime::observations)
    }
    pub fn callbacks(&self) -> Option<super::runtime::ApplicationCallbacks> {
        self.owner.as_ref().map(ApplicationRuntime::callbacks)
    }
    pub fn needs_sync(&self, project: uuid::Uuid, demand: &[ApplicationDemand]) -> bool {
        self.project != Some(project) || self.demand != demand
    }
    /// Run only after backend transaction settlement. Native operations remain on process workers.
    pub fn sync(
        &mut self,
        engine: &AppEngine,
        project: uuid::Uuid,
        demand: Vec<ApplicationDemand>,
        admission: Arc<[RevisionGuard]>,
        notifier: WorkerNotifier,
    ) -> Result<Option<PlanRetainer>, String> {
        let prepared = self.capture_sync(engine, project, demand, admission)?;
        self.apply_prepared(prepared, notifier)
    }
    /// Pure rendered/logical metadata capture. Apply its result outside the actor.
    pub fn capture_sync(
        &self,
        engine: &AppEngine,
        project: uuid::Uuid,
        demand: Vec<ApplicationDemand>,
        admission: Arc<[RevisionGuard]>,
    ) -> Result<PreparedApplicationService, String> {
        self.rendered_capture(project, demand, admission)(engine)
    }
    /// Owned rendered metadata request, retaining no process/controller owner on the actor.
    pub fn rendered_capture(
        &self,
        project: uuid::Uuid,
        demand: Vec<ApplicationDemand>,
        admission: Arc<[RevisionGuard]>,
    ) -> impl FnOnce(&AppEngine) -> Result<PreparedApplicationService, String> + Send + 'static {
        let logical = self.logical.clone();
        move |engine| {
            let manager = crate::media::authoring::manager(engine)?;
            if !matches!(engine.nodes.get(manager), Some(AppNode::MediaManager(node)) if node.instance == project) {
                return Err("application demand capture belongs to a replaced project".into());
            }
            if !admission.iter().all(RevisionGuard::is_current) {
                return Err("rendered application capture is stale".into());
            }
            let logical = match logical {
                Some(capture) if capture.project == project && capture.is_current() => capture,
                _ => capture_logical_applications(engine)?,
            };
            Self::prepare(engine, logical, demand, admission)
        }
    }
    fn prepare(
        engine: &AppEngine,
        logical: LogicalApplicationCapture,
        rendered: Vec<ApplicationDemand>,
        admission: Arc<[RevisionGuard]>,
    ) -> Result<PreparedApplicationService, String> {
        if !logical.is_current() || !admission.iter().all(RevisionGuard::is_current) {
            return Err("application demand changed before reconciliation".into());
        }
        let mut merged = std::collections::HashMap::<_, ApplicationDemand>::new();
        for entry in logical.demand.iter().chain(&rendered) {
            merged
                .entry(entry.application)
                .and_modify(|value| value.reasons.extend(&entry.reasons))
                .or_insert_with(|| entry.clone());
        }
        let mut demand = merged.into_values().collect::<Vec<_>>();
        demand.sort_by_key(|entry| entry.application.0.0);
        let guards = logical
            .guards
            .iter()
            .chain(admission.iter())
            .cloned()
            .collect::<Vec<_>>();
        let runtime = super::runtime::PreparedApplications::capture(engine, &demand, guards.into())?;
        Ok(PreparedApplicationService {
            logical,
            rendered,
            admission,
            runtime,
            has_demand: !demand.is_empty(),
        })
    }
    /// Host-side admission: validates capture fences again before creating any worker.
    pub fn apply_prepared(
        &mut self,
        prepared: PreparedApplicationService,
        notifier: WorkerNotifier,
    ) -> Result<Option<PlanRetainer>, String> {
        let PreparedApplicationService {
            logical,
            rendered,
            admission,
            runtime,
            has_demand,
        } = prepared;
        if !runtime.is_current() {
            return Err("application demand changed before reconciliation".into());
        }
        if self.project != Some(logical.project) {
            self.owner.take();
            self.project = Some(logical.project);
            self.demand.clear();
        }
        if self.owner.is_none() && has_demand {
            self.owner = Some(ApplicationRuntime::start_prepared(&runtime, Some(notifier))?);
        }
        if let Some(owner) = &mut self.owner {
            owner.reconcile_prepared(runtime)?;
        }
        self.demand = rendered;
        self.admission = admission;
        self.logical = Some(logical);
        Ok(self.owner.as_ref().map(ApplicationRuntime::plan_retainer))
    }
}
