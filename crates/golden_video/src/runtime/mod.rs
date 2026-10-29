//! Bounded worker-owned rendering and a deterministic null qualification backend.
//!
//! Control mailboxes contain metadata/retained CPU inputs only. Actual backend resources remain
//! worker-local through matching device completion and retirement. Closing or observing a final
//! event is not a native-window lifetime proof; use the dedicated retirement handle off UI/actor.

pub(crate) mod backend;
mod cadence;
mod control;
mod core;
mod error;
mod identity;
mod null;
mod preparation;
mod schedule;
mod shared;
mod submission;
mod types;
mod worker;

pub use control::{RuntimeRetirement, VideoRuntime};
pub use core::{GenerationInfo, ResourceLimits, ResourceSnapshot};
pub use error::{AdmissionError, RuntimeStartError};
pub use identity::{CompletionToken, DeviceGeneration, GenerationId, PlanIdentity, PlanRevision, ProjectEpoch};
pub use null::{NullConfig, NullControl, NullSnapshot};
pub use schedule::{FrameAdmission, FrameSchedule};
pub use submission::PlanSubmission;
pub use types::{
    FailureCode, FrameObservation, LatePolicy, OutcomeKind, PlanActivation, RequestId, RuntimeConfig, RuntimeFailure,
    RuntimeOutcome, RuntimeSnapshot, RuntimeState,
};

#[cfg(test)]
mod tests;
