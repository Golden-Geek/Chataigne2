use crate::{CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

use super::support::float_inputs;

#[derive(Debug)]
pub(super) struct PackVec3Eval;

impl CompiledNodeEvaluator for PackVec3Eval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<crate::NodeOutputs, String> {
        let [x, y, z] = float_inputs::<3>(evaluation.inputs)?;
        Ok(crate::node_outputs![RuntimeValue::Vec3([x, y, z])])
    }
}
