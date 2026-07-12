use std::sync::Arc;

use crate::{CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

#[derive(Debug)]
pub(super) struct SplitEval {
    pub(super) separator: String,
    pub(super) trim: bool,
    pub(super) omit_empty: bool,
}

impl CompiledNodeEvaluator for SplitEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let Some(RuntimeValue::String(value)) = evaluation.inputs.first() else {
            return Err("Split expects a string input".into());
        };
        let parts: Vec<String> = if self.separator.is_empty() {
            value.chars().map(|character| character.to_string()).collect()
        } else {
            value.split(&self.separator).map(ToOwned::to_owned).collect()
        };
        let values = parts
            .into_iter()
            .map(|part| if self.trim { part.trim().to_owned() } else { part })
            .filter(|part| !self.omit_empty || !part.is_empty())
            .map(|part| RuntimeValue::String(Arc::from(part)))
            .collect();
        Ok(vec![RuntimeValue::Array(values)])
    }
}
