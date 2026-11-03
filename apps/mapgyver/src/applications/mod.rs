//! App-owned executable definitions and column demand. Golden IO owns OS process mechanisms.
mod authoring;
pub mod configuration;
pub mod demand;
pub mod driver;
mod entries;
pub mod headless;
pub mod logical;
mod node;
pub mod protocol;
pub mod runtime;
mod settings;
pub(crate) use authoring::control_node;
pub use authoring::inspect;
pub(crate) use authoring::{apply, validate_associations, validate_project, validate_ui_intent};
pub use entries::{ArgumentNode, EnvironmentNode};
pub use node::AppControlNode;
pub(crate) use settings::configure as configure_node;
#[cfg(test)]
mod tests;
