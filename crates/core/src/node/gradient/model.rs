use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::color::Color;

/// Interpolation used between one gradient stop and the next.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default, TS)]
#[serde(rename_all = "camelCase")]
pub enum GradientInterpolation {
    /// Hold this stop's color until the next stop boundary (hard edge).
    None,
    /// Linear RGBA interpolation toward the next stop.
    #[default]
    Linear,
    /// Smoothstep-eased RGBA interpolation toward the next stop.
    Smooth,
}

impl GradientInterpolation {
    /// Returns the stable string id used by parameters and the UI protocol.
    #[must_use]
    pub const fn variant_id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Linear => "linear",
            Self::Smooth => "smooth",
        }
    }

    /// Parses one interpolation mode from its stable string id.
    #[must_use]
    pub fn from_variant_id(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "none" | "hold" | "step" => Self::None,
            "smooth" | "smoothstep" => Self::Smooth,
            _ => Self::Linear,
        }
    }

    /// Maps a normalized `[0, 1]` blend amount through this interpolation curve.
    #[must_use]
    pub fn apply(self, amount: f64) -> f64 {
        let amount = amount.clamp(0.0, 1.0);
        match self {
            Self::None => 0.0,
            Self::Linear => amount,
            Self::Smooth => amount * amount * (3.0 - 2.0 * amount),
        }
    }
}

/// One gradient color stop anchored at a normalized position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GradientStop {
    /// Stop position, normally in the `[0, 1]` range.
    pub position: f64,
    /// Stop color.
    pub color: Color,
    /// Interpolation used from this stop toward the next stop.
    pub interpolation: GradientInterpolation,
}

impl GradientStop {
    /// Creates one gradient stop.
    #[must_use]
    pub fn new(position: f64, color: Color, interpolation: GradientInterpolation) -> Self {
        Self {
            position,
            color,
            interpolation,
        }
    }
}

/// Sampled gradient model reconstructed from a gradient-node subtree.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Gradient {
    stops: Vec<GradientStop>,
}

impl Gradient {
    /// Creates one gradient from `stops`, sorted by ascending position.
    #[must_use]
    pub fn new(mut stops: Vec<GradientStop>) -> Self {
        stops.sort_by(|left, right| left.position.total_cmp(&right.position));
        Self { stops }
    }

    /// Returns the position-sorted stops.
    #[must_use]
    pub fn stops(&self) -> &[GradientStop] {
        &self.stops
    }

    /// Returns `true` when the gradient has no stops.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.stops.is_empty()
    }

    /// Samples the color at `position`, clamping to the end stops.
    ///
    /// Returns `None` only when the gradient has no stops.
    #[must_use]
    pub fn sample(&self, position: f64) -> Option<Color> {
        let first = self.stops.first()?;
        if !position.is_finite() || position <= first.position {
            return Some(first.color);
        }
        let last = self.stops.last().copied()?;
        if position >= last.position {
            return Some(last.color);
        }
        for pair in self.stops.windows(2) {
            let left = &pair[0];
            let right = &pair[1];
            if position > right.position {
                continue;
            }
            let span = (right.position - left.position).max(f64::EPSILON);
            let raw = ((position - left.position) / span).clamp(0.0, 1.0);
            let amount = left.interpolation.apply(raw);
            return Some(lerp_color(left.color, right.color, amount));
        }
        Some(last.color)
    }

    /// Samples the color at `position`, returning opaque black when empty.
    #[must_use]
    pub fn sample_or_black(&self, position: f64) -> Color {
        self.sample(position).unwrap_or(Color::new(0.0, 0.0, 0.0, 1.0))
    }
}

fn lerp_color(start: Color, end: Color, amount: f64) -> Color {
    Color::new(
        lerp(start.r(), end.r(), amount),
        lerp(start.g(), end.g(), amount),
        lerp(start.b(), end.b(), amount),
        lerp(start.a(), end.a(), amount),
    )
}

fn lerp(start: f64, end: f64, amount: f64) -> f64 {
    start + (end - start) * amount
}
