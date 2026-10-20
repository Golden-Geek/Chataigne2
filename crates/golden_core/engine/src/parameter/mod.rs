mod animation_control;
mod parameter_node;

pub use animation_control::ParameterAnimationControlNode;
pub use golden_parameters::*;
pub use parameter_node::Parameter;
pub(crate) use parameter_node::ParameterAuthoredState;

#[cfg(test)]
mod tests;
