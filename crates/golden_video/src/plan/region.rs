use crate::TextureId;

use super::SampleFilter;

/// Rectangle in normalized source or target coordinates. Execution validation requires finite
/// coordinates, positive extents and containment in the unit square.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct UnitRect {
    /// Left edge as a fraction of the source or target width.
    pub x: f32,
    /// Top edge as a fraction of the source or target height.
    pub y: f32,
    /// Positive horizontal fraction, contained within the unit square.
    pub width: f32,
    /// Positive vertical fraction, contained within the unit square.
    pub height: f32,
}

impl UnitRect {
    /// The complete source or target rectangle.
    pub const FULL: Self = Self {
        x: 0.0,
        y: 0.0,
        width: 1.0,
        height: 1.0,
    };

    /// Whether every coordinate is finite and the positive rectangle fits in the unit square.
    pub fn is_valid(self) -> bool {
        [self.x, self.y, self.width, self.height]
            .into_iter()
            .all(f32::is_finite)
            && self.x >= 0.0
            && self.y >= 0.0
            && self.width > 0.0
            && self.height > 0.0
            && self.x + self.width <= 1.0
            && self.y + self.height <= 1.0
    }
}

impl Default for UnitRect {
    fn default() -> Self {
        Self::FULL
    }
}

/// One replacement blit. Destination clipping is prepared before submission and accompanied by
/// the corresponding source crop, so clipping never stretches the remaining visible image.
#[derive(Clone, Debug)]
pub struct BlitRegion {
    /// Logical texture sampled by this blit.
    pub source: TextureId,
    /// Prepared visible rectangle within the target texture.
    pub destination: UnitRect,
    /// Source fraction corresponding to that visible destination.
    pub source_rect: UnitRect,
    /// Sampling used when source and destination extents differ.
    pub filter: SampleFilter,
    /// Optional destination lifetime. Withdrawn regions are skipped even while an older
    /// plan remains active; other regions in the same pass continue to draw.
    pub visibility: Option<golden_io::RevisionGuard>,
    /// Optional checked placement sampled by the GPU worker without preparing another plan.
    pub placement: Option<super::RegionPlacement>,
}

impl BlitRegion {
    #[cfg(any(test, all(windows, feature = "d3d11")))]
    pub(crate) fn for_frame(&self, token: crate::runtime::CompletionToken) -> Self {
        let mut sampled = self.clone();
        if let Some(placement) = &self.placement {
            let geometry = placement.sample(token);
            sampled.destination = geometry.destination;
            sampled.source_rect = geometry.source_rect;
        }
        sampled
    }
}
