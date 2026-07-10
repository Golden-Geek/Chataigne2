use std::cmp::Ordering;
use std::sync::Arc;

use crate::{ANodeInstance, ColorValue, CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

use super::support::{float_inputs, lerp};

/// Interpolation used between one gradient stop and the next.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(super) enum GradientInterpolation {
    /// Hold this stop's color until the next stop boundary (hard edge).
    None,
    /// Linear RGBA interpolation toward the next stop.
    #[default]
    Linear,
    /// Smoothstep-eased RGBA interpolation toward the next stop.
    Smooth,
}

impl GradientInterpolation {
    fn variant_id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Linear => "linear",
            Self::Smooth => "smooth",
        }
    }

    fn from_variant_id(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "none" | "hold" | "step" => Self::None,
            "smooth" | "smoothstep" => Self::Smooth,
            _ => Self::Linear,
        }
    }

    fn apply(self, amount: f64) -> f64 {
        let amount = amount.clamp(0.0, 1.0);
        match self {
            Self::None => 0.0,
            Self::Linear => amount,
            Self::Smooth => amount * amount * (3.0 - 2.0 * amount),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct GradientStop {
    position: f64,
    color: ColorValue,
    interpolation: GradientInterpolation,
}

#[derive(Debug)]
pub(super) struct GradientSamplerEval {
    pub(super) stops: Vec<GradientStop>,
}

impl CompiledNodeEvaluator for GradientSamplerEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let [position] = float_inputs::<1>(evaluation.inputs)?;
        Ok(vec![RuntimeValue::Color(sample_gradient(
            &self.stops,
            position.clamp(0.0, 1.0),
        ))])
    }
}

/// Builds the gradient sampler's position-sorted stops from its structured `gradient` config value.
///
/// The host materializes the config from a real gradient node subtree, so a missing or empty
/// value falls back to a default black-to-white ramp.
pub(super) fn stops_from_config(instance: &ANodeInstance) -> Vec<GradientStop> {
    let mut stops = instance
        .config
        .get("gradient")
        .map(stops_from_runtime_value)
        .unwrap_or_default();
    if stops.is_empty() {
        stops = default_stops();
    }
    stops.sort_by(|left, right| left.position.partial_cmp(&right.position).unwrap_or(Ordering::Equal));
    stops
}

/// Default structured gradient config value (black at 0, white at 1, linear).
pub(super) fn default_gradient_config() -> RuntimeValue {
    RuntimeValue::Array(default_stops().iter().map(stop_to_runtime_value).collect())
}

fn default_stops() -> Vec<GradientStop> {
    vec![
        GradientStop {
            position: 0.0,
            color: ColorValue::BLACK,
            interpolation: GradientInterpolation::Linear,
        },
        GradientStop {
            position: 1.0,
            color: ColorValue {
                red: 1.0,
                green: 1.0,
                blue: 1.0,
                alpha: 1.0,
            },
            interpolation: GradientInterpolation::Linear,
        },
    ]
}

fn stop_to_runtime_value(stop: &GradientStop) -> RuntimeValue {
    RuntimeValue::Array(vec![
        RuntimeValue::Float(stop.position),
        RuntimeValue::Color(stop.color),
        RuntimeValue::String(Arc::from(stop.interpolation.variant_id())),
    ])
}

fn stops_from_runtime_value(value: &RuntimeValue) -> Vec<GradientStop> {
    let RuntimeValue::Array(entries) = value else {
        return Vec::new();
    };
    entries.iter().filter_map(stop_from_runtime_value).collect()
}

fn stop_from_runtime_value(value: &RuntimeValue) -> Option<GradientStop> {
    let RuntimeValue::Array(fields) = value else {
        return None;
    };
    let position = fields.first()?.to_float_lossy();
    let color = fields.get(1)?.to_color_lossy();
    let interpolation = match fields.get(2) {
        Some(RuntimeValue::String(text)) => GradientInterpolation::from_variant_id(text),
        _ => GradientInterpolation::Linear,
    };
    Some(GradientStop {
        position: position.clamp(0.0, 1.0),
        color,
        interpolation,
    })
}

fn sample_gradient(stops: &[GradientStop], position: f64) -> ColorValue {
    let Some(first) = stops.first() else {
        return ColorValue::BLACK;
    };
    if position <= first.position {
        return first.color;
    }
    for pair in stops.windows(2) {
        let left = &pair[0];
        let right = &pair[1];
        if position > right.position {
            continue;
        }
        let width = (right.position - left.position).max(f64::EPSILON);
        let raw = ((position - left.position) / width).clamp(0.0, 1.0);
        let amount = left.interpolation.apply(raw);
        return ColorValue {
            red: lerp(f64::from(left.color.red), f64::from(right.color.red), amount) as f32,
            green: lerp(f64::from(left.color.green), f64::from(right.color.green), amount) as f32,
            blue: lerp(f64::from(left.color.blue), f64::from(right.color.blue), amount) as f32,
            alpha: lerp(f64::from(left.color.alpha), f64::from(right.color.alpha), amount) as f32,
        };
    }
    stops.last().map_or(ColorValue::BLACK, |stop| stop.color)
}
