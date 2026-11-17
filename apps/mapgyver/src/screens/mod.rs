//! App-owned mapping authoring. Native outputs and render resources are not project state.

mod authoring;
pub(crate) mod calibration;
pub(crate) mod catalog_runtime;
pub(crate) mod creation;
pub mod display_matching;
mod editor;
mod frame_rate;
mod geometry;
mod handle_links;
pub(crate) mod host_inventory;
mod nodes;
pub mod protocol;
mod queries;
mod related;
pub mod rendering;
mod rows;
mod screen_node;
mod scripting;
mod source_preview;
mod surface_node;
mod target;

#[cfg(test)]
pub(crate) use authoring::initialize_with_displays;
pub use authoring::{apply_operation, initialize, inspect, validate_project};
pub(crate) use calibration::CalibrationAssets;
pub use nodes::ScreenManagerNode;
pub(crate) use queries::query_operation;
pub use screen_node::ScreenNode;
pub use surface_node::SurfaceNode;

#[cfg(test)]
mod tests;
