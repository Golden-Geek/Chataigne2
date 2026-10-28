use golden_io::PendingSendError;
use golden_time::{ActivationState, TimeSnapshot};

use super::{Backend, Candidate, OutcomeKind, Runner};
use crate::runtime::core::{CoreError, InstallFailure};
use crate::runtime::preparation::{self, PreparationJob};
use crate::runtime::shared::PlanRequest;
use crate::runtime::types::{FailureCode, FrameObservation, LatePolicy, PlanActivation, RuntimeFailure};

fn activation(request: &PlanRequest, time: TimeSnapshot) -> Result<ActivationState, RuntimeFailure> {
    if !request.project_guard.is_current() {
        return Err(RuntimeFailure::new(FailureCode::StaleProject, "project replaced"));
    }
    if !request.desired_guard.is_current() {
        return Err(RuntimeFailure::new(
            FailureCode::Superseded,
            "newer desired plan published",
        ));
    }
    let observation = match request.activation {
        PlanActivation::NextFrame(epoch) if epoch == time.epoch() => ActivationState::Due {
            lateness: std::time::Duration::ZERO,
        },
        PlanActivation::NextFrame(_) => {
            return Err(RuntimeFailure::new(FailureCode::StaleTime, "activation epoch replaced"));
        }
        PlanActivation::Scheduled(stamp) => stamp
            .check(time)
            .map_err(|error| RuntimeFailure::new(FailureCode::StaleTime, error))?,
    };
    if let (ActivationState::Due { lateness }, LatePolicy::RejectAfter(tolerance)) = (observation, request.late_policy)
        && lateness > tolerance
    {
        return Err(RuntimeFailure::new(
            FailureCode::LateActivation,
            "prepared plan exceeded activation lateness tolerance",
        ));
    }
    Ok(observation)
}

impl<B: Backend> Runner<B> {
    fn reject(&self, request: PlanRequest, failure: RuntimeFailure) {
        let outcome = if failure.code() == FailureCode::Superseded {
            OutcomeKind::Superseded
        } else {
            OutcomeKind::Rejected(failure)
        };
        self.shared.complete(request.request, outcome);
    }

    pub(super) fn validate_prepared(&mut self, time: Option<TimeSnapshot>) {
        let time = time.unwrap_or_else(|| self.shared.snapshot().time);
        let rejection = self
            .prepared
            .as_ref()
            .and_then(|candidate| activation(&candidate.request, time).err());
        if let Some(failure) = rejection {
            let candidate = self.prepared.take().unwrap();
            drop(candidate.resources);
            self.reject(candidate.request, failure);
        }
    }

    pub(super) fn start_preparation(&mut self, time: Option<TimeSnapshot>) {
        if self.preparing.is_some() {
            return;
        }
        let desired = self.shared.lock().latest.take();
        let Some(desired) = desired else { return };
        let request = desired.request_info();
        if let Err(failure) = activation(&request, time.unwrap_or_else(|| self.shared.snapshot().time)) {
            self.reject(request, failure);
            return;
        }
        let device = self.core.snapshot().last_submitted.device;
        match self.preparation.send_weighted(PreparationJob { desired, device }, 1) {
            Ok(()) => self.preparing = Some(request),
            Err(error) => {
                let failure = match error {
                    PendingSendError::Full(_) => "preparation slot unexpectedly busy",
                    PendingSendError::Disconnected(_) => "preparation worker disconnected",
                };
                self.reject(request, RuntimeFailure::new(FailureCode::Backend, failure));
            }
        }
    }

    pub(super) fn preparation_result(&mut self, time: Option<TimeSnapshot>) {
        let (result, disconnected) = preparation::take_result(&self.preparation);
        let Some(result) = result else {
            if disconnected {
                if let Some(request) = self.preparing.take() {
                    self.reject(
                        request,
                        RuntimeFailure::new(FailureCode::Backend, "preparation worker stopped"),
                    );
                }
                self.fault(RuntimeFailure::new(
                    FailureCode::Backend,
                    "preparation worker disconnected",
                ));
                self.shared.close();
            }
            return;
        };
        let request = self.preparing.take().expect("only one preparation is admitted");
        assert_eq!(request.request, result.desired.request);
        let now = time.unwrap_or_else(|| self.shared.snapshot().time);
        if let Err(failure) = activation(&request, now) {
            self.reject(request, failure);
            return;
        }
        let device = self.core.snapshot().last_submitted.device;
        if result.device != device {
            // Retry the still-current desired metadata on the new device; no duplicate receipt.
            let mut state = self.shared.lock();
            if state.latest.is_none() {
                state.latest = Some(result.desired);
            } else {
                drop(state);
                self.shared.complete(request.request, OutcomeKind::Superseded);
            }
            return;
        }
        let plan = match result.compiled {
            Ok(plan) => plan,
            Err(failure) => {
                self.reject(request, failure);
                return;
            }
        };
        if let Some(old) = self.prepared.take() {
            drop(old.resources);
            self.shared.complete(old.request.request, OutcomeKind::Superseded);
        }
        match self.core.prepare(request.identity, plan) {
            Ok(resources) => self.prepared = Some(Candidate { resources, request }),
            Err(error) => {
                let code = if matches!(
                    error,
                    CoreError::ResourceBudgetExceeded { .. } | CoreError::PreparationBusy
                ) {
                    FailureCode::ResourcePressure
                } else {
                    FailureCode::Backend
                };
                self.reject(request, RuntimeFailure::new(code, error));
                self.core_fault(error, time);
            }
        }
    }

    pub(super) fn frame(&mut self, time: TimeSnapshot) {
        let project = self.shared.lock().project;
        let rendering = self
            .core
            .snapshot()
            .active
            .is_some_and(|active| active.output_count != 0 && active.identity.project == project);
        let ready = self
            .prepared
            .as_ref()
            .is_some_and(|candidate| matches!(activation(&candidate.request, time), Ok(ActivationState::Due { .. })));
        if !rendering && !ready {
            return;
        }
        if !rendering {
            self.cadence = super::Cadence::new(self.config.frame_rate, time.monotonic());
        }
        let tick = match self.cadence.consume_due(time.monotonic()) {
            Ok(Some(tick)) => tick,
            Ok(None) => return,
            Err(error) => {
                self.fault(RuntimeFailure::new(FailureCode::Time, error));
                return;
            }
        };
        {
            let mut state = self.shared.lock();
            state.snapshot.missed_frames = state.snapshot.missed_frames.saturating_add(tick.missed);
        }
        let can_install = self.prepared.as_ref().is_none_or(|candidate| {
            candidate.resources.info().output_count == 0
                || self.core.snapshot().in_flight < self.config.resource_limits.max_in_flight.get()
        });
        if ready && can_install {
            // This short metadata admission linearizes against close and publication fences.
            // Resource install/destruction stays outside the control lock.
            let admission = {
                let state = self.shared.lock();
                if state.closing {
                    return;
                }
                activation(&self.prepared.as_ref().unwrap().request, time)
            };
            let candidate = self.prepared.take().unwrap();
            if let Err(failure) = admission {
                drop(candidate.resources);
                self.reject(candidate.request, failure);
            } else {
                match self.core.install(candidate.resources) {
                    Ok(generation) => self.shared.complete(
                        candidate.request.request,
                        OutcomeKind::Installed { generation, at: time },
                    ),
                    Err(error) if error.reason() == InstallFailure::RetirementFull => {
                        self.prepared = Some(Candidate {
                            resources: error.into_prepared(),
                            request: candidate.request,
                        });
                    }
                    Err(error) => {
                        let failure = RuntimeFailure::new(
                            FailureCode::StaleDevice,
                            format_args!("install rejected: {:?}", error.reason()),
                        );
                        drop(error.into_prepared());
                        self.reject(candidate.request, failure);
                    }
                }
            }
        }
        let can_submit = {
            let state = self.shared.lock();
            !state.closing
                && self
                    .core
                    .snapshot()
                    .active
                    .is_some_and(|active| active.identity.project == state.project)
        };
        if !can_submit {
            return;
        }
        match self.core.submit(time) {
            Ok(Some(submission)) => {
                self.shared.record_frame(FrameObservation {
                    frame_index: tick.index,
                    scheduled_at: tick.scheduled,
                    completion: submission.token,
                    generation: submission.generation,
                    identity: submission.identity,
                    time,
                });
            }
            Ok(None) => {}
            Err(CoreError::InFlightFull) => {
                let mut state = self.shared.lock();
                state.snapshot.missed_frames = state.snapshot.missed_frames.saturating_add(1);
            }
            Err(error) => self.core_fault(error, Some(time)),
        }
    }
}
