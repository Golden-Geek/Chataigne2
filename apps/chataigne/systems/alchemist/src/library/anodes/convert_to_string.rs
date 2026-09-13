use std::sync::Arc;

use crate::{ANodeInstance, CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

use super::support::{config_string, decimal_string, format_runtime_value, time_string};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StringFormat {
    Decimal,
    Hexadecimal,
    Time,
    Compact,
}

impl StringFormat {
    pub(super) fn from_config(instance: &ANodeInstance) -> Self {
        match config_string(instance, "format", "decimal").as_str() {
            "hexadecimal" => Self::Hexadecimal,
            "time" => Self::Time,
            "compact" => Self::Compact,
            _ => Self::Decimal,
        }
    }
}

#[derive(Debug)]
pub(super) struct ConvertToStringEval {
    pub(super) format: StringFormat,
    pub(super) decimals: usize,
}

impl CompiledNodeEvaluator for ConvertToStringEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let Some(value) = evaluation.inputs.first() else {
            return Err("Convert To String expects one input".into());
        };
        ensure_finite(value)?;
        let text = match self.format {
            StringFormat::Decimal => decimal_string(value, self.decimals),
            StringFormat::Hexadecimal => match value {
                RuntimeValue::Int(number) => format!("0x{number:X}"),
                _ => return Err("hexadecimal formatting requires an integer".into()),
            },
            StringFormat::Time => match value {
                RuntimeValue::Float(number) => time_string(*number, self.decimals),
                RuntimeValue::Int(number) => time_string(*number as f64, self.decimals),
                _ => return Err("time formatting requires a float or integer".into()),
            },
            StringFormat::Compact => format_runtime_value(value, self.decimals),
        };
        Ok(vec![RuntimeValue::String(Arc::from(text))])
    }
}

fn ensure_finite(value: &RuntimeValue) -> Result<(), String> {
    let mut pending = vec![value];
    while let Some(value) = pending.pop() {
        let finite = match value {
            RuntimeValue::Float(value) => value.is_finite(),
            RuntimeValue::Vec2(value) => value.iter().all(|part| part.is_finite()),
            RuntimeValue::Vec3(value) => value.iter().all(|part| part.is_finite()),
            RuntimeValue::Color(value) => [value.red, value.green, value.blue, value.alpha]
                .iter()
                .all(|part| part.is_finite()),
            RuntimeValue::Array(values) => {
                pending.extend(values);
                true
            }
            _ => true,
        };
        if !finite {
            return Err("Convert To String requires finite numeric components".into());
        }
    }
    Ok(())
}
