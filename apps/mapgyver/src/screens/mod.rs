//! App-owned mapping authoring. Native outputs and render resources are not project state.

mod authoring;
mod calibration;
pub(crate) mod catalog_runtime;
mod commands;
pub(crate) mod creation;
pub mod display_matching;
mod geometry;
mod nodes;
pub mod protocol;
pub mod rendering;
mod screen_node;
mod scripting;
mod surface_node;

pub use authoring::{apply_operation, initialize, inspect, validate_project};
pub use commands::ScreenCommandNode;
pub use nodes::ScreenManagerNode;
pub use screen_node::ScreenNode;
pub use surface_node::SurfaceNode;

#[cfg(test)]
mod tests;
