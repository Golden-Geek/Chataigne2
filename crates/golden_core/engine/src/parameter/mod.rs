mod animation_control;
mod parameter_node;
mod runtime_range;

pub use animation_control::ParameterAnimationControlNode;
pub use golden_parameters::*;
pub use parameter_node::Parameter;
pub(crate) use parameter_node::ParameterAuthoredState;
pub use runtime_range::RuntimeScalarRange;

#[cfg(test)]
mod tests;
