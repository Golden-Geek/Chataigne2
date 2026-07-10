use crate::{CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

use super::support::{rgba_to_cmyk, rgba_to_hsla, rgba_to_hsva};

#[derive(Debug)]
pub(super) struct ExtractColorEval {
    pub(super) mode: super::color_mode::ColorMode,
}

impl CompiledNodeEvaluator for ExtractColorEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let Some(RuntimeValue::Color(color)) = evaluation.inputs.first() else {
            return Err("Extract Color expects a color input".into());
        };
        let channels = match self.mode {
            super::color_mode::ColorMode::Rgba => [
                f64::from(color.red),
                f64::from(color.green),
                f64::from(color.blue),
                f64::from(color.alpha),
            ],
            super::color_mode::ColorMode::Hsva => rgba_to_hsva(*color),
            super::color_mode::ColorMode::Hsla => rgba_to_hsla(*color),
            super::color_mode::ColorMode::Cmyk => rgba_to_cmyk(*color),
        };
        Ok(channels.into_iter().map(RuntimeValue::Float).collect())
    }
}
