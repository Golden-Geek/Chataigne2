use super::{
    demand::ApplicationDemand,
    runtime::{ApplicationRuntime, PlanRetainer},
};
use crate::app::{AppEngine, AppNode};
use golden_io::{RevisionGuard, WorkerNotifier};
use std::sync::Arc;

/// Host-owned application policy, independent of native renderer/topology incarnations.
#[derive(Default)]
pub(crate) struct Bridge {
    owner: Option<ApplicationRuntime>,
    project: Option<uuid::Uuid>,
    demand: Vec<ApplicationDemand>,
}
impl Bridge {
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
        if self.project != Some(project) {
            self.owner.take();
            self.project = Some(project);
            self.demand.clear();
        }
        if self.owner.is_none() && !demand.is_empty() {
            self.owner = Some(ApplicationRuntime::start(engine, Some(notifier))?);
        }
        if let Some(owner) = &mut self.owner {
            owner.reconcile_guarded(engine, &demand, admission)?;
        }
        self.demand = demand;
        Ok(self.owner.as_ref().map(ApplicationRuntime::plan_retainer))
    }
}
