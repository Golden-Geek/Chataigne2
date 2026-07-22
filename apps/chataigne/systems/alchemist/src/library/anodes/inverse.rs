use crate::{CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

use super::support::numeric_map_checked;

#[derive(Debug)]
pub(super) struct InverseEval;

impl CompiledNodeEvaluator for InverseEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let Some(value) = evaluation.inputs.first() else {
            return Err("numeric unary node expects one input".into());
        };
        Ok(vec![numeric_map_checked(value, |value| {
            if value.abs() <= f64::EPSILON {
                Err("Inverse input cannot be zero".into())
            } else {
                Ok(1.0 / value)
            }
        })?])
    }
}
