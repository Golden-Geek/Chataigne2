//! Chataigne state-machine behavior built on reusable Golden engines.

mod processor;
mod state_machine;

pub use chataigne_alchemist as alchemist;
pub use golden_statechart as statechart;
pub use processor::{
    ProcessorCommandPolicy, ProcessorDirtyFlags, ProcessorId, ProcessorLifecycleEvent, ProcessorLifecyclePolicy,
    ProcessorMemoryPolicy, ProcessorNode, ProcessorRuntime, ProcessorUiModel,
};
pub use state_machine::{
    ChataigneStateMachine, ChataigneStateMachineRuntime, ChataigneTransition, RuntimeExecutionMatrix,
    StateMachineTickOutput,
};

#[cfg(test)]
mod processor_tests;
#[cfg(test)]
mod state_machine_tests;
