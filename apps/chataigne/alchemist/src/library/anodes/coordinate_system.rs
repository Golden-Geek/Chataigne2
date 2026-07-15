use crate::{ANodeInstance, CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

use super::support::config_string;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CoordinateMode {
    CartesianToPolar,
    PolarToCartesian,
}

impl CoordinateMode {
    pub(super) fn from_config(instance: &ANodeInstance) -> Self {
        match config_string(instance, "mode", "cartesian_to_polar").as_str() {
            "polar_to_cartesian" => Self::PolarToCartesian,
            _ => Self::CartesianToPolar,
        }
    }
}

#[derive(Debug)]
pub(super) struct CoordinateSystemEval {
    pub(super) mode: CoordinateMode,
}

impl CompiledNodeEvaluator for CoordinateSystemEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let Some(RuntimeValue::Vec2(value)) = evaluation.inputs.first() else {
            return Err("Coordinate System expects a vec2 input".into());
        };
        let result = match self.mode {
            CoordinateMode::CartesianToPolar => {
                let radius = value[0].hypot(value[1]);
                let angle = value[1].atan2(value[0]);
                [radius, angle]
            }
            CoordinateMode::PolarToCartesian => {
                let radius = value[0];
                let angle = value[1];
                [radius * angle.cos(), radius * angle.sin()]
            }
        };
        Ok(vec![RuntimeValue::Vec2(result)])
    }
}
