use crate::{CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

#[derive(Debug)]
pub(super) struct ExtractVec3Eval;

impl CompiledNodeEvaluator for ExtractVec3Eval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<crate::NodeOutputs, String> {
        let Some(RuntimeValue::Vec3(components)) = evaluation.inputs.first() else {
            return Err("Extract Vec3 expects a Vec3 input".into());
        };
        Ok(components.iter().copied().map(RuntimeValue::Float).collect())
    }
}
