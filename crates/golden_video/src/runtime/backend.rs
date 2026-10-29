use std::time::Duration;

use golden_time::TimeSnapshot;
use thiserror::Error;

use crate::plan::CompiledPlan;

use super::GenerationInfo;
use super::identity::{CompletionToken, DeviceGeneration};

/// Compact backend failures; native interfaces and driver-owned payloads never cross workers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum BackendError {
    /// Overlapping source subscriptions still consume admitted worker/storage capacity.
    #[error("native source capacity is retained by existing or retiring subscriptions")]
    #[cfg(all(windows, feature = "d3d11"))]
    SourceCapacity,
    /// The owning device lifetime has ended and its completion serials are no longer usable.
    #[error("render device was lost")]
    DeviceLost,
    /// Allocation failed; partially created resources have already been released.
    #[error("render resource allocation failed")]
    AllocationFailed,
    /// The selected backend lacks a required initialization capability or valid native contract.
    #[error("render backend initialization is unsupported")]
    #[cfg(all(windows, feature = "d3d11"))]
    UnsupportedInitialization,
    /// Submission was rejected before the proposed token acquired any GPU use.
    #[error("render submission failed before GPU use")]
    SubmissionFailed,
    /// The backend cannot represent its complete allocation charge.
    #[error("backend resource byte charge overflowed")]
    ResourceSizeOverflow,
    /// A qualification control was already attached to a different backend.
    #[error("null qualification control is already bound")]
    AlreadyBound,
    /// The compiled plan needs a source or consumer not supported by this backend binding.
    #[error("render plan has an unsupported or unbound native resource")]
    #[cfg(all(windows, feature = "d3d11"))]
    UnsupportedPlan,
}

/// Private worker-local backend boundary. Resources intentionally have no Send/Sync bound.
pub(crate) trait Backend {
    type Resources;

    /// Actual atomic installation, including rebuilds without a new publication request.
    fn observe_install(&mut self, _generation: GenerationInfo) {}

    /// Enables optional scheduler telemetry. Disabled backends need no per-frame metadata.
    fn observes_schedule(&self) -> bool {
        false
    }

    /// Metadata only; never changes admission, submits GPU work or emits a UI frame event.
    fn observe_schedule(
        &mut self,
        _device: DeviceGeneration,
        _schedule: super::FrameSchedule,
        _admission: super::FrameAdmission,
    ) {
    }

    /// Fixed charge for resources retained independently of prepared/live generations.
    /// Construction must defer these allocations until `initialize`, after admission.
    fn base_resource_bytes(&self) -> Result<u64, BackendError> {
        Ok(0)
    }

    /// Allocates the admitted baseline without submitting GPU work. Failure must dispose all
    /// partial native resources before returning. AllocationFailed can be retried on this same
    /// owner while its original baseline reservation remains held; other errors are terminal.
    fn initialize(&mut self) -> Result<(), BackendError> {
        Ok(())
    }

    /// Stops new background producer admissions without waiting. Backend-global IO pools
    /// retain their reservations and drain their own uses during final backend cleanup.
    fn quiesce(&mut self) {}

    /// Declared resource charge including every pool retained by this generation. Backends
    /// must distinguish descriptor storage from measured physical/driver memory claims.
    fn resource_bytes(&self, plan: &CompiledPlan) -> Result<u64, BackendError>;

    /// Allocates without submitting untracked GPU use. Failure retires partial allocations
    /// before returning; the caller keeps the complete reservation until then.
    fn prepare(&mut self, plan: &CompiledPlan, generation: GenerationInfo) -> Result<Self::Resources, BackendError>;

    /// Success establishes the proposed completion token. Non-device-loss errors must occur
    /// before any GPU use; a backend may not abandon resources after a partial submission.
    /// Unwinding must also establish completion (or confirmed loss) for the proposed token.
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
    /// Replaces baseline resources within the original fixed charge, which remains reserved
    /// throughout old-resource disposal, replacement and any failed reset attempt.
    fn reset(&mut self, device: DeviceGeneration) -> Result<(), BackendError>;
}
