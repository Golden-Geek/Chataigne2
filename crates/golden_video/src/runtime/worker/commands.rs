use golden_time::{TimeAuthority, TimeSnapshot};

use super::{Backend, OutcomeKind, ProjectRetirement, Runner};
use crate::runtime::shared::ControlKind;
use crate::runtime::types::{FailureCode, RuntimeFailure};

impl<B: Backend> Runner<B> {
    pub(super) fn commands(&mut self) {
        // A continuous command producer cannot starve frame/retirement service.
        for _ in 0..16 {
            let command = self.shared.lock().commands.pop_front();
            let Some(command) = command else { break };
            let mut next_clock = None;
            let result = match command.kind {
                ControlKind::Project { previous, current } => {
                    self.project_retirements.push_back(ProjectRetirement {
                        request: command.request,
                        previous,
                        current,
                    });
                    continue;
                }
                ControlKind::Discontinuity { show, rate } => self.authority.discontinuity(show, rate),
                ControlKind::Clock { clock, show, rate } => {
                    TimeAuthority::new(clock.clone(), show, rate).and_then(|mut authority| {
                        let time = authority.sample()?;
                        let previous = self.authority.sample().unwrap_or_else(|_| self.shared.snapshot().time);
                        self.close_schedule(previous);
                        self.authority = authority;
                        self.cadence = super::Cadence::new(self.config.frame_rate, time.monotonic());
                        next_clock = Some(clock);
                        Ok(time)
                    })
                }
            };
            match result {
                Ok(time) => self.time_changed(command.request, time, next_clock),
                Err(error) => self.shared.complete(
                    command.request,
                    OutcomeKind::Rejected(RuntimeFailure::new(FailureCode::Time, error)),
                ),
            }
        }
    }

    fn time_changed(
        &self,
        request: crate::runtime::types::RequestId,
        time: TimeSnapshot,
        clock: Option<std::sync::Arc<dyn golden_time::Clock>>,
    ) {
        self.shared.publish_time(time, clock);
        self.shared.complete(request, OutcomeKind::TimeChanged(time));
    }

    pub(super) fn quiesce_old_project(&mut self) {
        let project = self.shared.lock().project;
        if self
            .core
            .snapshot()
            .active
            .is_some_and(|active| active.identity.project != project)
            && let Err(error) = self.core.clear_live()
            && !matches!(error, crate::runtime::core::CoreError::RetirementFull)
        {
            self.core_fault(error, None);
        }
    }

    pub(super) fn finish_projects(&mut self) {
        let mut waiting = std::collections::VecDeque::new();
        while let Some(retirement) = self.project_retirements.pop_front() {
            let preparing = self
                .preparing
                .as_ref()
                .is_some_and(|request| request.identity.project == retirement.previous);
            let prepared = self
                .prepared
                .as_ref()
                .is_some_and(|candidate| candidate.resources.info().identity.project == retirement.previous);
            if !preparing && !prepared && !self.core.has_project(retirement.previous) {
                self.shared
                    .complete(retirement.request, OutcomeKind::ProjectChanged(retirement.current));
            } else {
                waiting.push_back(retirement);
            }
        }
        self.project_retirements = waiting;
    }
}
