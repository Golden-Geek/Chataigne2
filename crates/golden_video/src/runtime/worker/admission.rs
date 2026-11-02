//! A compiled replacement waits for reclaimable GPU uses without allocating or losing its receipt.
use golden_time::TimeSnapshot;

use super::{Backend, Candidate, OutcomeKind, Runner};
use crate::plan::CompiledPlan;
use crate::runtime::core::CoreError;
use crate::runtime::identity::DeviceGeneration;
use crate::runtime::shared::PlanRequest;
use crate::runtime::types::{FailureCode, RuntimeFailure};

pub(super) struct PendingAdmission {
    pub request: PlanRequest,
    plan: CompiledPlan,
    reserved: u64,
    device: DeviceGeneration,
}

impl<B: Backend> Runner<B> {
    pub(super) fn admit_compiled(&mut self, request: PlanRequest, plan: CompiledPlan, time: Option<TimeSnapshot>) {
        if let Some(previous) = self.admission.take() {
            self.shared.complete(previous.request.request, OutcomeKind::Superseded);
        }
        // Retain only bounded immutable metadata while the core owns all GPU reservations.
        match self.core.prepare(request.identity, plan.clone()) {
            Ok(resources) => self.prepared = Some(Candidate { resources, request }),
            Err(error) if self.core.admission_can_wait(error) => {
                let resources = self.core.snapshot();
                self.admission = Some(PendingAdmission {
                    request,
                    plan,
                    reserved: resources.bytes_used,
                    device: resources.last_submitted.device,
                });
            }
            Err(error) => {
                let pressure = match error {
                    CoreError::ResourceBudgetExceeded { .. } | CoreError::PreparationBusy => true,
                    #[cfg(all(windows, feature = "d3d11"))]
                    CoreError::Backend(crate::runtime::backend::BackendError::SourceCapacity) => true,
                    _ => false,
                };
                let stale = error == CoreError::StaleSource;
                let code = if stale {
                    FailureCode::StaleSource
                } else if pressure {
                    FailureCode::ResourcePressure
                } else {
                    FailureCode::Backend
                };
                self.reject(request, RuntimeFailure::new(code, error));
                if !pressure && !stale {
                    self.core_fault(error, time);
                }
            }
        }
    }

    pub(super) fn retry_admission(&mut self, time: Option<TimeSnapshot>) {
        let Some(pending) = self.admission.as_ref() else { return };
        let now = time.unwrap_or_else(|| self.shared.snapshot().time);
        if let Err(failure) = super::plans::activation(&pending.request, now) {
            let pending = self.admission.take().unwrap();
            self.reject(pending.request, failure);
            return;
        }
        let resources = self.core.snapshot();
        if resources.bytes_used == pending.reserved && resources.last_submitted.device == pending.device {
            return;
        }
        let pending = self.admission.take().unwrap();
        self.admit_compiled(pending.request, pending.plan, time);
    }
}
