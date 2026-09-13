use crate::{
    ANodeInstance, ANodeSignature, CompiledNodeEvaluator, InputSocketDecl, NodeEvaluation, OutputSocketDecl,
    RuntimeValue, TriggerValue, TypeBindingSource, TypeBindings, TypeConstraint, TypeVar, ValueTypeId,
};

use super::support::exact;

fn gate_config_string<'a>(instance: &'a ANodeInstance, field: &str, fallback: &'a str) -> Result<&'a str, String> {
    match instance.config.get(field) {
        Some(RuntimeValue::String(value)) => Ok(value),
        Some(value) => Err(format!(
            "Condition Gate `{field}` expects String, got {}",
            value.value_type()
        )),
        None => Ok(fallback),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ConditionGateMode {
    PassWhenTrue,
    PassWhenFalse,
    HoldLast,
    HoldLastWithDefault,
    OutputDefault,
    OutputDefaultWhenFalse,
    BlockTrigger,
    BlockTriggerWithDefault,
}

impl ConditionGateMode {
    fn from_config(instance: &ANodeInstance) -> Result<Self, String> {
        let mode = gate_config_string(instance, "mode", "pass_when_true")?;
        match mode {
            "pass_when_true" => Ok(Self::PassWhenTrue),
            "pass_when_false" => Ok(Self::PassWhenFalse),
            "hold_last" => Ok(Self::HoldLast),
            "hold_last_with_default" => Ok(Self::HoldLastWithDefault),
            "output_default" => Ok(Self::OutputDefault),
            "output_default_when_false" => Ok(Self::OutputDefaultWhenFalse),
            "block_trigger" => Ok(Self::BlockTrigger),
            "block_trigger_with_default" => Ok(Self::BlockTriggerWithDefault),
            _ => Err(format!("unknown Condition Gate mode `{mode}`")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GateApplication {
    Whole,
    PerLane,
}

impl GateApplication {
    fn from_config(instance: &ANodeInstance) -> Result<Self, String> {
        let application = gate_config_string(instance, "gate_application", "whole")?;
        match application {
            "whole" => Ok(Self::Whole),
            "per_lane" => Ok(Self::PerLane),
            _ => Err(format!("unknown Condition Gate application `{application}`")),
        }
    }
}

#[derive(Debug)]
pub(super) struct ConditionGateEval {
    mode: ConditionGateMode,
    application: GateApplication,
    explicit_default: bool,
    implicit_managed_default: bool,
}

impl ConditionGateEval {
    pub(super) fn from_config(instance: &ANodeInstance) -> Result<Self, String> {
        Ok(Self {
            mode: ConditionGateMode::from_config(instance)?,
            application: GateApplication::from_config(instance)?,
            explicit_default: instance
                .input_defaults
                .contains_key(&crate::SocketId::new("default_value")),
            implicit_managed_default: instance.config.get(crate::MANAGED_IMPLICIT_GATE_DEFAULT_FIELD)
                == Some(&RuntimeValue::Bool(true)),
        })
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
            ConditionGateMode::PassWhenFalse | ConditionGateMode::OutputDefaultWhenFalse => !condition,
            _ => *condition,
        };
        if !passes {
            match self.mode {
                ConditionGateMode::PassWhenTrue
                | ConditionGateMode::PassWhenFalse
                | ConditionGateMode::BlockTrigger => {
                    evaluation.suppress_output(0);
                }
                ConditionGateMode::HoldLast
                    if !(self.explicit_default
                        || (!self.implicit_managed_default && evaluation.input_has_connection(2)))
                        && evaluation
                            .state
                            .first()
                            .is_none_or(|value| matches!(value, RuntimeValue::Unit)) =>
                {
                    evaluation.suppress_output(0);
                }
                ConditionGateMode::HoldLast
                | ConditionGateMode::HoldLastWithDefault
                | ConditionGateMode::OutputDefault
                | ConditionGateMode::OutputDefaultWhenFalse
                | ConditionGateMode::BlockTriggerWithDefault => {}
            }
        }
        let output_value = match self.mode {
            ConditionGateMode::HoldLast | ConditionGateMode::HoldLastWithDefault => {
                hold_last_output(evaluation.state, value, default_value, passes)
            }
            ConditionGateMode::BlockTrigger | ConditionGateMode::BlockTriggerWithDefault => {
                block_trigger_output(value, default_value, passes)
            }
            ConditionGateMode::PassWhenTrue
            | ConditionGateMode::PassWhenFalse
            | ConditionGateMode::OutputDefault
            | ConditionGateMode::OutputDefaultWhenFalse => {
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
