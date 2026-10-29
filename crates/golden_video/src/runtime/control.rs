use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};

use golden_io::{BoundedWorker, PayloadPool, RetirementPool, RevisionSource, WorkerLimits};
use golden_time::{Clock, PlaybackRate, ShowPosition, SystemClock, TimeAuthority, TimeError, TimeSnapshot};
use uuid::Uuid;

use super::PlanSubmission;
use crate::plan::RenderPlan;

use super::error::{AdmissionError, RuntimeStartError};
use super::identity::{DeviceGeneration, PlanIdentity, PlanRevision, ProjectEpoch};
use super::null::{NullConfig, NullControl};
use super::shared::{ControlCommand, ControlKind, DesiredPlan, Shared};
use super::types::{
    LatePolicy, OutcomeKind, PlanActivation, RequestId, RuntimeConfig, RuntimeOutcome, RuntimeSnapshot,
};

/// Nonblocking video control owner. Backend/resources never enter its mailboxes or snapshots.
///
/// Plan construction/publication belongs to the preparation boundary, outside actor/UI callbacks.
/// Snapshot reads, bounded outcome drains and close/drop never join resource workers.
pub struct VideoRuntime {
    worker: BoundedWorker<(), ()>,
    shared: Arc<Shared>,
    payloads: PayloadPool,
    project_source: RevisionSource,
    desired_source: RevisionSource,
    runtime: Uuid,
    next_request: u64,
    config: RuntimeConfig,
    backend: BackendControl,
    retirement: RuntimeRetirement,
}

enum BackendControl {
    Null(NullControl),
    #[cfg(all(windows, feature = "d3d11"))]
    D3d11(crate::native::D3d11Control),
}

enum BackendStart {
    Null(NullConfig, NullControl),
    #[cfg(all(windows, feature = "d3d11"))]
    D3d11(crate::native::D3d11Config, crate::native::D3d11Control),
}

/// Explicit worker retirement observation for a host/shutdown worker, never an actor/UI callback.
///
/// A timeout is not retirement: keep native windows alive while this returns false. The pool
/// remains occupied through actual render/preparation cleanup, independently from event draining.
#[derive(Clone)]
pub struct RuntimeRetirement {
    lifetimes: RetirementPool,
    preparation: RetirementPool,
    payloads: PayloadPool,
}

impl RuntimeRetirement {
    /// Waits at most this total duration for both workers and retained input disposal.
    pub fn wait(&self, timeout: Duration) -> bool {
        let started = Instant::now();
        loop {
            let quantum = timeout.saturating_sub(started.elapsed()).min(Duration::from_millis(20));
            if self.lifetimes.wait_for_idle(quantum)
                && self
                    .preparation
                    .wait_for_idle(timeout.saturating_sub(started.elapsed()).min(Duration::from_millis(20)))
                && self
                    .payloads
                    .wait_for_idle(timeout.saturating_sub(started.elapsed()).min(Duration::from_millis(20)))
            {
                return true;
            }
            if started.elapsed() >= timeout {
                return false;
            }
        }
    }
}

impl VideoRuntime {
    /// Starts the production control/ownership path with a pixel-free qualification backend.
    pub fn spawn_null(config: RuntimeConfig, null: NullConfig) -> Result<Self, RuntimeStartError> {
        Self::spawn_null_with_clock(config, null, Arc::new(SystemClock::new()))
    }

    /// Starts with an injectable shared monotonic source, using the same authority/scheduler.
    pub fn spawn_null_with_clock(
        config: RuntimeConfig,
        null_config: NullConfig,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, RuntimeStartError> {
        Self::spawn(config, clock, BackendStart::Null(null_config, NullControl::new()))
    }

    /// Starts native execution on an explicitly selected hardware adapter. Native resources
    /// are created on the render worker after baseline admission, never on the caller/UI.
    #[cfg(all(windows, feature = "d3d11"))]
    pub fn spawn_d3d11(config: RuntimeConfig, native: crate::native::D3d11Config) -> Result<Self, RuntimeStartError> {
        native.validate(&config)?;
        Self::spawn(
            config,
            Arc::new(SystemClock::new()),
            BackendStart::D3d11(native, Default::default()),
        )
    }

    #[cfg(all(test, windows, feature = "d3d11"))]
    pub(crate) fn spawn_d3d11_with_control(
        config: RuntimeConfig,
        native: crate::native::D3d11Config,
        control: crate::native::D3d11Control,
    ) -> Result<Self, RuntimeStartError> {
        native.validate(&config)?;
        Self::spawn(
            config,
            Arc::new(SystemClock::new()),
            BackendStart::D3d11(native, control),
        )
    }

    fn spawn(config: RuntimeConfig, clock: Arc<dyn Clock>, backend: BackendStart) -> Result<Self, RuntimeStartError> {
        if config.max_input_bytes > config.max_retained_input_bytes {
            return Err(RuntimeStartError::InvalidConfig(
                "per-plan input limit exceeds aggregate limit",
            ));
        }
        if config.recovery_delay.is_zero() || config.recovery_delay > config.max_recovery_delay {
            return Err(RuntimeStartError::InvalidConfig("invalid device-recovery backoff"));
        }
        if Instant::now().checked_add(config.max_recovery_delay).is_none() {
            return Err(RuntimeStartError::InvalidConfig(
                "device-recovery backoff is not representable",
            ));
        }
        if config.resource_limits.max_in_flight.get() > 2 || config.resource_limits.max_retiring > 2 {
            return Err(RuntimeStartError::InvalidConfig(
                "in-flight and retiring generation limits cannot exceed two",
            ));
        }
        let mut authority = TimeAuthority::new(clock.clone(), ShowPosition::from_nanos(0), PlaybackRate::NORMAL)?;
        let time = authority.sample()?;
        let runtime = Uuid::new_v4();
        let project = ProjectEpoch::new(runtime, 0);
        let device = DeviceGeneration::new(runtime, 1);
        let project_source = RevisionSource::default();
        let desired_source = RevisionSource::default();
        let shared = Arc::new(Shared::new(
            project,
            time,
            clock,
            config.max_outstanding.get(),
            config.notifier.clone(),
        ));
        let lifetimes = RetirementPool::new(NonZeroUsize::MIN);
        let preparation_lifetimes = RetirementPool::new(NonZeroUsize::MIN);
        let payloads = PayloadPool::new(
            "golden-video-input-disposal",
            config.max_outstanding.get(),
            config.max_retained_input_bytes.get(),
        )?;
        let retirement = RuntimeRetirement {
            lifetimes: lifetimes.clone(),
            preparation: preparation_lifetimes.clone(),
            payloads: payloads.clone(),
        };
        let run_shared = shared.clone();
        let run_config = config.clone();
        let control = match &backend {
            BackendStart::Null(_, control) => BackendControl::Null(control.clone()),
            #[cfg(all(windows, feature = "d3d11"))]
            BackendStart::D3d11(_, control) => BackendControl::D3d11(control.clone()),
        };
        let worker = BoundedWorker::<(), ()>::spawn(
            "golden-video-render",
            &lifetimes,
            WorkerLimits {
                command_items: 1,
                command_weight: 1,
                event_items: 1,
                event_weight: 1,
            },
            move |context| match backend {
                BackendStart::Null(null_config, run_null) => super::worker::run(
                    context,
                    run_shared,
                    run_config,
                    null_config,
                    run_null,
                    preparation_lifetimes,
                    device,
                    authority,
                ),
                #[cfg(all(windows, feature = "d3d11"))]
                BackendStart::D3d11(native_config, native_control) => super::worker::run_d3d11(
                    context,
                    run_shared,
                    run_config,
                    native_config,
                    native_control,
                    preparation_lifetimes,
                    device,
                    authority,
                ),
            },
        )?;
        let wake = worker.wake_notifier();
        shared.set_waker(wake.clone());
        match &control {
            BackendControl::Null(null) => null.set_waker(wake),
            #[cfg(all(windows, feature = "d3d11"))]
            BackendControl::D3d11(_) => {}
        }
        Ok(Self {
            worker,
            shared,
            payloads,
            project_source,
            desired_source,
            runtime,
            next_request: 0,
            config,
            backend: control,
            retirement,
        })
    }

    /// Current project publication token; capture it before beginning external preparation.
    pub fn project_epoch(&self) -> ProjectEpoch {
        self.shared.lock().project
    }

    /// Copies coalesced metadata, without waiting for a frame, fence or outcome capacity.
    pub fn snapshot(&self) -> RuntimeSnapshot {
        self.shared.snapshot()
    }

    /// Projects the current immutable mapping at the source's current coordinate.
    /// Reading does not advance authority state; the worker revalidates epochs at admission.
    pub fn read_time(&self) -> Result<TimeSnapshot, TimeError> {
        self.shared.read_time()
    }

    /// Publishes one latest desired plan. Superseded inputs dispose on their payload worker.
    ///
    /// This method measures retained metadata and is a preparation/publication operation:
    /// call outside engine ticks/UI callbacks. Admission consumes the supplied CPU plan even
    /// on rejection; no native resource ownership can be present in this type.
    pub fn submit_plan(
        &mut self,
        project: ProjectEpoch,
        plan: RenderPlan,
        activation: PlanActivation,
        late_policy: LatePolicy,
    ) -> Result<RequestId, AdmissionError> {
        self.submit(project, PlanSubmission::new(plan), activation, late_policy)
    }

    /// Publishes guarded CPU input, including source-only demand, from a preparation worker.
    pub fn submit(
        &mut self,
        project: ProjectEpoch,
        mut plan: PlanSubmission,
        activation: PlanActivation,
        late_policy: LatePolicy,
    ) -> Result<RequestId, AdmissionError> {
        if plan.guards.len() > self.config.plan_limits.max_dependencies
            || plan.plan.external_textures.len() > self.config.plan_limits.max_external_textures
        {
            return Err(AdmissionError::InputTooLarge);
        }
        plan.attach_image_guards(self.config.plan_limits.max_dependencies)
            .ok_or(AdmissionError::InputTooLarge)?;
        let weight = plan
            .retained_bytes()
            .and_then(|bytes| bytes.checked_add(2 * size_of::<usize>()))
            .filter(|&bytes| bytes <= self.config.max_input_bytes.get())
            .ok_or(AdmissionError::InputTooLarge)?;
        let input = self
            .payloads
            .try_retain(plan, weight)
            .map_err(|_| AdmissionError::InputCapacity)?;
        let mut state = self.shared.lock();
        if !state.accepting {
            return Err(AdmissionError::Closed);
        }
        if project != state.project {
            return Err(AdmissionError::StaleProject);
        }
        if !self.shared.has_capacity(&state) {
            return Err(AdmissionError::OutstandingCapacity);
        }
        if self.desired_source.reader().current() == u64::MAX {
            return Err(AdmissionError::IdentityExhausted);
        }
        let request = RequestId(
            self.next_request
                .checked_add(1)
                .ok_or(AdmissionError::IdentityExhausted)?,
        );
        let revision = self.desired_source.advance();
        self.next_request = request.0;
        state.requests.insert(request, false);
        let old = state.latest.replace(DesiredPlan {
            request,
            identity: PlanIdentity {
                project,
                revision: PlanRevision::new(revision),
            },
            input,
            project_guard: self.project_source.reader().guard_at(project.generation()),
            desired_guard: self.desired_source.reader().guard_at(revision),
            activation,
            late_policy,
        });
        drop(state);
        if let Some(old) = old {
            self.shared.complete(old.request, OutcomeKind::Superseded);
        }
        self.shared.wake();
        Ok(request)
    }

    /// Fences new old-project admissions; already-admitted uses may finish afterward.
    /// The outcome arrives after old GPU/resources and preparation retire.
    pub fn replace_project(&mut self) -> Result<(RequestId, ProjectEpoch), AdmissionError> {
        let mut state = self.shared.lock();
        if !state.accepting {
            return Err(AdmissionError::Closed);
        }
        if !self.shared.has_capacity(&state) {
            return Err(AdmissionError::OutstandingCapacity);
        }
        if self.project_source.reader().current() == u64::MAX {
            return Err(AdmissionError::IdentityExhausted);
        }
        let request = RequestId(
            self.next_request
                .checked_add(1)
                .ok_or(AdmissionError::IdentityExhausted)?,
        );
        let current = ProjectEpoch::new(self.runtime, self.project_source.advance());
        let previous = state.project;
        self.next_request = request.0;
        state.project = current;
        state.snapshot.last_frame = None;
        state.requests.insert(request, false);
        state.commands.push_back(ControlCommand {
            request,
            kind: ControlKind::Project { previous, current },
        });
        let old = state.latest.take();
        drop(state);
        if let Some(old) = old {
            self.shared.complete(
                old.request,
                OutcomeKind::Rejected(super::types::RuntimeFailure::new(
                    super::types::FailureCode::StaleProject,
                    "project replaced before preparation",
                )),
            );
        }
        self.shared.wake();
        Ok((request, current))
    }

    /// Applies app-owned seek/play/pause/rate intent as a checked discontinuity.
    pub fn discontinuity(&mut self, show: ShowPosition, rate: PlaybackRate) -> Result<RequestId, AdmissionError> {
        self.command(ControlKind::Discontinuity { show, rate })
    }

    /// Replaces the clock authority lifetime, fencing old generation-zero activations as well.
    pub fn replace_clock(
        &mut self,
        clock: Arc<dyn Clock>,
        show: ShowPosition,
        rate: PlaybackRate,
    ) -> Result<RequestId, AdmissionError> {
        self.command(ControlKind::Clock { clock, show, rate })
    }

    fn command(&mut self, kind: ControlKind) -> Result<RequestId, AdmissionError> {
        let mut state = self.shared.lock();
        if !state.accepting {
            return Err(AdmissionError::Closed);
        }
        if !self.shared.has_capacity(&state) {
            return Err(AdmissionError::OutstandingCapacity);
        }
        let request = RequestId(
            self.next_request
                .checked_add(1)
                .ok_or(AdmissionError::IdentityExhausted)?,
        );
        self.next_request = request.0;
        state.requests.insert(request, false);
        state.commands.push_back(ControlCommand { request, kind });
        drop(state);
        self.shared.wake();
        Ok(request)
    }

    /// Drains at most the supplied count; only drained outcomes release admission credits.
    pub fn drain_outcomes(&self, output: &mut Vec<RuntimeOutcome>, budget: NonZeroUsize) -> usize {
        let mut state = self.shared.lock();
        let mut drained = 0;
        while drained < budget.get() {
            let Some(outcome) = state.outcomes.pop_front() else {
                break;
            };
            assert_eq!(state.requests.remove(&outcome.request), Some(true));
            output.push(outcome);
            drained += 1;
        }
        drained
    }

    /// Stops admission and cancels pending activation without waiting for resources or reply drains.
    pub fn close(&self) {
        self.shared.close();
    }
    /// Dedicated retirement proof for the outer presentation/shutdown worker.
    pub fn retirement(&self) -> RuntimeRetirement {
        self.retirement.clone()
    }
    /// Qualification-only manual completion/failure control, carrying metadata only.
    pub fn null_control(&self) -> Option<NullControl> {
        match &self.backend {
            BackendControl::Null(control) => Some(control.clone()),
            #[cfg(all(windows, feature = "d3d11"))]
            BackendControl::D3d11(_) => None,
        }
    }

    /// Native metadata reader, present only for a D3D11 runtime.
    #[cfg(all(windows, feature = "d3d11"))]
    pub fn d3d11_control(&self) -> Option<crate::native::D3d11Control> {
        match &self.backend {
            BackendControl::D3d11(control) => Some(control.clone()),
            BackendControl::Null(_) => None,
        }
    }
    /// Wakes an injected-clock service after the caller advances its fake source.
    pub fn wake(&self) {
        self.shared.wake();
    }
}

impl Drop for VideoRuntime {
    fn drop(&mut self) {
        self.shared.close();
        self.worker.cancel();
    }
}
