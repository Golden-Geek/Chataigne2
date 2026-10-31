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
impl ApplicationDriver {
    /// Check on an IO/host owner; unchanged projects require no actor capture.
    pub fn needs_service(&self) -> bool {
        self.logical.as_ref().is_none_or(|capture| !capture.is_current())
            || (!self.demand.is_empty() && !self.admission.iter().all(RevisionGuard::is_current))
    }
    /// Services committed project-wide demand without a renderer, device, window or video clock.
    pub fn service(&mut self, engine: &AppEngine, notifier: WorkerNotifier) -> Result<Option<PlanRetainer>, String> {
        let capture = capture_logical_applications(engine)?;
        let same = self.project == Some(capture.project) && self.admission.iter().all(RevisionGuard::is_current);
        let demand = if same { self.demand.clone() } else { Vec::new() };
        let admission = if same { self.admission.clone() } else { Arc::from([]) };
        self.sync_captured(engine, capture, demand, admission, notifier)
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
        let manager = crate::media::authoring::manager(engine)?;
        if !matches!(engine.nodes.get(manager), Some(AppNode::MediaManager(node)) if node.instance == project) {
            return Err("application demand capture belongs to a replaced project".into());
        }
        if !admission.iter().all(RevisionGuard::is_current) {
            return Err("rendered application capture is stale".into());
        }
        let logical = match &self.logical {
            Some(capture) if capture.project == project && capture.is_current() => capture.clone(),
            _ => capture_logical_applications(engine)?,
        };
        self.sync_captured(engine, logical, demand, admission, notifier)
    }
    fn sync_captured(
        &mut self,
        engine: &AppEngine,
        logical: LogicalApplicationCapture,
        rendered: Vec<ApplicationDemand>,
        admission: Arc<[RevisionGuard]>,
        notifier: WorkerNotifier,
    ) -> Result<Option<PlanRetainer>, String> {
        if !logical.is_current() || !admission.iter().all(RevisionGuard::is_current) {
            return Err("application demand changed before reconciliation".into());
        }
        if self.project != Some(logical.project) {
            self.owner.take();
            self.project = Some(logical.project);
            self.demand.clear();
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
        if self.owner.is_none() && !demand.is_empty() {
            self.owner = Some(ApplicationRuntime::start(engine, Some(notifier))?);
        }
        if let Some(owner) = &mut self.owner {
            let guards = logical
                .guards
                .iter()
                .chain(admission.iter())
                .cloned()
                .collect::<Vec<_>>();
            owner.reconcile_guarded(engine, &demand, guards.into())?;
        }
        self.demand = rendered;
        self.admission = admission;
        self.logical = Some(logical);
        Ok(self.owner.as_ref().map(ApplicationRuntime::plan_retainer))
    }
}
