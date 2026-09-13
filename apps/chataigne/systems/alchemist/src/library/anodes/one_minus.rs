use crate::{CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

use super::support::numeric_map_checked;

#[derive(Debug)]
pub(super) struct OneMinusEval;

impl CompiledNodeEvaluator for OneMinusEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let Some(value) = evaluation.inputs.first() else {
            return Err("numeric unary node expects one input".into());
        };
        let result = match value {
            RuntimeValue::Int(number) => {
                RuntimeValue::Int(1_i64.checked_sub(*number).ok_or("One Minus integer overflow")?)
            }
            _ => numeric_map_checked(value, |number| Ok(1.0 - number))?,
        };
        Ok(vec![result])
    }
}
