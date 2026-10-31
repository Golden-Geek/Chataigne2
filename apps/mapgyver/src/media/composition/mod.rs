//! Composition document editing. Instance ownership is independent from referenced source media.

pub(super) mod authoring;
use crate::media::dependencies;
mod geometry;
mod node;
pub mod protocol;

pub(super) use authoring::apply;
pub(crate) use node::INSTANCE_TYPE;
pub use node::InstanceNode;

#[cfg(test)]
mod tests;
