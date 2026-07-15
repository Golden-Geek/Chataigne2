use crate::{ANodeInstance, CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

use super::support::{config_string, runtime_bool};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BooleanOperator {
    And,
    Or,
    Xor,
}

impl BooleanOperator {
    pub(super) fn from_config(instance: &ANodeInstance) -> Self {
        match config_string(instance, "operator", "and").as_str() {
            "or" => Self::Or,
            "xor" => Self::Xor,
            _ => Self::And,
        }
    }
}

#[derive(Debug)]
pub(super) struct BooleanOperationEval {
    pub(super) operator: BooleanOperator,
}

impl CompiledNodeEvaluator for BooleanOperationEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let values = evaluation
            .inputs
            .iter()
            .map(runtime_bool)
            .collect::<Result<Vec<_>, _>>()?;
        let result = match self.operator {
            BooleanOperator::And => values.into_iter().all(|value| value),
            BooleanOperator::Or => values.into_iter().any(|value| value),
            BooleanOperator::Xor => values.into_iter().filter(|value| *value).count() % 2 == 1,
        };
        Ok(vec![RuntimeValue::Bool(result)])
    }
}
