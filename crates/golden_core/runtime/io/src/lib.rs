//! App-agnostic primitives for event-driven IO workers and recoverable transports.

mod bounded_worker;
mod deadline;
mod notifier;
mod payload;
mod pending;
mod queue;
mod recovery;
mod retirement;
mod revision;
mod worker;

pub mod testkit;

pub use bounded_worker::{
    BoundedWorker, WorkerCancellation, WorkerContext, WorkerLimits, WorkerReceive, WorkerStartError, WorkerWake,
    WorkerWakeCheckpoint,
};
pub use deadline::DelayedNotifier;
pub use notifier::WorkerNotifier;
pub use payload::{
    PayloadBatch, PayloadCapacityError, PayloadPool, PayloadReservation, RetainedPayload, SharedPayload,
};
pub use pending::{
    PendingDrain, PendingDrainState, PendingMetricsSnapshot, PendingReceiver, PendingSendError, PendingSender,
    bounded_pending_channel, notified_pending_channel, pending_channel,
};
pub use queue::{BoundedQueue, QueueFull};
pub use recovery::ReconnectBackoff;
pub use retirement::{
    RetirementCapacityError, RetirementError, RetirementMetricsSnapshot, RetirementPermit, RetirementPool,
};
pub use revision::{RevisionGuard, RevisionReader, RevisionSource};
pub use worker::WorkerTask;

#[cfg(test)]
mod tests;
