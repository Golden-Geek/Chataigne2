//! MapGyver Matrix document ownership. Golden supplies graph edits, history and execution mechanisms.
mod admission;
mod authoring;
mod bindings;
pub(crate) mod capture;
mod cells;
mod copying;
pub(crate) mod document;
pub(crate) mod node;
pub(crate) mod playback;
mod projection;
pub mod protocol;
mod timeline;
pub(crate) use admission::validate_ui_intent;
pub(crate) use authoring::apply;
pub(crate) use document::{inspect, validate_project};
pub use node::ElementNode;
#[cfg(test)]
mod tests;
