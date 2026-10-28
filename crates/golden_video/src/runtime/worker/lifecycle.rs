use std::time::{Duration, Instant};

use golden_time::TimeSnapshot;

use super::{Backend, Candidate, OutcomeKind, Runner};
use crate::runtime::backend::BackendError;
use crate::runtime::core::CoreError;
use crate::runtime::types::{FailureCode, RuntimeFailure, RuntimeState};

impl<B: Backend> Runner<B> {
    pub(super) fn core_fault(&mut self, error: CoreError, time: Option<TimeSnapshot>) {
        if matches!(
            error,
            CoreError::ResourceBudgetExceeded { .. }
                | CoreError::PreparationBusy
                | CoreError::RetirementFull
                | CoreError::InFlightFull
        ) {
            return;
        }
        self.fault(RuntimeFailure::new(FailureCode::Backend, error));
        if error != CoreError::Backend(BackendError::DeviceLost) {
            return;
        }
        if self.retry_at.is_some_and(|retry| Instant::now() < retry) {
            return;
        }
        let pending = self.prepared.take().map(|candidate| {
            let plan = candidate.resources.plan().clone();
            drop(candidate.resources);
            (candidate.request, plan)
        });
        let Some(device) = self.core.snapshot().last_submitted.device.next() else {
            if let Some((request, _)) = pending {
                self.shared.complete(
                    request.request,
                    OutcomeKind::Rejected(RuntimeFailure::new(FailureCode::Backend, "device identity exhausted")),
                );
            }
            self.shared.close();
            return;
        };
        match self.core.reset_after_loss(device) {
            Ok(rebuild) => {
                self.retry_at = None;
                self.recovery.reset();
                if let Some(rebuild) = rebuild {
                    self.rebuild = Some(rebuild);
                }
                self.retry_rebuild(time);
                if let Some((request, plan)) = pending {
                    if self.rebuild.is_some() {
                        self.shared.complete(
                            request.request,
                            OutcomeKind::Rejected(RuntimeFailure::new(
                                FailureCode::Backend,
                                "device resources are still recovering",
                            )),
                        );
                    } else if request.project_guard.is_current() && request.desired_guard.is_current() {
                        match self.core.prepare(request.identity, plan) {
                            Ok(resources) => self.prepared = Some(Candidate { resources, request }),
                            Err(error) => self.shared.complete(
                                request.request,
                                OutcomeKind::Rejected(RuntimeFailure::new(FailureCode::Backend, error)),
                            ),
                        }
                    } else {
                        self.shared.complete(request.request, OutcomeKind::Superseded);
                    }
                }
                let time = time.unwrap_or_else(|| self.shared.snapshot().time);
                self.cadence = super::Cadence::new(self.config.frame_rate, time.monotonic());
                if self.rebuild.is_none() {
                    self.shared.lock().snapshot.state = RuntimeState::Running;
                }
                self.shared.notify();
            }
            Err(error) => {
                // Request metadata is never silently lost during an unsuccessful recreation.
                if let Some((request, _)) = pending {
                    self.shared.complete(
                        request.request,
                        OutcomeKind::Rejected(RuntimeFailure::new(FailureCode::Backend, error)),
                    );
                }
                self.retry_at = Some(self.recovery.schedule(Instant::now()));
            }
        }
    }

    pub(super) fn retry_rebuild(&mut self, time: Option<TimeSnapshot>) {
        let Some((identity, _)) = self.rebuild.as_ref() else {
            return;
        };
        let project = self.shared.lock().project;
        if identity.project != project
            || self.core.snapshot().active.is_some_and(|active| {
                active.identity.project == project && active.identity.revision >= identity.revision
            })
        {
            self.rebuild = None;
            self.retry_at = None;
            return;
        }
        if self.core.snapshot().device_lost || self.retry_at.is_some_and(|retry| Instant::now() < retry) {
            return;
        }
        if self.core.snapshot().prepared_generations != 0 {
            self.retry_at = Some(self.recovery.schedule(Instant::now()));
            return;
        }
        let (identity, plan) = self.rebuild.as_ref().unwrap();
        match self.core.prepare(*identity, plan.clone()) {
            Ok(resources) => match self.core.install(resources) {
                Ok(_) => {
                    self.rebuild = None;
                    self.retry_at = None;
                    self.recovery.reset();
                    let time = time.unwrap_or_else(|| self.shared.snapshot().time);
                    self.cadence = super::Cadence::new(self.config.frame_rate, time.monotonic());
                    self.shared.lock().snapshot.state = RuntimeState::Running;
                    self.shared.notify();
                }
                Err(error) => {
                    drop(error.into_prepared());
                    self.retry_at = Some(self.recovery.schedule(Instant::now()));
                }
            },
            Err(error) => {
                self.fault(RuntimeFailure::new(FailureCode::Backend, error));
                self.retry_at = Some(self.recovery.schedule(Instant::now()));
            }
        }
    }

    pub(super) fn shutdown(mut self) {
        self.shared.close();
        self.shared.lock().snapshot.state = RuntimeState::Stopping;
        self.shared.notify();
        self.preparation.cancel();
        if let Some(request) = self.preparing.take() {
            self.shared.complete(request.request, OutcomeKind::Cancelled);
        }
        if let Some(candidate) = self.prepared.take() {
            drop(candidate.resources);
            self.shared.complete(candidate.request.request, OutcomeKind::Cancelled);
        }
        let (latest, commands) = {
            let mut state = self.shared.lock();
            (state.latest.take(), state.commands.drain(..).collect::<Vec<_>>())
        };
        if let Some(latest) = latest {
            self.shared.complete(latest.request, OutcomeKind::Cancelled);
        }
        for command in commands {
            self.shared.complete(command.request, OutcomeKind::Cancelled);
        }

        // Closing never waits on reply capacity. Resource waits stay on this worker, and a
        // timeout does not drop resources or falsely authorize destruction of its native window.
        loop {
            match self.core.poll_completion() {
                Ok(()) if !self.core.has_pending_uses() => break,
                Err(CoreError::Backend(BackendError::DeviceLost)) => {
                    let _ = self.core.lose_device();
                    break;
                }
                Err(error) => self.fault(RuntimeFailure::new(FailureCode::Backend, error)),
                Ok(()) => {}
            }
            let _ = self.core.wait_for_completion(Duration::from_millis(20));
        }
        let _ = self.core.clear_live();
        let resources = self.core.snapshot();
        // Dispose renderer/backend objects before any final status/retirement observation.
        drop(self.core);
        drop(self.preparation);
        while !self.preparation_lifetimes.wait_for_idle(Duration::from_millis(20)) {}
        while let Some(retirement) = self.project_retirements.pop_front() {
            self.shared
                .complete(retirement.request, OutcomeKind::ProjectChanged(retirement.current));
        }
        let mut state = self.shared.lock();
        state.snapshot.resources = Some(resources);
        state.snapshot.state = RuntimeState::Stopped;
        state.snapshot.preparing = false;
        state.snapshot.prepared = false;
        drop(state);
        self.shared.notify();
    }
}
