//! App-owned mapping authoring. Native outputs and render resources are not project state.

mod authoring;
mod commands;
mod nodes;
pub mod protocol;
pub mod rendering;
mod scripting;

pub use authoring::{apply_operation, initialize, inspect, validate_project};
pub use commands::ScreenCommandNode;
pub use nodes::{ScreenManagerNode, ScreenNode, SurfaceNode};

#[cfg(test)]
mod tests;
