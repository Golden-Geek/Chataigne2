//! Chataigne state-machine behavior built on reusable Golden engines.

mod processor;

pub use chataigne_alchemist as alchemist;
pub use golden_statechart as statechart;
pub use processor::{
    ProcessorCommandPolicy, ProcessorDirtyFlags, ProcessorId, ProcessorLifecycleEvent, ProcessorLifecyclePolicy,
    ProcessorMemoryPolicy, ProcessorNode, ProcessorRuntime, ProcessorUiModel,
};

#[cfg(test)]
mod processor_tests;
