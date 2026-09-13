use crate::{ANodeInstance, CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

use super::support::{config_string, float_inputs};

#[derive(Debug)]
pub(super) struct ThresholdEval {
    below: bool,
}

impl ThresholdEval {
    pub(super) fn from_config(instance: &ANodeInstance) -> Self {
        Self {
            below: config_string(instance, "direction", "above") == "below",
        }
    }
}

impl CompiledNodeEvaluator for ThresholdEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let [value, threshold, hysteresis] = float_inputs::<3>(evaluation.inputs)?;
        if !threshold.is_finite() || !hysteresis.is_finite() || hysteresis < 0.0 {
            return Err("Threshold requires a finite threshold and nonnegative finite hysteresis".into());
        }
        let half_hysteresis = hysteresis * 0.5;
        if !(threshold + half_hysteresis).is_finite() || !(threshold - half_hysteresis).is_finite() {
            return Err("Threshold hysteresis boundaries exceed the finite float range".into());
        }
        let was_active = matches!(evaluation.state.first(), Some(RuntimeValue::Bool(true)));
        let active = if self.below {
            let boundary = if was_active {
                threshold + half_hysteresis
            } else {
                threshold - half_hysteresis
            };
            value <= boundary
        } else {
            let boundary = if was_active {
                threshold - half_hysteresis
            } else {
                threshold + half_hysteresis
            };
            value >= boundary
        };
        if let Some(state) = evaluation.state.first_mut() {
            *state = RuntimeValue::Bool(active);
        }
        Ok(vec![RuntimeValue::Bool(active)])
    }
}
