use crate::{CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

#[derive(Debug)]
pub(super) struct ExtractVec2Eval;

impl CompiledNodeEvaluator for ExtractVec2Eval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<crate::NodeOutputs, String> {
        let Some(RuntimeValue::Vec2(components)) = evaluation.inputs.first() else {
            return Err("Extract Vec2 expects a Vec2 input".into());
        };
        Ok(components.iter().copied().map(RuntimeValue::Float).collect())
    }
}
