//! Immutable runtime generations and allocation-stable semantic execution.

mod compiler;
mod control;
mod model;
mod semantic;
mod service;

pub use compiler::{GenerationCompiler, RuntimeCompileError};
pub use control::{ControlPlaneEvent, RuntimeControlPlane};
pub use model::{
    DenseSlot, DirectInputSlot, EffectOrder, EffectSinkId, GenerationId, GenerationSpec, InputBindingSpec, InputId,
    InputUpdate, OperationSpec, RuntimeBatch, RuntimeGeneration, RuntimeOperation, ScalarConversionError, ScalarType,
    ScalarValue, SlotSpec, StableStateKey, StateSpec,
};
pub use semantic::{
    CommittedEffect, EffectCommitter, ExecutionMode, GenerationSwapReport, SemanticRuntime, SemanticRuntimeError,
    TickMetrics,
};
pub use service::{CompilationRequestId, CompilationResult, CompilationService};

#[cfg(test)]
mod tests;
