//! Persistent media/channel authoring; native resources remain owned by Golden Video.

mod authoring;
mod nodes;
pub mod protocol;
pub mod resolution;

pub use authoring::{apply_operation, channel, initialize, validate_project};
pub use nodes::{ChannelNode, MediaNode};

#[cfg(test)]
mod tests;
