//! Video's app-owned transport policy. Golden owns clocks and codec/frame mechanics.
//!
//! A Video media has one owner, shared by all channel decoders and Reference consumers.
//! File workers supply durations and exact timestamp neighbours; transport never invents a
//! nominal frame rate, counts render callbacks or performs file work. Node/Parameter and worker
//! integration consume this boundary; this module does not persist authored settings in DTOs.

mod transport;
pub use transport::{MediaTransport, TransportError, TransportSample};

#[cfg(test)]
mod tests;
