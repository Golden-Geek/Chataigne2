//! Chataigne-owned processor composition over reusable condition and formula kernels.

pub mod alchemist;
mod channel_frame;
mod input_set;
mod kernel_profile;
mod managed_formula;
mod managed_stage;
mod manager;
mod output_set;
mod processor;
#[cfg(any(test, feature = "testkit"))]
#[doc(hidden)]
pub mod testkit;
pub mod value_set;
mod value_set_pipeline;

pub use channel_frame::{ChannelFrame, ChannelFrameError, ChannelSlot, ChannelValidity};
pub use chataigne_alchemist::ValueLaneKey;
pub use input_set::{
    ChannelSourceSchema, INPUT_SOURCE_FIELD, InputSetError, InputSetItem, InputSetMaterialization, InputSetRuntime,
};
#[cfg(feature = "kernel-profiling")]
pub use kernel_profile::{ProcessorKernelProfile, processor_kernel_profile_snapshot};
pub use managed_formula::{
    ExecutableFilterApplication, ManagedFilterAvailabilityError, ManagedFormulaError, ManagedFormulaRuntime,
    executable_filter_applications, validate_executable_filter_application, validate_mapping_filter_application,
    validate_trigger_filter_application,
};
pub use managed_stage::{ManagedStageChain, ManagedStageError, ManagedStageRuntime, ManagedStageSpecializationCache};
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
    processor_output_preview_samples_from_lanes,
};
pub use value_set::{ValueSet, ValueSetEntry, ValueSetError};
pub use value_set_pipeline::{
    RuntimeInputBinding, ValueSetPipelineError, ValueSetPipelineRuntime, ValueSetProjectionRuntime,
};

#[cfg(test)]
mod tests;
