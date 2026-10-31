use super::*;
use golden_video::runtime::PlanLifetimeObserver;

/// Plan observers preserve existing process owners; they can never start/restart an executable.
#[derive(Clone)]
pub struct PlanRetainer(pub(super) Arc<Shared>);
impl PlanRetainer {
    pub fn project(&self) -> uuid::Uuid {
        self.0.project
    }
    pub fn retain(&self, applications: &[ApplicationId], lifetime: PlanLifetimeObserver) -> Result<(), String> {
        if applications.len() > MAX_APPLICATIONS || !self.0.guard.is_current() {
            return Err("application plan retention exceeds capacity or belongs to a retired project".into());
        }
        let mut held = self.0.final_use.lock().expect("application final-use lock poisoned");
        held.retain(|_, lifetimes| {
            lifetimes.retain(|lifetime| !lifetime.is_complete());
            !lifetimes.is_empty()
        });
        let ids = applications.iter().copied().collect::<HashSet<_>>();
        if held.len() + ids.iter().filter(|id| !held.contains_key(id)).count() > MAX_APPLICATIONS
            || ids
                .iter()
                .any(|id| held.get(id).is_some_and(|lifetimes| lifetimes.len() >= 128))
        {
            return Err("application final-use capacity is held by live/retiring plans".into());
        }
        let desired = self.0.desired.lock().expect("application demand lock poisoned");
        if ids.iter().any(|id| !desired.targets.contains_key(id)) {
            return Err("plan application was not admitted in the committed demand set".into());
        }
        for id in ids {
            held.entry(id).or_default().push(lifetime.clone());
        }
        Ok(())
    }
}
