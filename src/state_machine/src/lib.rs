//! Chataigne state-machine behavior built on reusable Golden engines.

pub mod alchemist;
mod arbitration;
mod manager;
mod models;
mod processor;
pub mod protocol;
mod state_machine;

pub use arbitration::{
    ArbitrationDecision, ArbitrationResult, BlendPolicy, CommandDispatcher, CommandIntent, CommandIntentArbiter,
    CommandPolicy, IntentOrigin,
};
pub use golden_statechart as statechart;
pub use manager::{
    ProcessorExecutionPolicy, ProcessorGroup, ProcessorGroupId, ProcessorManager, ProcessorManagerError,
    ProcessorManagerId,
};
pub use models::builtin_formulas;
pub use processor::{
    Processor, ProcessorCommandPolicy, ProcessorDirtyFlags, ProcessorId, ProcessorLifecycleEvent,
    ProcessorLifecyclePolicy, ProcessorMemoryPolicy, ProcessorRuntime, ProcessorUiModel,
};
pub use protocol::export_typescript;
pub use state_machine::{
    ChataigneStateMachine, ChataigneStateMachineRuntime, ChataigneTransition, RuntimeExecutionMatrix,
    StateMachineTickOutput,
};

#[cfg(test)]
mod arbitration_tests;
#[cfg(test)]
mod manager_tests;
#[cfg(test)]
mod models_tests;
#[cfg(test)]
mod processor_tests;
#[cfg(test)]
mod state_machine_tests;
