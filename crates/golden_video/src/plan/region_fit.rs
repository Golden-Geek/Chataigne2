use super::{RegionTile, UnitRect};
use crate::Extent2D;

/// Presentation of a complete image inside a passive tile's unclipped aperture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub enum RegionFit {
    /// Fill the aperture independently on each axis.
    Stretch,
    /// Preserve source aspect with centered empty margins, then apply host clipping.
    Contain,
}

impl RegionTile {
    /// Fit using the actual rendered source dimensions. The input source crop describes
    /// host clipping of a full-image aperture, not an authored image crop. Reconstruct that
    /// aperture before fitting so partially scrolled tiles retain the same scale and center.
    pub fn fitted(self, fit: RegionFit, source: Extent2D, target: Extent2D) -> Option<Self> {
        if fit == RegionFit::Stretch {
            return Some(self);
        }
        let d = self.destination;
        let s = self.source_rect;
        let width = f64::from(d.width) / f64::from(s.width);
        let height = f64::from(d.height) / f64::from(s.height);
        let left = f64::from(d.x) - f64::from(s.x) * width;
        let top = f64::from(d.y) - f64::from(s.y) * height;
        let scale = (width * f64::from(target.width()) / f64::from(source.width()))
            .min(height * f64::from(target.height()) / f64::from(source.height()));
        let fitted_width = f64::from(source.width()) * scale / f64::from(target.width());
        let fitted_height = f64::from(source.height()) * scale / f64::from(target.height());
        let fitted_left = left + (width - fitted_width) * 0.5;
        let fitted_top = top + (height - fitted_height) * 0.5;
        let x0 = fitted_left.max(f64::from(d.x));
        let y0 = fitted_top.max(f64::from(d.y));
        let x1 = (fitted_left + fitted_width).min(f64::from(d.x) + f64::from(d.width));
        let y1 = (fitted_top + fitted_height).min(f64::from(d.y) + f64::from(d.height));
        Some(Self {
            destination: rect(x0, y0, x1, y1)?,
            source_rect: rect(
                (x0 - fitted_left) / fitted_width,
                (y0 - fitted_top) / fitted_height,
                (x1 - fitted_left) / fitted_width,
                (y1 - fitted_top) / fitted_height,
            )?,
        })
    }
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Option<UnitRect> {
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let x = x0.clamp(0., 1.) as f32;
    let y = y0.clamp(0., 1.) as f32;
    let rect = UnitRect {
        x,
        y,
        width: ((x1.clamp(0., 1.) as f32) - x).min(1. - x),
        height: ((y1.clamp(0., 1.) as f32) - y).min(1. - y),
    };
    rect.is_valid().then_some(rect)
}
