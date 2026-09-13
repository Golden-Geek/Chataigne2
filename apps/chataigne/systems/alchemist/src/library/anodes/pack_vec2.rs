use crate::{CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

use super::support::float_inputs;

#[derive(Debug)]
pub(super) struct PackVec2Eval;

impl CompiledNodeEvaluator for PackVec2Eval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<crate::NodeOutputs, String> {
        let [x, y] = float_inputs::<2>(evaluation.inputs)?;
        Ok(crate::node_outputs![RuntimeValue::Vec2([x, y])])
    }
}
