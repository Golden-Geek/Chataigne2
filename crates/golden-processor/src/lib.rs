//! Processor definitions, context lane compilation, and shared-kernel instances.

mod builtins;
mod compiler;
mod model;
mod runtime;

pub use builtins::{ACTION_FORMULA_ID, BuiltinFormulaAssets, MAPPING_FORMULA_ID};
pub use compiler::{ProcessorCompileError, ProcessorCompiler, ProcessorPlan};
pub use model::{
    MappingSpec, ProcessorDefinition, ProcessorDefinitionError, ProcessorId, ProcessorKind, ProcessorSurfaceBinding,
};
pub use runtime::{ProcessorBatchReport, ProcessorRuntime, ProcessorRuntimeError};

#[cfg(test)]
mod tests;
