use crate::{CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

use super::support::numeric_map;

#[derive(Debug)]
pub(super) struct OneMinusEval;

impl CompiledNodeEvaluator for OneMinusEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let Some(value) = evaluation.inputs.first() else {
            return Err("numeric unary node expects one input".into());
        };
        Ok(vec![numeric_map(value, |value| 1.0 - value)])
    }
}
