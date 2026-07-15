//! Chataigne state-machine behavior built on reusable Golden engines.

mod arbitration;
pub mod protocol;
mod state_machine;

pub use arbitration::{
    ArbitrationDecision, ArbitrationResult, BlendPolicy, CommandDispatcher, CommandIntent, CommandIntentArbiter,
    CommandPolicy, IntentOrigin, RateLimitScope,
};
pub use chataigne_processor::*;
pub use golden_statechart as statechart;
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

#[cfg(test)]
mod arbitration_tests;
#[cfg(test)]
mod protocol_tests;
#[cfg(test)]
mod state_machine_tests;
