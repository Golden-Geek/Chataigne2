use crate::{CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

use super::support::{delta_seconds, float_inputs, set_state_values, state_values};

#[derive(Debug)]
pub(super) struct SpeedEval {
    pub(super) window_seconds: f64,
}

impl CompiledNodeEvaluator for SpeedEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let [input] = float_inputs::<1>(evaluation.inputs)?;
        let dt = delta_seconds(evaluation.ctx.delta_time);
        let state = evaluation.state.first_mut();
        let mut values = state_values(state.as_deref(), 3);
        let output = if values[2] == 0.0 {
            values[2] = 1.0;
            0.0
        } else {
            let instant = (input - values[0]) / dt;
            let alpha = if self.window_seconds <= dt {
                1.0
            } else {
                (dt / self.window_seconds).clamp(0.0, 1.0)
            };
            values[1] += (instant - values[1]) * alpha;
            values[1]
        };
        values[0] = input;
        set_state_values(state, values);
        Ok(vec![RuntimeValue::Float(output)])
    }
}
