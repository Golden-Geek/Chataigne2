//! Statechart authoring on `golden-graph` and compiled, non-multiplexed execution.

mod compiler;
mod model;
mod runtime;

pub use compiler::{CompiledState, CompiledTransition, StatechartCompileError, StatechartCompiler, StatechartPlan};
pub use model::{StateKind, StateNode, StatechartData, StatechartGraphDomain, StatechartPort, TransitionData};
pub use runtime::{ActiveConfiguration, ProcessorInvocation, StatechartRuntime, StatechartStep};

#[cfg(test)]
mod tests;
