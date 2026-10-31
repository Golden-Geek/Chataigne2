//! App-owned executable definitions and column demand. Golden IO owns OS process mechanisms.
mod authoring;
pub mod demand;
pub mod driver;
pub mod logical;
mod node;
pub mod protocol;
pub mod runtime;
pub use authoring::inspect;
pub(crate) use authoring::{apply, validate_associations, validate_project, validate_ui_intent};
pub use node::ApplicationNode;
#[cfg(test)]
mod tests;
