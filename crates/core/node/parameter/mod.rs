pub use crate::color::Color;

mod control;
mod constraints;
mod parameter_node;
mod projection;
mod types;
mod value;

pub use control::*;
pub use constraints::*;
pub use parameter_node::Parameter;
pub use projection::*;
pub use types::*;
pub use value::*;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;