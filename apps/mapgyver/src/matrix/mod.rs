//! MapGyver Matrix document ownership. Golden supplies graph edits, history and execution mechanisms.
mod admission;
mod applications;
mod authoring;
mod bindings;
pub(crate) mod capture;
mod cell_applications;
pub(crate) mod cell_node;
pub(crate) mod cells;
mod copying;
pub(crate) mod document;
pub(crate) mod node;
pub(crate) mod playback;
mod projection;
pub mod protocol;
mod timeline;
mod view;
pub(crate) use admission::validate_ui_intent;
pub(crate) use applications::capture_application_columns;
pub(crate) use authoring::apply;
pub use cell_node::CellNode;
pub(crate) use document::{inspect, validate_project};
pub use node::ElementNode;
#[cfg(test)]
mod tests;
