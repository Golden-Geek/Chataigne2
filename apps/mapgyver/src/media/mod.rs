//! Persistent media/channel authoring; native resources remain owned by Golden Video.

mod assets;
pub(crate) mod authoring;
mod capture;
mod catalog;
mod commands;
pub mod composition;
pub(crate) mod dependencies;
mod importing;
mod nodes;
pub mod picture;
pub mod protocol;
pub mod rendering;
pub mod resolution;
mod scripting;

pub use assets::{AssetNode, asset};
pub use authoring::{apply_operation, channel, initialize, validate_project};
pub(crate) use capture::capture_channels_with_time;
pub use capture::{CapturedChannels, capture_channels, capture_channels_at};
pub use commands::MediaCommandNode;
pub use nodes::{ChannelNode, MediaManagerNode, MediaNode};

#[cfg(test)]
mod tests;
