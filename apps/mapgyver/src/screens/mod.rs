//! App-owned mapping authoring. Native outputs and render resources are not project state.

mod authoring;
mod commands;
mod geometry;
mod nodes;
pub mod protocol;
mod scripting;

pub use authoring::{apply_operation, initialize, inspect, validate_project};
pub use commands::ScreenCommandNode;
pub use nodes::{ScreenManagerNode, ScreenNode, SurfaceNode};

#[cfg(test)]
mod tests;
