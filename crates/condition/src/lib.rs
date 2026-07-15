mod authoring;
mod compile;
mod runtime;

pub use authoring::{
    ConditionBehavior, ConditionDefinition, ConditionGroupPolicy, ConditionId, ConditionKind, ConditionOperand,
    ConditionProjection, EdgePolicy, InputNodeCondition, InputValueCondition, ScriptCondition, TypedComparator,
};
pub use compile::{
    CompiledConditionInstruction, CompiledConditionProgram, ConditionCompileDiagnostic, ConditionObservationDescriptor,
    ConditionStateKey, ConditionStateLayout, compile_condition,
};
pub use runtime::{
    ConditionEvaluationError, ConditionEvaluationFrame, ConditionEvaluationResult, ConditionInputProvider,
    ConditionRuntime, ConditionState,
};

#[cfg(test)]
mod tests;
