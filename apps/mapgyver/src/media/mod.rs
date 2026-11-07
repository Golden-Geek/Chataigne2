//! Persistent media/channel authoring; native resources remain owned by Golden Video.

mod assets;
pub(crate) mod authoring;
mod capture;
mod catalog;
mod channel_node;
pub mod commands;
pub mod composition;
pub(crate) mod creation;
pub(crate) mod dependencies;
pub(crate) mod folders;
mod importing;
mod media_node;
mod nodes;
mod persistence;
pub mod picture;
mod picture_file;
mod preview_capture;
mod preview_selection;
pub(crate) use preview_capture::capture_editor_channels;
pub mod protocol;
mod queries;
pub(crate) mod reference;
pub mod rendering;
pub mod resolution;
pub(crate) mod retention;
mod script_templates;
mod scripting;
pub(crate) mod sender_catalog;
mod source_api;
mod source_node;
pub mod spatial;
pub(crate) mod structure;
pub(crate) mod targeting;
mod visibility;

pub use assets::{AssetNode, asset};
pub use authoring::{apply_operation, channel, initialize, validate_project};
pub use capture::{CapturedChannels, capture_channels, capture_channels_at};
pub use channel_node::ChannelNode;
pub use commands::MediaCommandNode;
pub use folders::MediaFolderNode;
pub use media_node::MediaNode;
pub use nodes::MediaManagerNode;
pub(crate) use persistence::validate_project_file;
pub(crate) use queries::query_operation;
pub use reference::ReferenceNode;
pub use reference::ReferenceOutputNode;
pub use source_node::resolve_source_node;
pub(crate) use source_node::{MediaSource, source_selection};

#[cfg(test)]
mod tests;
