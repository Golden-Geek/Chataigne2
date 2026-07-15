//! Chataigne-owned processor composition over reusable condition and formula kernels.

pub mod alchemist;
mod input_set;
mod managed_formula;
mod manager;
mod output_set;
mod processor;
#[cfg(any(test, feature = "testkit"))]
#[doc(hidden)]
pub mod test_support;
pub mod value_set;
mod value_set_pipeline;

pub use input_set::{INPUT_SOURCE_FIELD, InputSetError, InputSetItem, InputSetMaterialization, InputSetRuntime};
pub use managed_formula::{ManagedFormulaError, ManagedFormulaRuntime};
pub use manager::{
    ProcessorExecutionPolicy, ProcessorGroup, ProcessorGroupId, ProcessorManager, ProcessorManagerError,
    ProcessorManagerId,
};
pub use output_set::{
    COMMAND_INTENT_KIND, OUTPUT_TARGET_FIELD, OutputSetError, OutputSetItem, OutputSetMaterialization, OutputSetRuntime,
};
pub use processor::{
    ANodeOutputPreviewSample, DefaultProcessorContextProvider, Processor, ProcessorBindingAnalysis,
    ProcessorCommandPolicy, ProcessorContextPropertyBinding, ProcessorContextProvider, ProcessorDebugCapture,
    ProcessorDirtyFlags, ProcessorExecutionPlan, ProcessorExecutionStrategy, ProcessorFormulaSourceKind,
    ProcessorFormulaUiState, ProcessorId, ProcessorLaneOutput, ProcessorLifecycleEvent, ProcessorLifecyclePolicy,
    ProcessorMemoryPolicy, ProcessorRuntime, ProcessorUiModel, processor_output_preview_samples,
};
pub use value_set::{ValueLaneKey, ValueSet, ValueSetEntry, ValueSetError, lane_scoped_stable_ref};
pub use value_set_pipeline::{ValueSetPipelineError, ValueSetPipelineRuntime, ValueSetProjectionRuntime};

#[cfg(test)]
mod input_set_tests;
#[cfg(test)]
mod managed_formula_tests;
#[cfg(test)]
mod manager_tests;
#[cfg(test)]
mod output_set_tests;
#[cfg(test)]
mod processor_tests;
#[cfg(test)]
mod value_set_pipeline_tests;
#[cfg(test)]
mod value_set_tests;
