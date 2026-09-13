use std::sync::Arc;

use crate::{CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

use super::support::format_runtime_value;

#[derive(Debug)]
pub(super) struct ConcatenateEval {
    pub(super) prefix: String,
    pub(super) suffix: String,
    pub(super) separator: String,
}

impl CompiledNodeEvaluator for ConcatenateEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<crate::NodeOutputs, String> {
        let body = evaluation
            .inputs
            .iter()
            .map(|value| format_runtime_value(value, 3))
            .collect::<Vec<_>>()
            .join(&self.separator);
        Ok(crate::node_outputs![RuntimeValue::String(Arc::from(format!(
            "{}{}{}",
            self.prefix, body, self.suffix
        )))])
    }
}
