//! Bounded presentation geometry. Coordinates are relative to the complete source image;
//! widths/radii use screen units whose physical scale is authenticated by the preview host.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Maximum retained presentation primitives per independently placed view.
pub const MAX_VIEW_OVERLAYS: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
/// App-neutral editor presentation primitives, never authored document state.
pub enum OverlayShape {
    /// Closed four-sided path, including unrestricted finite off-image coordinates.
    Quad {
        /// Ordered source-image vertices.
        corners: [[f32; 2]; 4],
    },
    /// A screen-sized circular handle centered in image coordinates.
    Circle {
        /// Source-image center.
        center: [f32; 2],
        /// Radius in screen units.
        radius: f32,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
/// One filled/stroked primitive. Colors are straight-alpha sRGB components in 0..=1.
pub struct OverlayPrimitive {
    /// Optional opaque local input identity; absent for noninteractive decoration.
    pub hit: Option<String>,
    /// Image-relative path or screen-sized handle.
    pub shape: OverlayShape,
    /// Interior color; zero alpha leaves the image unchanged.
    pub fill: [f32; 4],
    /// Outline color.
    pub stroke: [f32; 4],
    /// Nonnegative screen-unit stroke width.
    pub width: f32,
    /// Optional positive screen-unit dash and gap lengths.
    pub dash: Option<[f32; 2]>,
}

impl OverlayPrimitive {
    /// Validate bounded metadata before retaining it or submitting GPU work.
    pub fn is_valid(&self) -> bool {
        let point = |point: &[f32; 2]| point.iter().all(|value| value.is_finite() && value.abs() <= 1_000_000.);
        let shape = match &self.shape {
            OverlayShape::Quad { corners } => corners.iter().all(point),
            OverlayShape::Circle { center, radius } => {
                point(center) && radius.is_finite() && (0.0..=128.0).contains(radius)
            }
        };
        shape
            && self
                .hit
                .as_ref()
                .is_none_or(|id| !id.is_empty() && id.len() <= 128 && !id.contains('\0'))
            && self
                .fill
                .iter()
                .chain(&self.stroke)
                .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
            && self.width.is_finite()
            && (0.0..=64.0).contains(&self.width)
            && self.dash.is_none_or(|dash| {
                dash.iter()
                    .all(|value| value.is_finite() && *value > 0.0 && *value <= 256.0)
            })
    }
}
