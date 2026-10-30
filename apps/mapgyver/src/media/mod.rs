//! Persistent media/channel authoring; native resources remain owned by Golden Video.

mod assets;
mod authoring;
mod capture;
mod catalog;
mod commands;
mod importing;
mod nodes;
pub mod picture;
pub mod protocol;
pub mod rendering;
pub mod resolution;
mod scripting;

pub use assets::{AssetNode, asset};
pub use authoring::{apply_operation, channel, initialize, validate_project};
pub use capture::{CapturedChannels, capture_channels};
pub use commands::MediaCommandNode;
pub use nodes::{ChannelNode, MediaManagerNode, MediaNode};

#[cfg(test)]
mod tests;
