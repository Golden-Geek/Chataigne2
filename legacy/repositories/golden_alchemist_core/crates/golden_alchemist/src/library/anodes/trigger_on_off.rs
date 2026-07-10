use crate::{CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

use super::support::{bool_inputs, set_state_values, state_values, trigger};

#[derive(Debug)]
pub(super) struct TriggerOnOffEval {
    pub(super) toggle: bool,
}

impl CompiledNodeEvaluator for TriggerOnOffEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let [value] = bool_inputs::<1>(evaluation.inputs)?;
        let state = evaluation.state.first_mut();
        let mut values = state_values(state.as_deref(), 2);
        let previous = values[0] != 0.0;
        let rising = value && !previous;
        let falling = !value && previous;
        let mut on = false;
        let mut off = false;
        if self.toggle {
            if rising {
                let toggled_on = values[1] == 0.0;
                values[1] = f64::from(toggled_on);
                on = toggled_on;
                off = !toggled_on;
            }
        } else {
            on = rising;
            off = falling;
        }
        values[0] = f64::from(value);
        set_state_values(state, values);
        let edge_id = u64::from(evaluation.exec_node.index() as u32);
        Ok(vec![
            RuntimeValue::Trigger(trigger(on, edge_id, evaluation.ctx.logical_tick)),
            RuntimeValue::Trigger(trigger(off, edge_id, evaluation.ctx.logical_tick)),
        ])
    }
}
