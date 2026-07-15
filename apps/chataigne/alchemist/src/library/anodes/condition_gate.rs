use crate::{
    ANodeInstance, ANodeSignature, CompiledNodeEvaluator, InputSocketDecl, NodeEvaluation, OutputSocketDecl,
    RuntimeValue, TriggerValue, TypeBindingSource, TypeBindings, TypeConstraint, TypeVar, ValueTypeId,
};

use super::support::{config_string, exact};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ConditionGateMode {
    PassWhenTrue,
    PassWhenFalse,
    HoldLast,
    OutputDefault,
    BlockTrigger,
}

impl ConditionGateMode {
    fn from_config(instance: &ANodeInstance) -> Self {
        match config_string(instance, "mode", "pass_when_true").as_str() {
            "pass_when_false" => Self::PassWhenFalse,
            "hold_last" => Self::HoldLast,
            "output_default" => Self::OutputDefault,
            "block_trigger" => Self::BlockTrigger,
            _ => Self::PassWhenTrue,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GateApplication {
    Whole,
    PerLane,
}

impl GateApplication {
    fn from_config(instance: &ANodeInstance) -> Self {
        match config_string(instance, "gate_application", "whole").as_str() {
            "per_lane" => Self::PerLane,
            _ => Self::Whole,
        }
    }
}

#[derive(Debug)]
pub(super) struct ConditionGateEval {
    mode: ConditionGateMode,
    application: GateApplication,
}

impl ConditionGateEval {
    pub(super) fn from_config(instance: &ANodeInstance) -> Self {
        Self {
            mode: ConditionGateMode::from_config(instance),
            application: GateApplication::from_config(instance),
        }
    }
}

impl CompiledNodeEvaluator for ConditionGateEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        if self.application == GateApplication::PerLane {
            return Err("ConditionGate per-lane application requires lane-aware ValueSet lowering".into());
        }
        let [value, condition, default_value] = evaluation
            .inputs
            .iter()
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| "ConditionGate expects value, condition, and default value inputs".to_string())?;
        let RuntimeValue::Bool(condition) = condition else {
            return Err("ConditionGate expects a boolean condition input".into());
        };
        let passes = match self.mode {
            ConditionGateMode::PassWhenFalse => !condition,
            _ => *condition,
        };
        let output_value = match self.mode {
            ConditionGateMode::HoldLast => hold_last_output(evaluation.state, value, default_value, passes),
            ConditionGateMode::BlockTrigger => block_trigger_output(value, default_value, passes),
            ConditionGateMode::PassWhenTrue | ConditionGateMode::PassWhenFalse | ConditionGateMode::OutputDefault => {
                if passes {
                    value.clone()
                } else {
                    default_value.clone()
                }
            }
        };
        Ok(vec![
            output_value,
            RuntimeValue::Bool(passes),
            RuntimeValue::Bool(!passes),
        ])
    }
}

fn hold_last_output(
    state: &mut [RuntimeValue],
    value: &RuntimeValue,
    default_value: &RuntimeValue,
    passes: bool,
) -> RuntimeValue {
    if passes {
        if let Some(state) = state.first_mut() {
            *state = value.clone();
        }
        return value.clone();
    }
    state
        .first()
        .filter(|value| !matches!(value, RuntimeValue::Unit))
        .cloned()
        .unwrap_or_else(|| default_value.clone())
}

fn block_trigger_output(value: &RuntimeValue, default_value: &RuntimeValue, passes: bool) -> RuntimeValue {
    let RuntimeValue::Trigger(trigger) = value else {
        return if passes { value.clone() } else { default_value.clone() };
    };
    RuntimeValue::Trigger(TriggerValue {
        fired: trigger.fired && passes,
        ..*trigger
    })
}

pub(super) fn signature() -> ANodeSignature {
    let variable = TypeVar::new("TValue");
    let mut default_bindings = TypeBindings::default();
    let mut generic_constraints = indexmap::IndexMap::new();
    default_bindings.insert(variable.clone(), ValueTypeId::new("float"), TypeBindingSource::Default);
    generic_constraints.insert(variable.clone(), TypeConstraint::Any);
    ANodeSignature {
        inputs: vec![
            InputSocketDecl::new("value", "Value", TypeConstraint::Generic(variable.clone())),
            InputSocketDecl::new("condition", "Condition", exact("bool")),
            InputSocketDecl::new("default_value", "Default", TypeConstraint::Generic(variable.clone())),
        ],
        outputs: vec![
            OutputSocketDecl::new("value", "Value", TypeConstraint::Generic(variable)),
            OutputSocketDecl::new("passed", "Passed", exact("bool")),
            OutputSocketDecl::new("blocked", "Blocked", exact("bool")),
        ],
        default_bindings,
        generic_constraints,
    }
}
