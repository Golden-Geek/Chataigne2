//! Persistent media/channel authoring; native resources remain owned by Golden Video.

mod assets;
pub(crate) mod authoring;
mod capture;
mod catalog;
mod channel_node;
pub mod commands;
pub mod composition;
mod creation;
pub(crate) mod dependencies;
pub(crate) mod folders;
mod importing;
mod media_node;
mod nodes;
mod persistence;
pub mod picture;
pub mod protocol;
pub(crate) mod reference;
pub mod rendering;
pub mod resolution;
pub(crate) mod retention;
mod script_templates;
mod scripting;
mod source_api;
mod source_node;
pub(crate) mod structure;

pub use assets::{AssetNode, asset};
pub use authoring::{apply_operation, channel, initialize, validate_project};
pub(crate) use capture::capture_channels_with_time;
pub use capture::{CapturedChannels, capture_channels, capture_channels_at};
pub use channel_node::ChannelNode;
pub use commands::MediaCommandNode;
pub use folders::MediaFolderNode;
pub use media_node::MediaNode;
pub use nodes::MediaManagerNode;
pub(crate) use persistence::validate_project_file;
pub use reference::ReferenceNode;
pub use source_node::resolve_source_node;
pub(crate) use source_node::{MediaSource, source_selection};

#[cfg(test)]
mod tests;
