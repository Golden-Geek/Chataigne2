use std::sync::Arc;

use crate::{ANodeInstance, CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

use super::support::{config_string, decimal_string};

#[derive(Clone, Copy, Debug)]
pub(super) enum ScalarTarget {
    Int,
    Float,
    Bool,
    String,
}

impl ScalarTarget {
    pub(super) fn from_config(instance: &ANodeInstance) -> Self {
        match config_string(instance, "target", "float").as_str() {
            "int" => Self::Int,
            "bool" => Self::Bool,
            "string" => Self::String,
            _ => Self::Float,
        }
    }

    pub(super) const fn type_name(self) -> &'static str {
        match self {
            Self::Int => "int",
            Self::Float => "float",
            Self::Bool => "bool",
            Self::String => "string",
        }
    }
}

#[derive(Debug)]
pub(super) struct ConvertScalarEval {
    pub(super) target: ScalarTarget,
}

impl CompiledNodeEvaluator for ConvertScalarEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<crate::NodeOutputs, String> {
        let [value] = evaluation.inputs else {
            return Err("scalar conversion expects one input".into());
        };
        let converted = convert(value, self.target)?;
        Ok(crate::node_outputs![converted])
    }
}

pub(super) fn convert(value: &RuntimeValue, target: ScalarTarget) -> Result<RuntimeValue, String> {
    match target {
        ScalarTarget::Int => Ok(RuntimeValue::Int(to_int(value)?)),
        ScalarTarget::Float => Ok(RuntimeValue::Float(to_float(value)?)),
        ScalarTarget::Bool => Ok(RuntimeValue::Bool(to_bool(value)?)),
        ScalarTarget::String => {
            if matches!(value, RuntimeValue::Float(number) if !number.is_finite()) {
                return Err("non-finite float cannot convert to string".into());
            }
            Ok(RuntimeValue::String(Arc::from(decimal_string(value, 3))))
        }
    }
}

fn to_int(value: &RuntimeValue) -> Result<i64, String> {
    match value {
        RuntimeValue::Int(value) => Ok(*value),
        RuntimeValue::Bool(value) => Ok(i64::from(*value)),
        RuntimeValue::Float(value) if value.is_finite() => {
            let truncated = value.trunc();
            if truncated < i64::MIN as f64 || truncated >= 9_223_372_036_854_775_808.0 {
                return Err("float is outside the integer range".into());
            }
            Ok(truncated as i64)
        }
        RuntimeValue::String(value) => value
            .trim()
            .parse::<i64>()
            .map_err(|_| "string is not a valid integer".into()),
        RuntimeValue::Float(_) => Err("non-finite float cannot convert to integer".into()),
        _ => Err("integer conversion requires int, float, bool, or string".into()),
    }
}

fn to_float(value: &RuntimeValue) -> Result<f64, String> {
    let converted = match value {
        RuntimeValue::Int(value) => *value as f64,
        RuntimeValue::Float(value) => *value,
        RuntimeValue::Bool(value) => f64::from(u8::from(*value)),
        RuntimeValue::String(value) => value
            .trim()
            .parse::<f64>()
            .map_err(|_| "string is not a valid float".to_owned())?,
        _ => return Err("float conversion requires int, float, bool, or string".into()),
    };
    converted
        .is_finite()
        .then_some(converted)
        .ok_or_else(|| "non-finite float conversion result".into())
}

fn to_bool(value: &RuntimeValue) -> Result<bool, String> {
    match value {
        RuntimeValue::Bool(value) => Ok(*value),
        RuntimeValue::Int(value) => Ok(*value != 0),
        RuntimeValue::Float(value) if value.is_finite() => Ok(*value != 0.0),
        RuntimeValue::Float(_) => Err("non-finite float cannot convert to boolean".into()),
        RuntimeValue::String(value) => match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" => Ok(true),
            "false" | "0" => Ok(false),
            _ => Err("string is not a valid boolean".into()),
        },
        _ => Err("boolean conversion requires int, float, bool, or string".into()),
    }
}
