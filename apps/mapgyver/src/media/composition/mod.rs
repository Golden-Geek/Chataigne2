//! Composition document editing. Instance ownership is independent from referenced source media.

mod authoring;
mod dependencies;
mod node;
pub mod protocol;

pub(super) use authoring::apply;
pub(crate) use dependencies::validate_project;
pub(crate) use node::INSTANCE_TYPE;
pub use node::InstanceNode;

#[cfg(test)]
mod tests;
