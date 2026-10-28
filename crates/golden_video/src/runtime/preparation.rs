use std::num::NonZeroUsize;
use std::time::Duration;

use golden_io::{BoundedWorker, PendingDrainState, RetirementPool, WorkerLimits, WorkerNotifier, WorkerStartError};

use crate::plan::{CompiledPlan, PlanLimits};

use super::identity::DeviceGeneration;
use super::shared::DesiredPlan;
use super::types::{FailureCode, RuntimeFailure};

pub(super) struct PreparationJob {
    pub desired: DesiredPlan,
    pub device: DeviceGeneration,
}

pub(super) struct PreparationResult {
    pub desired: DesiredPlan,
    pub device: DeviceGeneration,
    pub compiled: Result<CompiledPlan, RuntimeFailure>,
}

pub(super) fn start(
    lifetimes: &RetirementPool,
    notifier: WorkerNotifier,
    limits: PlanLimits,
    delay: Duration,
) -> Result<BoundedWorker<PreparationJob, PreparationResult>, WorkerStartError> {
    BoundedWorker::<PreparationJob, PreparationResult>::spawn_notified(
        "golden-video-prepare",
        lifetimes,
        WorkerLimits {
            command_items: 1,
            command_weight: 1,
            event_items: 1,
            event_weight: 1,
        },
        notifier,
        move |context| {
            while let Some(job) = context.receive() {
                if context.cancellation().wait(delay) {
                    break;
                }
                let compiled = if !job.desired.project_guard.is_current() {
                    Err(RuntimeFailure::new(
                        FailureCode::StaleProject,
                        "project replaced during preparation",
                    ))
                } else if !job.desired.desired_guard.is_current() {
                    Err(RuntimeFailure::new(
                        FailureCode::Superseded,
                        "newer desired plan published",
                    ))
                } else {
                    (*job.desired.input)
                        .clone()
                        .compile(&limits)
                        .map_err(|error| RuntimeFailure::new(FailureCode::InvalidPlan, error))
                };
                let result = PreparationResult {
                    desired: job.desired,
                    device: job.device,
                    compiled,
                };
                if context.publish_weighted(result, 1).is_err() {
                    break;
                }
            }
        },
    )
}

pub(super) fn take_result(
    worker: &BoundedWorker<PreparationJob, PreparationResult>,
) -> (Option<PreparationResult>, bool) {
    let mut results = Vec::with_capacity(1);
    let drain = worker.drain_events(&mut results, NonZeroUsize::MIN);
    (results.pop(), drain.state == PendingDrainState::Disconnected)
}
