use crate::{CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

use super::support::numeric_map_checked;

#[derive(Debug)]
pub(super) struct NegateEval;

impl CompiledNodeEvaluator for NegateEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<crate::NodeOutputs, String> {
        let Some(value) = evaluation.inputs.first() else {
            return Err("numeric unary node expects one input".into());
        };
        let result = match value {
            RuntimeValue::Int(number) => RuntimeValue::Int(number.checked_neg().ok_or("Negate integer overflow")?),
            _ => numeric_map_checked(value, |number| Ok(-number))?,
        };
        Ok(crate::node_outputs![result])
    }
}
