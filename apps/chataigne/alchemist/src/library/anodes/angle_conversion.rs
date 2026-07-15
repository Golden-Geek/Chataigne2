use crate::{ANodeInstance, CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

use super::support::{config_string, float_inputs};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AngleMode {
    DegreesToRadians,
    RadiansToDegrees,
}

impl AngleMode {
    pub(super) fn from_config(instance: &ANodeInstance) -> Self {
        match config_string(instance, "mode", "degrees_to_radians").as_str() {
            "radians_to_degrees" => Self::RadiansToDegrees,
            _ => Self::DegreesToRadians,
        }
    }
}

#[derive(Debug)]
pub(super) struct AngleConversionEval {
    pub(super) mode: AngleMode,
}

impl CompiledNodeEvaluator for AngleConversionEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let [value] = float_inputs::<1>(evaluation.inputs)?;
        let result = match self.mode {
            AngleMode::DegreesToRadians => value.to_radians(),
            AngleMode::RadiansToDegrees => value.to_degrees(),
        };
        Ok(vec![RuntimeValue::Float(result)])
    }
}
