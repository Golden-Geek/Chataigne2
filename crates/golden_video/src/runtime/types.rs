use std::fmt::{self, Write};
use std::num::NonZeroUsize;
use std::time::Duration;

use golden_io::WorkerNotifier;
use golden_time::{ActivationStamp, TimeSnapshot};

use crate::plan::PlanLimits;

use super::core::{GenerationInfo, ResourceLimits, ResourceSnapshot};
use super::identity::{CompletionToken, GenerationId, PlanIdentity, ProjectEpoch};

/// Explicit control, preparation and resource bounds for one video runtime.
#[derive(Clone, Debug)]
pub struct RuntimeConfig {
    /// Submitted graph limits, checked on the preparation worker.
    pub plan_limits: PlanLimits,
    /// Declared baseline/prepared/live/in-flight/retiring resource-charge ceiling.
    pub resource_limits: ResourceLimits,
    /// Independent frame cadence, not a source of show-time advancement.
    pub frame_rate: golden_time::RationalRate,
    /// All accepted requests, including undrained outcomes, share these credits.
    pub max_outstanding: NonZeroUsize,
    /// Maximum retained input heap charge for one submitted plan.
    pub max_input_bytes: NonZeroUsize,
    /// Aggregate retained raw plan inputs, disposed off the publisher/actor thread.
    pub max_retained_input_bytes: NonZeroUsize,
    /// Initial device-recreation retry delay after a failed attempt.
    pub recovery_delay: Duration,
    /// Capped device-recreation retry delay.
    pub max_recovery_delay: Duration,
    /// Optional payload-free notification for outcomes and lifecycle changes, never each frame.
    pub notifier: Option<WorkerNotifier>,
}

/// Identity of one accepted request and its exactly-once reliable outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RequestId(pub(crate) u64);

impl RequestId {
    /// Monotonic runtime-local request number.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Explicit plan activation intent within a captured time epoch.
#[derive(Clone, Copy, Debug)]
pub enum PlanActivation {
    /// Install at the next frame boundary in this exact epoch, without treating idle age as lateness.
    NextFrame(golden_time::TimeEpoch),
    /// Install no earlier than the declared monotonic deadline.
    Scheduled(ActivationStamp),
}

/// Policy for a prepared plan that reaches its activation deadline late.
#[derive(Clone, Copy, Debug)]
pub enum LatePolicy {
    /// Admit the plan at the next available frame and report its actual activation time.
    Activate,
    /// Reject lateness exceeding the supplied tolerance.
    RejectAfter(Duration),
}

/// Bounded diagnostic category, separate from backend/native handles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FailureCode {
    /// An app-owned source lifetime/revision changed before activation or reconstruction.
    StaleSource,
    /// Render-plan validation failed.
    InvalidPlan,
    /// Allocation or bounded-retirement admission failed.
    ResourcePressure,
    /// Preparation belongs to a replaced project.
    StaleProject,
    /// A newer desired plan superseded this preparation.
    Superseded,
    /// Preparation belongs to an old native device lifetime.
    StaleDevice,
    /// Activation belongs to an old time authority/discontinuity.
    StaleTime,
    /// Explicit late-work policy rejected activation.
    LateActivation,
    /// Backend preparation/submission/retirement failed.
    Backend,
    /// Time observation or conversion failed.
    Time,
    /// Runtime stop cancelled accepted work.
    Stopped,
}

/// Diagnostic text is capped at 1,024 UTF-8 bytes, bounding reliable outcome storage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeFailure {
    code: FailureCode,
    message: String,
}

impl RuntimeFailure {
    pub(crate) fn new(code: FailureCode, message: impl fmt::Display) -> Self {
        let mut writer = LimitedMessage(String::new());
        let _ = write!(writer, "{message}");
        Self {
            code,
            message: writer.0,
        }
    }

    /// Stable diagnostic category.
    pub const fn code(&self) -> FailureCode {
        self.code
    }
    /// Bounded human-readable detail.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for RuntimeFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}: {}", self.code, self.message)
    }
}

struct LimitedMessage(String);
impl Write for LimitedMessage {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let available = 1_024 - self.0.len();
        let mut end = available.min(value.len());
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        self.0.push_str(&value[..end]);
        if end == value.len() { Ok(()) } else { Err(fmt::Error) }
    }
}

/// Exactly one terminal result for each accepted request.
#[derive(Clone, Debug)]
pub struct RuntimeOutcome {
    /// Accepted request identity.
    pub request: RequestId,
    /// Terminal result, carrying metadata only.
    pub result: OutcomeKind,
}

/// Reliable request completion, independent of coalesced runtime status.
#[derive(Clone, Debug)]
pub enum OutcomeKind {
    /// Complete resource generation installed at a frame boundary.
    Installed {
        /// Installed generation metadata.
        generation: GenerationInfo,
        /// Authority observation at actual installation.
        at: TimeSnapshot,
    },
    /// A newer desired publication won before installation.
    Superseded,
    /// Rejection leaves the prior live generation intact.
    Rejected(RuntimeFailure),
    /// Shutdown cancelled preparation or scheduled activation.
    Cancelled,
    /// App-supplied transport intent produced a new time epoch.
    TimeChanged(TimeSnapshot),
    /// The requested replacement has retired its old project's resources and preparation.
    ProjectChanged(ProjectEpoch),
}

/// Aggregate runtime lifecycle. Source/output-specific observations arrive with native integration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeState {
    /// Worker/backend creation is pending.
    Starting,
    /// Worker is serving control and frame deadlines.
    Running,
    /// Control admission ended; resources are still retiring.
    Stopping,
    /// Resource and preparation workers finished cleanup.
    Stopped,
    /// A time/backend fault remains unresolved; control remains responsive.
    Faulted,
}

/// Metadata for the latest submitted frame; no pixels/native resources leave the worker.
#[derive(Clone, Copy, Debug)]
pub struct FrameObservation {
    /// Absolute cadence ordinal, without a catch-up frame loop.
    pub frame_index: u64,
    /// Ideal cadence deadline, distinct from actual submission and physical presentation.
    pub scheduled_at: golden_time::MonotonicTime,
    /// Submission device/fence lifetime.
    pub completion: CompletionToken,
    /// Resource generation used by this frame.
    pub generation: GenerationId,
    /// Authored plan publication rendered.
    pub identity: PlanIdentity,
    /// Shared authority supplied to every output in this submission.
    pub time: TimeSnapshot,
}

/// Coalesced read-only observation; reliable outcomes use their own bounded queue.
#[derive(Clone, Debug)]
pub struct RuntimeSnapshot {
    /// Aggregate lifecycle.
    pub state: RuntimeState,
    /// Current project publication lifetime.
    pub project: ProjectEpoch,
    /// Last accepted time observation/mapping.
    pub time: TimeSnapshot,
    /// Worker-owned accounting once the baseline is admitted, including pending initialization.
    pub resources: Option<ResourceSnapshot>,
    /// Whether one CPU preparation is in flight.
    pub preparing: bool,
    /// Whether one latest desired publication is pending.
    pub latest_desired: bool,
    /// Whether one GPU-prepared generation awaits activation/install.
    pub prepared: bool,
    /// Requests retaining outcome admission credits.
    pub outstanding: usize,
    /// Successfully submitted frames.
    pub frames_submitted: u64,
    /// Coalesced elapsed or admission-blocked frame deadlines.
    pub missed_frames: u64,
    /// Latest submitted frame's shared-time observation.
    pub last_frame: Option<FrameObservation>,
    /// Latest bounded error, retained as history after recovery and separate from request results.
    pub failure: Option<RuntimeFailure>,
}

#[cfg(test)]
mod tests;
