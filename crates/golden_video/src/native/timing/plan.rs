use super::{DiagnosticEvent, PlanTrace, Recorder};
use crate::runtime::{PlanIdentity, PlanObserver, PlanStage};

impl PlanObserver for Recorder {
    fn observe(&self, identity: PlanIdentity, stage: PlanStage) {
        self.push(DiagnosticEvent::Plan(PlanTrace {
            identity,
            stage,
            observed_qpc: self.stamp(),
        }));
    }
}
