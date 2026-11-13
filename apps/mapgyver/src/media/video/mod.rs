//! Video's app-owned transport policy. Golden owns clocks and codec/frame mechanics.
//!
//! A Video media has one owner, shared by all channel decoders and Reference consumers.
//! File workers supply durations and exact timestamp neighbours; transport never invents a
//! nominal frame rate, counts render callbacks or performs file work. Node/Parameter and worker
//! integration consume this boundary; this module does not persist authored settings in DTOs.

pub(crate) mod authoring;
pub(crate) mod bindings;
mod control;
pub(crate) mod feedback;
mod frames;
pub mod protocol;
mod transport;
pub use control::{MatrixVideoUse, VideoControl};
#[cfg(any(feature = "video-files", test))]
pub(crate) use frames::Neighbours;
pub use transport::{MediaTransport, TransportError, TransportSample};
#[cfg(feature = "video-files")]
pub(crate) mod files;
#[cfg(feature = "video-files")]
pub use files::{FileCpuAdmission, FileRetirement};
#[cfg(feature = "video-files")]
mod failure;
#[cfg(feature = "video-files")]
pub(crate) mod metadata;
#[cfg(feature = "video-files")]
mod reader;
#[cfg(feature = "video-files")]
pub(crate) mod registry;

#[cfg(test)]
mod tests;
