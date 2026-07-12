use crate::{CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

use super::support::{require_inputs, set_state_values, state_values, trigger_fired, value_to_f64};

#[derive(Debug)]
pub(super) struct CounterEval;

impl CompiledNodeEvaluator for CounterEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let [add, amount, reset] = require_inputs::<3>(evaluation.inputs)?;
        let state = evaluation.state.first_mut();
        let mut values = state_values(state.as_deref(), 1);
        if trigger_fired(reset)? {
            values[0] = 0.0;
        } else if trigger_fired(add)? {
            values[0] += value_to_f64(amount);
        }
        let output = values[0];
        set_state_values(state, values);
        Ok(vec![RuntimeValue::Float(output)])
    }
}
