use crate::{CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

use super::support::require_inputs;

#[derive(Debug)]
pub(super) struct DebugValueEval;

impl CompiledNodeEvaluator for DebugValueEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let [value] = require_inputs::<1>(evaluation.inputs)?;
        Ok(vec![value.clone()])
    }
}
