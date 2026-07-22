use crate::{CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

use super::support::float_inputs;

#[derive(Debug)]
pub(super) struct PackVec3Eval;

impl CompiledNodeEvaluator for PackVec3Eval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let [x, y, z] = float_inputs::<3>(evaluation.inputs)?;
        Ok(vec![RuntimeValue::Vec3([x, y, z])])
    }
}
