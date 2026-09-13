use crate::{CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

use super::support::float_inputs;

#[derive(Debug)]
pub(super) struct PackVec2Eval;

impl CompiledNodeEvaluator for PackVec2Eval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let [x, y] = float_inputs::<2>(evaluation.inputs)?;
        Ok(vec![RuntimeValue::Vec2([x, y])])
    }
}
