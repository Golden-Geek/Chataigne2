use crate::{CompiledNodeEvaluator, NodeEvaluation};

use super::convert_scalar::{ScalarTarget, convert};

#[derive(Debug)]
pub(super) struct ConvertTupleEval {
    pub(super) target: ScalarTarget,
}

impl CompiledNodeEvaluator for ConvertTupleEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<crate::NodeOutputs, String> {
        evaluation
            .inputs
            .iter()
            .map(|value| convert(value, self.target))
            .collect()
    }
}
