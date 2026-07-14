pub use crate::color::Color;

mod animation_control;
mod canonical;
mod constraints;
mod control;
mod parameter_node;
mod projection;
mod types;
mod value;
mod value_type;

pub use animation_control::ParameterAnimationControlNode;
pub use canonical::*;
pub use constraints::*;
pub use control::*;
pub use parameter_node::Parameter;
pub use projection::*;
pub use types::*;
pub use value::*;
pub use value_type::*;

#[cfg(test)]
mod canonical_tests;
#[cfg(test)]
mod tests;
