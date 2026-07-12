use std::cmp::Ordering;

use crate::{ANodeInstance, CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

use super::support::{
    config_float, config_int, config_string, delta_seconds, float_inputs, set_state_values, smoothing_alpha,
    state_values, trim_history,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SmoothMethod {
    OneEuro,
    Sma,
    Damping,
    SavitzkyGolay,
    Median,
}

#[derive(Debug)]
pub(super) struct SmoothFilterEval {
    method: SmoothMethod,
    window: usize,
    min_cutoff: f64,
    beta: f64,
    mass: f64,
    friction: f64,
}

impl SmoothFilterEval {
    pub(super) fn from_config(instance: &ANodeInstance) -> Self {
        let method = match config_string(instance, "method", "one_euro").as_str() {
            "sma" => SmoothMethod::Sma,
            "damping" => SmoothMethod::Damping,
            "savitzky_golay" => SmoothMethod::SavitzkyGolay,
            "median" => SmoothMethod::Median,
            _ => SmoothMethod::OneEuro,
        };
        Self {
            method,
            window: config_int(instance, "window", 5).clamp(1, 128) as usize,
            min_cutoff: config_float(instance, "min_cutoff", 1.0).max(0.0),
            beta: config_float(instance, "beta", 0.0).max(0.0),
            mass: config_float(instance, "mass", 1.0).max(0.001),
            friction: config_float(instance, "friction", 8.0).max(0.0),
        }
    }
}

impl CompiledNodeEvaluator for SmoothFilterEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let [input] = float_inputs::<1>(evaluation.inputs)?;
        let dt = delta_seconds(evaluation.ctx.delta_time);
        let state = evaluation.state.first_mut();
        let output = match self.method {
            SmoothMethod::OneEuro => {
                let mut values = state_values(state.as_deref(), 2);
                let derivative = (input - values[0]) / dt;
                let cutoff = self.min_cutoff + self.beta * derivative.abs();
                let alpha = smoothing_alpha(cutoff, dt);
                values[0] = input;
                values[1] += (input - values[1]) * alpha;
                let output = values[1];
                set_state_values(state, values);
                output
            }
            SmoothMethod::Sma => {
                let mut history = state_values(state.as_deref(), 0);
                history.push(input);
                trim_history(&mut history, self.window);
                let output = history.iter().sum::<f64>() / history.len() as f64;
                set_state_values(state, history);
                output
            }
            SmoothMethod::Damping => {
                let mut values = state_values(state.as_deref(), 2);
                let acceleration = (input - values[0]) / self.mass - values[1] * self.friction;
                values[1] += acceleration * dt;
                values[0] += values[1] * dt;
                let output = values[0];
                set_state_values(state, values);
                output
            }
            SmoothMethod::SavitzkyGolay => {
                let mut history = state_values(state.as_deref(), 0);
                history.push(input);
                trim_history(&mut history, self.window.max(5));
                let output = if history.len() >= 5 {
                    let start = history.len() - 5;
                    (-3.0 * history[start]
                        + 12.0 * history[start + 1]
                        + 17.0 * history[start + 2]
                        + 12.0 * history[start + 3]
                        - 3.0 * history[start + 4])
                        / 35.0
                } else {
                    history.iter().sum::<f64>() / history.len() as f64
                };
                set_state_values(state, history);
                output
            }
            SmoothMethod::Median => {
                let mut history = state_values(state.as_deref(), 0);
                history.push(input);
                trim_history(&mut history, self.window);
                let mut sorted = history.clone();
                sorted.sort_by(|left, right| left.partial_cmp(right).unwrap_or(Ordering::Equal));
                let midpoint = sorted.len() / 2;
                let output = if sorted.len() % 2 == 0 {
                    (sorted[midpoint - 1] + sorted[midpoint]) * 0.5
                } else {
                    sorted[midpoint]
                };
                set_state_values(state, history);
                output
            }
        };
        Ok(vec![RuntimeValue::Float(output)])
    }
}
