use crate::{ANodeInstance, CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

use super::support::{config_string, delta_seconds, set_state_values, state_values};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LfoShape {
    Sine,
    Triangle,
    Saw,
    Square,
    Pulse,
}

impl LfoShape {
    pub(super) fn from_config(instance: &ANodeInstance) -> Self {
        match config_string(instance, "shape", "sine").as_str() {
            "triangle" => Self::Triangle,
            "saw" => Self::Saw,
            "square" => Self::Square,
            "pulse" => Self::Pulse,
            _ => Self::Sine,
        }
    }

    fn sample(self, phase: f64) -> f64 {
        match self {
            Self::Sine => (phase * std::f64::consts::TAU).sin() * 0.5 + 0.5,
            Self::Triangle => {
                if phase < 0.5 {
                    phase * 2.0
                } else {
                    (1.0 - phase) * 2.0
                }
            }
            Self::Saw => phase,
            Self::Square => f64::from(phase < 0.5),
            Self::Pulse => f64::from(phase < 0.1),
        }
    }
}

#[derive(Debug)]
pub(super) struct LfoEval {
    pub(super) shape: LfoShape,
    pub(super) frequency: f64,
    pub(super) update_rate: f64,
    pub(super) minimum: f64,
    pub(super) maximum: f64,
}

impl CompiledNodeEvaluator for LfoEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let dt = delta_seconds(evaluation.ctx.delta_time);
        let state = evaluation.state.first_mut();
        let mut values = state_values(state.as_deref(), 4);
        values[2] += dt;
        let interval = if self.update_rate <= 0.0 {
            0.0
        } else {
            1.0 / self.update_rate
        };
        if values[3] == 0.0 || interval == 0.0 || values[2] >= interval {
            values[0] = (values[0] + self.frequency * dt).rem_euclid(1.0);
            values[1] = self.minimum + self.shape.sample(values[0]) * (self.maximum - self.minimum);
            values[2] = 0.0;
            values[3] = 1.0;
        }
        let output = values[1];
        set_state_values(state, values);
        Ok(vec![RuntimeValue::Float(output)])
    }
}
