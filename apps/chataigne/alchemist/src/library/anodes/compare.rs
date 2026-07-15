use crate::{ANodeInstance, CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

use super::support::{brightness, config_string, format_runtime_value, require_inputs, value_to_f64};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Comparator {
    Equal,
    NotEqual,
    Greater,
    GreaterOrEqual,
    Less,
    LessOrEqual,
    Longer,
    Shorter,
    Contains,
    Brighter,
    Darker,
}

impl Comparator {
    pub(super) fn from_config(instance: &ANodeInstance) -> Self {
        match config_string(instance, "comparator", "equal").as_str() {
            "not_equal" => Self::NotEqual,
            "greater" => Self::Greater,
            "greater_or_equal" => Self::GreaterOrEqual,
            "less" => Self::Less,
            "less_or_equal" => Self::LessOrEqual,
            "longer" => Self::Longer,
            "shorter" => Self::Shorter,
            "contains" => Self::Contains,
            "brighter" => Self::Brighter,
            "darker" => Self::Darker,
            _ => Self::Equal,
        }
    }
}

#[derive(Debug)]
pub(super) struct CompareEval {
    pub(super) comparator: Comparator,
}

impl CompiledNodeEvaluator for CompareEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let [left, right] = require_inputs::<2>(evaluation.inputs)?;
        let result = match self.comparator {
            Comparator::Equal => left == right,
            Comparator::NotEqual => left != right,
            Comparator::Greater => value_to_f64(left) > value_to_f64(right),
            Comparator::GreaterOrEqual => value_to_f64(left) >= value_to_f64(right),
            Comparator::Less => value_to_f64(left) < value_to_f64(right),
            Comparator::LessOrEqual => value_to_f64(left) <= value_to_f64(right),
            Comparator::Longer => format_runtime_value(left, 3).len() > format_runtime_value(right, 3).len(),
            Comparator::Shorter => format_runtime_value(left, 3).len() < format_runtime_value(right, 3).len(),
            Comparator::Contains => format_runtime_value(left, 3).contains(&format_runtime_value(right, 3)),
            Comparator::Brighter => brightness(left) > brightness(right),
            Comparator::Darker => brightness(left) < brightness(right),
        };
        Ok(vec![RuntimeValue::Bool(result)])
    }
}
