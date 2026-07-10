use crate::{ColorValue, CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

use super::support::{cmyk_to_rgba, float_inputs, hsla_to_rgba, hsva_to_rgba};

#[derive(Debug)]
pub(super) struct ConvertToColorEval {
    pub(super) mode: super::color_mode::ColorMode,
}

impl CompiledNodeEvaluator for ConvertToColorEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let [first, second, third, fourth] = float_inputs::<4>(evaluation.inputs)?;
        let color = match self.mode {
            super::color_mode::ColorMode::Rgba => ColorValue {
                red: first as f32,
                green: second as f32,
                blue: third as f32,
                alpha: fourth as f32,
            },
            super::color_mode::ColorMode::Hsva => hsva_to_rgba(first, second, third, fourth),
            super::color_mode::ColorMode::Hsla => hsla_to_rgba(first, second, third, fourth),
            super::color_mode::ColorMode::Cmyk => cmyk_to_rgba(first, second, third, fourth),
        };
        Ok(vec![RuntimeValue::Color(color)])
    }
}
