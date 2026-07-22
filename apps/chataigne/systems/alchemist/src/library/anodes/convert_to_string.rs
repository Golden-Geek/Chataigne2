use std::sync::Arc;

use crate::{ANodeInstance, CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

use super::support::{config_string, decimal_string, format_runtime_value, time_string, value_to_f64, value_to_i64};

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
        let text = match self.format {
            StringFormat::Decimal => decimal_string(value, self.decimals),
            StringFormat::Hexadecimal => format!("0x{:X}", value_to_i64(value)),
            StringFormat::Time => time_string(value_to_f64(value), self.decimals),
            StringFormat::Compact => format_runtime_value(value, self.decimals),
        };
        Ok(vec![RuntimeValue::String(Arc::from(text))])
    }
}
