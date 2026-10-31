//! Immutable actor capture; controller admission and worker creation happen on the host owner.
use super::*;

#[derive(Clone)]
pub(super) struct Context {
    pub project: uuid::Uuid,
    pub guard: RevisionGuard,
    pub view: RuntimeView,
}
impl Context {
    pub fn capture(engine: &AppEngine) -> Result<Self, String> {
        let id = crate::media::authoring::manager(engine)?;
        let Some(AppNode::MediaManager(manager)) = engine.nodes.get(id) else {
            unreachable!()
        };
        Ok(Self {
            project: manager.instance,
            guard: manager.project_guard(),
            view: manager.applications.clone(),
        })
    }
}

pub(crate) struct PreparedApplications {
    pub(super) context: Context,
    pub(super) targets: HashMap<ApplicationId, Target>,
    admission: Arc<[RevisionGuard]>,
}
impl PreparedApplications {
    pub(crate) fn is_current(&self) -> bool {
        self.context.guard.is_current()
            && self.admission.iter().all(RevisionGuard::is_current)
            && self
                .targets
                .values()
                .all(|target| target.guard.as_ref().is_none_or(RevisionGuard::is_current))
    }
    pub(crate) fn capture(
        engine: &AppEngine,
        demand: &[ApplicationDemand],
        admission: Arc<[RevisionGuard]>,
    ) -> Result<Self, String> {
        if demand.len() > MAX_APPLICATIONS {
            return Err("application runtime admits at most 64 active/retiring definitions".into());
        }
        let context = Context::capture(engine)?;
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
                    let definition = if snapshot.enablement == ApplicationEnablement::Enabled
                        && snapshot.definition.executable.is_some()
                    {
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
                        admission: admission.clone(),
                        definition_revision: Some(snapshot.revision),
                        definition_instance: Some(snapshot.definition_instance),
                    }
                }
                _ => Target {
                    epoch: uuid::Uuid::new_v4(),
                    retain_existing: false,
                    definition: None,
                    guard: None,
                    admission: admission.clone(),
                    definition_revision: None,
                    definition_instance: None,
                },
            };
            target.retain_existing = entry.reasons.iter().any(|reason| {
                matches!(
                    reason,
                    super::super::demand::DemandReason::Outgoing | super::super::demand::DemandReason::FinalUse
                )
            });
            if target.definition.is_none()
                && entry
                    .reasons
                    .iter()
                    .all(|reason| *reason == super::super::demand::DemandReason::Neighbor)
            {
                continue;
            }
            targets.insert(entry.application, target);
        }
        let capture = Self {
            context,
            targets,
            admission,
        };
        if !capture.is_current() {
            return Err("application demand capture is stale".into());
        }
        Ok(capture)
    }
}
