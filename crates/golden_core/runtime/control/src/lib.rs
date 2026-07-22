//! App-agnostic compiled runtime, scheduling, and control-plane primitives.
//!
//! Authoring documents are compiled outside steady-state execution. A live semantic runtime owns
//! one immutable generation and dense arenas; authored identifiers are retained only in compile
//! catalogs and state-migration keys.

#![warn(missing_docs)]

mod compiler;
mod control;
mod effects;
mod generation;
mod ids;
mod input;
mod metrics;
mod scheduler;

pub use compiler::{
    CompilationCompletion, CompilationError, CompilationHandle, CompilationService, CompileRequest, GenerationCompiler,
    RuntimeChangeSet,
};
pub use control::{ControlActor, ControlError, ControlHandle, ControlReceipt, ControlStatus, PendingControl};
pub use effects::{EffectBuffer, EffectCommitMode, EffectCommitReport, EffectSink, StagedEffect};
pub use generation::{
    ArenaLayout, CompiledArtifact, CompiledContextCatalog, CompiledProcessorKernel, CompiledStatechart, EffectRoute,
    EffectRoutingTable, GenerationSwapReport, InputRoute, InputRoutingTable, ObservationCatalog, ObservationRoute,
    ProcessorInstanceLayout, RuntimeArenas, RuntimeGeneration, RuntimeGenerationBuilder, RuntimeGenerationError,
    SemanticRuntime, StableStateBinding,
};
pub use ids::{
    ArtifactId, EffectSlot, InputSlot, KernelId, LaneIndex, ProcessorInstanceId, ProjectRevision, RuntimeGenerationId,
    StableStateKey, StateSlot, ValueSlot, WorkUnitId,
};
pub use input::{
    InputDelivery, InputIngressConfig, InputIngressError, RuntimeInputHandle, RuntimeInputMailbox, RuntimeInputUpdate,
};
pub use metrics::{RuntimeMetrics, RuntimeMetricsSnapshot};
pub use scheduler::{
    BatchExecution, BatchExecutor, DirtySet, ExecutionMode, PersistentBatchScheduler, RuntimeSchedule, ScheduledWork,
};

#[cfg(test)]
mod tests;
