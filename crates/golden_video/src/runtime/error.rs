use thiserror::Error;

/// Admission fails synchronously before accepting a request/outcome obligation.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AdmissionError {
    /// Runtime no longer accepts work.
    #[error("video runtime admission is closed")]
    Closed,
    /// Accepted work or undrained results retain every reliable outcome credit.
    #[error("video runtime outstanding-request capacity exhausted")]
    OutstandingCapacity,
    /// Input metadata exceeds its explicit bound or cannot be represented.
    #[error("render plan input metadata exceeds its admission bound")]
    InputTooLarge,
    /// Accepted raw payloads still own the aggregate CPU preparation budget.
    #[error("video runtime retained-input capacity exhausted")]
    InputCapacity,
    /// A caller tried to republish work prepared for another project lifetime.
    #[error("render plan belongs to a stale or foreign project")]
    StaleProject,
    /// Identity exhaustion must not wrap and revive stale work.
    #[error("video runtime identity space exhausted")]
    IdentityExhausted,
}

/// Startup fails before native resource ownership is transferred to a worker.
#[derive(Debug, Error)]
pub enum RuntimeStartError {
    /// Explicit limits violate a runtime invariant.
    #[error("invalid video runtime configuration: {0}")]
    InvalidConfig(&'static str),
    /// Shared time could not be initialized.
    #[error(transparent)]
    Time(#[from] golden_time::TimeError),
    /// CPU payload disposer could not start.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Bounded worker lifetime could not start.
    #[error(transparent)]
    Worker(#[from] golden_io::WorkerStartError),
}
