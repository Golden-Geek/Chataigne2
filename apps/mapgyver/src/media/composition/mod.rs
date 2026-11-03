//! Composition document editing over owned media and typed placement Parameters.

pub(super) mod authoring;
pub(crate) mod children;
use crate::media::dependencies;
mod drops;
mod geometry;
mod node;
pub mod protocol;
pub(in crate::media) mod query;

pub(super) use authoring::apply;
pub use authoring::inspect;
pub(crate) use node::PLACEMENT_TYPE;
pub use node::PlacementNode;

#[cfg(test)]
mod tests;
