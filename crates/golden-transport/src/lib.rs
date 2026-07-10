//! Transport-side queues, observation interests, safeguards, and metrics.

mod handle;
mod interests;
mod queue;
mod security;

pub use handle::{ControlHandleError, EngineControlHandle};
pub use interests::ObservationRegistry;
pub use queue::{ClientOutboundQueue, OutboundFrame, QueueError, TransportMetrics, TransportMetricsSnapshot};
pub use security::{AdmissionError, ConnectionLimiter, ConnectionPermit, NetworkPolicy, NetworkPolicyError};

#[cfg(test)]
mod tests;
