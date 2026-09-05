//! App-agnostic primitives for event-driven IO workers and recoverable transports.

mod pending;
mod queue;
mod recovery;
mod worker;

pub mod testkit;

pub use pending::{PendingDrain, PendingDrainState, PendingReceiver, PendingSender, pending_channel};
pub use queue::{BoundedQueue, QueueFull};
pub use recovery::ReconnectBackoff;
pub use worker::WorkerTask;

#[cfg(test)]
mod tests;
