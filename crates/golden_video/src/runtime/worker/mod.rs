use std::collections::VecDeque;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::time::{Duration, Instant};

use golden_io::{BoundedWorker, ReconnectBackoff, RetirementPool, WorkerContext};
use golden_time::{Clock, TimeAuthority, TimeSnapshot};

use super::backend::Backend;
use super::cadence::Cadence;
use super::core::{PreparedGeneration, RenderCore};
use super::identity::{DeviceGeneration, ProjectEpoch};
use super::null::{NullBackend, NullConfig, NullControl};
use super::preparation::{self, PreparationJob, PreparationResult};
use super::shared::{PlanRequest, Shared};
use super::types::{FailureCode, OutcomeKind, RequestId, RuntimeConfig, RuntimeFailure, RuntimeState};

mod commands;
mod lifecycle;
mod plans;

#[cfg(test)]
mod tests;

struct Candidate<R> {
    resources: PreparedGeneration<R>,
    request: PlanRequest,
}

struct ProjectRetirement {
    request: RequestId,
    previous: ProjectEpoch,
    current: ProjectEpoch,
}

struct Runner<B: Backend> {
    core: RenderCore<B>,
    context: WorkerContext<(), ()>,
    shared: Arc<Shared>,
    config: RuntimeConfig,
    preparation: BoundedWorker<PreparationJob, PreparationResult>,
    preparation_lifetimes: RetirementPool,
    preparing: Option<PlanRequest>,
    prepared: Option<Candidate<B::Resources>>,
    project_retirements: VecDeque<ProjectRetirement>,
    authority: TimeAuthority<Arc<dyn Clock>>,
    cadence: Cadence,
    recovery: ReconnectBackoff,
    retry_at: Option<Instant>,
    rebuild: Option<(super::identity::PlanIdentity, crate::plan::CompiledPlan)>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    context: WorkerContext<(), ()>,
    shared: Arc<Shared>,
    config: RuntimeConfig,
    null_config: NullConfig,
    null: NullControl,
    preparation_lifetimes: RetirementPool,
    device: DeviceGeneration,
    authority: TimeAuthority<Arc<dyn Clock>>,
) {
    run_backend(
        context,
        shared,
        config,
        preparation_lifetimes,
        device,
        authority,
        null_config.preparation_delay,
        move || {
            NullBackend::new(null_config, device, null)
                .map_err(|error| RuntimeFailure::new(FailureCode::Backend, error))
        },
    );
}

#[cfg(all(windows, feature = "d3d11"))]
#[allow(clippy::too_many_arguments)]
pub(super) fn run_d3d11(
    context: WorkerContext<(), ()>,
    shared: Arc<Shared>,
    config: RuntimeConfig,
    native_config: crate::native::D3d11Config,
    native_control: crate::native::D3d11Control,
    preparation_lifetimes: RetirementPool,
    device: DeviceGeneration,
    authority: TimeAuthority<Arc<dyn Clock>>,
) {
    let notify = context.wake_notifier();
    run_backend(
        context,
        shared,
        config,
        preparation_lifetimes,
        device,
        authority,
        Duration::ZERO,
        move || Ok(crate::native::D3d11Backend::new(native_config, native_control, device).with_notifier(notify)),
    );
}

#[allow(clippy::too_many_arguments)]
fn run_backend<B: Backend>(
    context: WorkerContext<(), ()>,
    shared: Arc<Shared>,
    config: RuntimeConfig,
    preparation_lifetimes: RetirementPool,
    device: DeviceGeneration,
    authority: TimeAuthority<Arc<dyn Clock>>,
    preparation_delay: Duration,
    create: impl FnOnce() -> Result<B, RuntimeFailure>,
) {
    let run_shared = shared.clone();
    let prepare_pool = preparation_lifetimes.clone();
    let result = catch_unwind(AssertUnwindSafe(move || {
        let backend = create()?;
        let core = RenderCore::admit(backend, device, config.resource_limits)
            .map_err(|error| RuntimeFailure::new(FailureCode::ResourcePressure, error))?;
        let preparation = preparation::start(
            &prepare_pool,
            context.wake_notifier(),
            config.plan_limits,
            preparation_delay,
        )
        .map_err(|error| RuntimeFailure::new(FailureCode::Backend, error))?;
        let time = run_shared.snapshot().time;
        let mut runner = Runner {
            core,
            context,
            shared: run_shared,
            config: config.clone(),
            preparation,
            preparation_lifetimes: prepare_pool,
            preparing: None,
            prepared: None,
            project_retirements: VecDeque::new(),
            authority,
            cadence: Cadence::new(config.frame_rate, time.monotonic()),
            recovery: ReconnectBackoff::new(config.recovery_delay, config.max_recovery_delay),
            retry_at: None,
            rebuild: None,
        };
        runner.run_loop();
        runner.shutdown();
        Ok::<_, RuntimeFailure>(())
    }));
    let failure = match result {
        Ok(Ok(())) => None,
        Ok(Err(error)) => Some(error),
        Err(_) => Some(RuntimeFailure::new(
            FailureCode::Backend,
            "render worker panicked after resource safety drain",
        )),
    };
    if let Some(failure) = failure {
        shared.close();
        while !preparation_lifetimes.wait_for_idle(Duration::from_millis(20)) {}
        let pending = {
            let mut state = shared.lock();
            state.latest.take();
            state.commands.clear();
            state
                .requests
                .iter()
                .filter_map(|(&id, &queued)| (!queued).then_some(id))
                .collect::<Vec<_>>()
        };
        for request in pending {
            shared.complete(request, OutcomeKind::Rejected(failure.clone()));
        }
        let mut state = shared.lock();
        state.snapshot.failure = Some(failure);
        state.snapshot.state = RuntimeState::Stopped;
        state.snapshot.preparing = false;
        state.snapshot.prepared = false;
        state.snapshot.resources = None;
        state.snapshot.last_frame = None;
        drop(state);
        shared.notify();
    }
}

impl<B: Backend> Runner<B> {
    fn run_loop(&mut self) {
        self.publish_snapshot();
        self.shared.notify();
        loop {
            // Capture before servicing domain state: a publication during service cannot be lost.
            let checkpoint = self.context.wake_checkpoint();
            if self.context.cancellation().is_cancelled() || self.shared.lock().closing {
                break;
            }
            self.commands();
            let time = self.observe_time();
            if self.initialize(time) {
                if let Err(error) = self.core.poll_completion() {
                    self.core_fault(error, time);
                }
                self.retry_rebuild(time);
                self.quiesce_old_project();
                self.validate_prepared(time);
                self.preparation_result(time);
                self.start_preparation(time);
                if let Some(time) = self.observe_time() {
                    self.frame(time);
                }
            }
            self.finish_projects();
            self.publish_snapshot();
            // Fatal startup can close before the caller installs Shared's wake notifier.
            // Never depend on a notification to observe a close made by this same service.
            if self.context.cancellation().is_cancelled() || self.shared.lock().closing {
                break;
            }
            if !self.shared.lock().commands.is_empty() {
                continue;
            }
            // Work/driver calls may take time; reobserve before converting an absolute
            // cadence deadline to an OS wait, rather than accumulating submission duration.
            let wait_time = self.observe_time();
            let deadline = self.deadline(wait_time);
            self.context.wait_until(checkpoint, deadline);
        }
    }

    fn fault(&self, failure: RuntimeFailure) {
        self.shared.fault(failure);
    }

    fn observe_time(&mut self) -> Option<TimeSnapshot> {
        match self.authority.sample() {
            Ok(time) => {
                self.shared.publish_time(time, None);
                Some(time)
            }
            Err(error) => {
                self.fault(RuntimeFailure::new(FailureCode::Time, error));
                None
            }
        }
    }

    fn publish_snapshot(&self) {
        let mut state = self.shared.lock();
        state.snapshot.resources = Some(self.core.snapshot());
        state.snapshot.preparing = self.preparing.is_some();
        state.snapshot.prepared = self.prepared.is_some();
    }

    fn deadline(&self, time: Option<TimeSnapshot>) -> Instant {
        let Some(time) = time else {
            let poll = Instant::now() + Duration::from_millis(50);
            return self.retry_at.map_or(poll, |retry| retry.min(poll));
        };
        let project = self.shared.lock().project;
        let rendering = self
            .core
            .snapshot()
            .active
            .is_some_and(|active| active.output_count != 0 && active.identity.project == project);
        let pending_uses = self.core.has_pending_uses();
        let period = self
            .config
            .frame_rate
            .duration_for(1, golden_time::Rounding::Ceil)
            .unwrap_or(Duration::from_millis(1));
        let mut delay = if rendering {
            self.cadence
                .deadline()
                .map(|at| at.duration_since(time.monotonic()).unwrap_or(Duration::ZERO))
                .unwrap_or(period)
        } else if pending_uses {
            period
        } else {
            Duration::from_secs(3600)
        };
        if let Some(candidate) = &self.prepared {
            let request = &candidate.request;
            if let super::types::PlanActivation::Scheduled(stamp) = request.activation {
                if let Ok(golden_time::ActivationState::Pending { remaining }) = stamp.check(time) {
                    delay = delay.min(remaining);
                } else {
                    delay = delay.min(period);
                }
            } else {
                delay = delay.min(period);
            }
        }
        if let Some(retry_at) = self.retry_at {
            delay = delay.min(retry_at.saturating_duration_since(Instant::now()));
        }
        Instant::now()
            .checked_add(delay.min(Duration::from_secs(3600)))
            .unwrap_or_else(Instant::now)
    }
}
