use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard};

use golden_io::{RetainedPayload, RevisionGuard, WorkerNotifier};
use golden_time::{Clock, PlaybackRate, ShowPosition, TimeError, TimeSnapshot};

use super::PlanSubmission;

use super::identity::{PlanIdentity, ProjectEpoch};
use super::types::{
    FailureCode, FrameObservation, LatePolicy, OutcomeKind, PlanActivation, RequestId, RuntimeFailure, RuntimeOutcome,
    RuntimeSnapshot, RuntimeState,
};

pub(super) struct DesiredPlan {
    pub request: RequestId,
    pub identity: PlanIdentity,
    pub input: RetainedPayload<PlanSubmission>,
    pub project_guard: RevisionGuard,
    pub desired_guard: RevisionGuard,
    pub activation: PlanActivation,
    pub late_policy: LatePolicy,
}

#[cfg(test)]
mod tests;

pub(super) struct PlanRequest {
    pub guards: Arc<[golden_io::RevisionGuard]>,
    pub request: RequestId,
    pub identity: PlanIdentity,
    pub project_guard: RevisionGuard,
    pub desired_guard: RevisionGuard,
    pub activation: PlanActivation,
    pub late_policy: LatePolicy,
}

impl DesiredPlan {
    pub fn request_info(&self) -> PlanRequest {
        PlanRequest {
            guards: self.input.guards.clone(),
            request: self.request,
            identity: self.identity,
            project_guard: self.project_guard.clone(),
            desired_guard: self.desired_guard.clone(),
            activation: self.activation,
            late_policy: self.late_policy,
        }
    }
}

pub(super) struct ControlCommand {
    pub request: RequestId,
    pub kind: ControlKind,
}

pub(super) enum ControlKind {
    Project {
        previous: ProjectEpoch,
        current: ProjectEpoch,
    },
    Discontinuity {
        show: ShowPosition,
        rate: PlaybackRate,
    },
    Clock {
        clock: Arc<dyn Clock>,
        show: ShowPosition,
        rate: PlaybackRate,
    },
}

pub(super) struct Shared {
    state: Mutex<State>,
    wake: Mutex<Option<WorkerNotifier>>,
    notifier: Option<WorkerNotifier>,
    capacity: usize,
}

pub(super) struct State {
    pub project: ProjectEpoch,
    pub latest: Option<DesiredPlan>,
    pub commands: VecDeque<ControlCommand>,
    pub outcomes: VecDeque<RuntimeOutcome>,
    pub requests: BTreeMap<RequestId, bool>,
    pub accepting: bool,
    pub closing: bool,
    pub snapshot: RuntimeSnapshot,
    pub clock: Arc<dyn Clock>,
    time_fault: bool,
    backend_fault: bool,
}

impl State {
    fn resume(&mut self) -> bool {
        if !self.closing && !self.time_fault && !self.backend_fault && self.snapshot.state == RuntimeState::Faulted {
            self.snapshot.state = RuntimeState::Running;
            true
        } else {
            false
        }
    }
}

impl Shared {
    pub fn new(
        project: ProjectEpoch,
        time: TimeSnapshot,
        clock: Arc<dyn Clock>,
        capacity: usize,
        notifier: Option<WorkerNotifier>,
    ) -> Self {
        Self {
            state: Mutex::new(State {
                project,
                latest: None,
                commands: VecDeque::new(),
                outcomes: VecDeque::new(),
                requests: BTreeMap::new(),
                accepting: true,
                closing: false,
                snapshot: RuntimeSnapshot {
                    state: RuntimeState::Starting,
                    project,
                    time,
                    resources: None,
                    preparing: false,
                    latest_desired: false,
                    prepared: false,
                    outstanding: 0,
                    frames_submitted: 0,
                    missed_frames: 0,
                    last_frame: None,
                    failure: None,
                },
                clock,
                time_fault: false,
                backend_fault: false,
            }),
            wake: Mutex::new(None),
            notifier,
            capacity,
        }
    }

    pub fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn set_waker(&self, wake: WorkerNotifier) {
        *self.wake.lock().unwrap() = Some(wake);
    }

    pub fn wake(&self) {
        let wake = self.wake.lock().unwrap().clone();
        if let Some(wake) = wake {
            wake.notify();
        }
    }

    pub fn notify(&self) {
        if let Some(notifier) = &self.notifier {
            notifier.notify();
        }
    }

    pub fn has_capacity(&self, state: &State) -> bool {
        state.requests.len() < self.capacity
    }

    /// Every accepted ID retains one credit until its terminal outcome is drained.
    /// No reply publication can block the GPU loop or exceed the admitted credit count.
    pub fn complete(&self, request: RequestId, result: OutcomeKind) {
        let mut state = self.lock();
        // Fatal worker finalization can race a publisher superseding its detached old
        // slot. Claim the terminal state once; later contenders must not duplicate it.
        let Some(queued) = state.requests.get_mut(&request) else {
            return;
        };
        if *queued {
            return;
        }
        *queued = true;
        state.outcomes.push_back(RuntimeOutcome { request, result });
        debug_assert!(state.outcomes.len() <= self.capacity);
        drop(state);
        self.notify();
    }

    pub fn snapshot(&self) -> RuntimeSnapshot {
        let state = self.lock();
        let mut snapshot = state.snapshot.clone();
        snapshot.project = state.project;
        snapshot.latest_desired = state.latest.is_some();
        snapshot.outstanding = state.requests.len();
        snapshot
    }

    pub fn read_time(&self) -> Result<TimeSnapshot, TimeError> {
        let (snapshot, clock) = {
            let state = self.lock();
            (state.snapshot.time, state.clock.clone())
        };
        snapshot.at_monotonic(clock.now()?)
    }

    pub fn publish_time(&self, time: TimeSnapshot, clock: Option<Arc<dyn Clock>>) {
        let mut state = self.lock();
        // A mapping and its clock origin are one publication, including authority replacement.
        if let Some(clock) = clock {
            state.clock = clock;
        }
        state.snapshot.time = time;
        // Independent fault domains cannot certify one another's recovery. Keep the latest
        // error as bounded history; lifecycle follows the still-unresolved domains.
        state.time_fault = false;
        let recovered = state.resume();
        drop(state);
        if recovered {
            self.notify();
        }
    }

    pub fn fault(&self, failure: RuntimeFailure) {
        let mut state = self.lock();
        let prior = state.snapshot.state;
        let changed = state.snapshot.failure.as_ref() != Some(&failure);
        match failure.code() {
            FailureCode::Time => state.time_fault = true,
            _ => state.backend_fault = true,
        }
        state.snapshot.failure = Some(failure);
        if !state.closing {
            state.snapshot.state = RuntimeState::Faulted;
        }
        let changed = changed || prior != state.snapshot.state;
        drop(state);
        if changed {
            self.notify();
        }
    }

    pub fn backend_recovered(&self) {
        let mut state = self.lock();
        state.backend_fault = false;
        let recovered = state.resume();
        drop(state);
        if recovered {
            self.notify();
        }
    }

    pub fn record_frame(&self, frame: FrameObservation) {
        let mut state = self.lock();
        state.snapshot.frames_submitted = state.snapshot.frames_submitted.saturating_add(1);
        // A use admitted before the project fence may finish afterward. Preserve its lifetime
        // count, but never publish its frame as the new project's latest observation.
        if frame.identity.project == state.project {
            state.snapshot.last_frame = Some(frame);
        }
        let recovered = if frame.identity.project == state.project && !state.closing {
            state.backend_fault = false;
            state.resume()
        } else {
            false
        };
        drop(state);
        if recovered {
            self.notify();
        }
    }

    pub fn close(&self) {
        let changed = {
            let mut state = self.lock();
            let changed = !state.closing;
            state.accepting = false;
            state.closing = true;
            if state.snapshot.state != RuntimeState::Stopped {
                state.snapshot.state = RuntimeState::Stopping;
            }
            changed
        };
        self.wake();
        if changed {
            self.notify();
        }
    }
}
