use serde::{Deserialize, Serialize};

use super::{FailureCode, GenerationInfo, PlanIdentity};

/// Metadata-only plan lifecycle, separate from frame execution and physical presentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanStage {
    /// Accepted into the latest desired slot, observed under the publication lock.
    Published,
    /// First render-worker receipt, before CPU compilation is admitted.
    Received,
    /// Actual atomic resource installation, including recreation of the same authored plan.
    Installed {
        /// Exact installed resource/device generation.
        generation: GenerationInfo,
    },
    /// A newer publication won before this plan was installed.
    Superseded,
    /// Accepted input was rejected; admission failures never publish a plan record.
    Rejected {
        /// Typed reason, without an allocated diagnostic message.
        code: FailureCode,
    },
    /// Shutdown cancelled accepted input before installation.
    Cancelled,
}

/// Optional internal observer. Implementations must record bounded metadata without waiting,
/// allocating, changing admission or reentering runtime control. Publication/receipt callbacks
/// run under the existing short metadata lock so their order follows the mailbox handoff.
pub(crate) trait PlanObserver: Send + Sync {
    fn observe(&self, identity: PlanIdentity, stage: PlanStage);
}
