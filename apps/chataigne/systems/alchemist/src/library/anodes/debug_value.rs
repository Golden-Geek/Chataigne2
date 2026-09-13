use crate::{CompiledNodeEvaluator, NodeEvaluation};

use super::support::require_inputs;

#[derive(Debug)]
pub(super) struct DebugValueEval;

impl CompiledNodeEvaluator for DebugValueEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<crate::NodeOutputs, String> {
        let [value] = require_inputs::<1>(evaluation.inputs)?;
        Ok(crate::node_outputs![value.clone()])
    }
}
