use crate::{CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

use super::support::float_inputs;

#[derive(Debug)]
pub(super) struct RemapEval;

impl CompiledNodeEvaluator for RemapEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let [value, in_min, in_max, out_min, out_max] = float_inputs::<5>(evaluation.inputs)?;
        if (in_max - in_min).abs() <= f64::EPSILON {
            return Err("Remap input range cannot be zero".into());
        }
        let normalized = (value - in_min) / (in_max - in_min);
        Ok(vec![RuntimeValue::Float(out_min + normalized * (out_max - out_min))])
    }
}
