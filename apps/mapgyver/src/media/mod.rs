//! Persistent media/channel authoring; native resources remain owned by Golden Video.

mod assets;
mod authoring;
mod capture;
mod catalog;
mod nodes;
pub mod picture;
pub mod protocol;
pub mod rendering;
pub mod resolution;

pub use assets::{AssetNode, asset};
pub use authoring::{apply_operation, channel, initialize, validate_project};
pub use capture::{CapturedChannels, capture_channels};
pub use nodes::{ChannelNode, MediaManagerNode, MediaNode};

#[cfg(test)]
mod tests;
