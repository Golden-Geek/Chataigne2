use std::time::Duration;

use golden_time::TimeSnapshot;
use thiserror::Error;

use crate::plan::CompiledPlan;

use super::identity::{CompletionToken, DeviceGeneration};

/// Compact backend failures; native interfaces and driver-owned payloads never cross workers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum BackendError {
    /// The owning device lifetime has ended and its completion serials are no longer usable.
    #[error("render device was lost")]
    DeviceLost,
    /// Allocation failed; partially created resources have already been released.
    #[error("render resource allocation failed")]
    AllocationFailed,
    /// Submission was rejected before the proposed token acquired any GPU use.
    #[error("render submission failed before GPU use")]
    SubmissionFailed,
    /// The backend cannot represent its complete allocation charge.
    #[error("backend resource byte charge overflowed")]
    ResourceSizeOverflow,
    /// A qualification control was already attached to a different backend.
    #[error("null qualification control is already bound")]
    AlreadyBound,
}

/// Private worker-local backend boundary. Resources intentionally have no Send/Sync bound.
pub(super) trait Backend {
    type Resources;

    /// Full allocation charge, including any padding/pools retained by this generation.
    fn resource_bytes(&self, plan: &CompiledPlan) -> Result<u64, BackendError>;

    /// Allocates without submitting untracked GPU use. Failure retires partial allocations
    /// before returning; the caller keeps the complete reservation until then.
    fn prepare(&mut self, plan: &CompiledPlan, device: DeviceGeneration) -> Result<Self::Resources, BackendError>;

    /// Success establishes the proposed completion token. Non-device-loss errors must occur
    /// before any GPU use; a backend may not abandon resources after a partial submission.
    fn submit(
        &mut self,
        resources: &Self::Resources,
        plan: &CompiledPlan,
        token: CompletionToken,
        time: TimeSnapshot,
    ) -> Result<(), BackendError>;

    fn completion(&mut self) -> Result<CompletionToken, BackendError>;

    /// Worker-only wait, used during orderly cleanup while its window remains alive.
    fn wait_for_completion(&mut self, timeout: Duration) -> Result<(), BackendError>;

    /// Called only after confirmed loss and disposal of the old generation's resources.
    fn reset(&mut self, device: DeviceGeneration) -> Result<(), BackendError>;
}
