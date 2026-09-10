//! App-agnostic primitives for event-driven IO workers and recoverable transports.

mod pending;
mod queue;
mod recovery;
mod retirement;
mod worker;

pub mod testkit;

pub use pending::{
    PendingDrain, PendingDrainState, PendingMetricsSnapshot, PendingReceiver, PendingSendError, PendingSender,
    bounded_pending_channel, pending_channel,
};
pub use queue::{BoundedQueue, QueueFull};
pub use recovery::ReconnectBackoff;
pub use retirement::{
    RetirementCapacityError, RetirementError, RetirementMetricsSnapshot, RetirementPermit, RetirementPool,
};
pub use worker::WorkerTask;

#[cfg(test)]
mod tests;
