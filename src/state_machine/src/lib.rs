//! Chataigne state-machine behavior built on reusable Golden engines.

pub mod alchemist;
mod arbitration;
mod input_set;
mod managed_formula;
mod manager;
mod output_set;
mod processor;
pub mod protocol;
mod state_machine;
pub mod value_set;
mod value_set_pipeline;

pub use arbitration::{
    ArbitrationDecision, ArbitrationResult, BlendPolicy, CommandDispatcher, CommandIntent, CommandIntentArbiter,
    CommandPolicy, IntentOrigin, RateLimitScope,
};
pub use golden_statechart as statechart;
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
    ProcessorMemoryPolicy, ProcessorMultiplexError, ProcessorMultiplexLimits, ProcessorRuntime, ProcessorUiModel,
    checked_context_cardinality, processor_output_preview_samples,
};
pub use protocol::{
    ANodeOutputPreviewSampleDto, ContextKeyDto, ContextKeyPartDto, ManagedItemDto, ManagedItemUiStateDto,
    ManagedRegionDefinitionDto, ManagedRegionInstanceDto, ManagedRegionKindDto, ManagedSocketRefDto,
    ProcessorFormulaSourceKindDto, ProcessorLaneConditionPreviewDto, ProcessorLaneInspectionDto,
    ProcessorLaneParameterPreviewDto, ProcessorLaneSummaryDto, ProcessorUiDto, StateMachineProtocolBundle,
    export_typescript,
};
pub use state_machine::{
    ChataigneStateMachine, ChataigneStateMachineRuntime, ChataigneTransition, GlobalCompiledGraphRuntime,
    GlobalStateMachineContextFrame, RuntimeExecutionMatrix, StateMachineTickOutput, StateMachineTransitionRuntime,
};
pub use value_set::{ValueLaneKey, ValueSet, ValueSetEntry, ValueSetError, lane_scoped_stable_ref};
pub use value_set_pipeline::{ValueSetPipelineError, ValueSetPipelineRuntime, ValueSetProjectionRuntime};

#[cfg(test)]
mod arbitration_tests;
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
mod protocol_tests;
#[cfg(test)]
mod state_machine_tests;
#[cfg(test)]
mod value_set_pipeline_tests;
#[cfg(test)]
mod value_set_tests;
