//! Pixel-aligned partitions for supersampling within a fixed texture-dimension budget.
use super::{MappingError, ProjectiveMapping, UnitRect};
use crate::Extent2D;

/// A logical output tile. Resolving integer sample groups never crosses its edges.
#[derive(Clone, Copy, Debug)]
pub struct RasterTile {
    /// Top-left logical pixel.
    pub origin: [u32; 2],
    /// Logical pixel extent before supersampling.
    pub extent: Extent2D,
}

impl RasterTile {
    /// Checked integer-density raster extent.
    pub fn raster_extent(self, scale: u32) -> Option<Extent2D> {
        Extent2D::new(
            self.extent.width().checked_mul(scale)?,
            self.extent.height().checked_mul(scale)?,
        )
        .ok()
    }

    /// Normalized position within the complete logical output.
    pub fn viewport(self, output: Extent2D) -> UnitRect {
        let x = self.origin[0] as f32 / output.width() as f32;
        let y = self.origin[1] as f32 / output.height() as f32;
        UnitRect {
            x,
            y,
            width: (self.origin[0] + self.extent.width()) as f32 / output.width() as f32 - x,
            height: (self.origin[1] + self.extent.height()) as f32 / output.height() as f32 - y,
        }
    }
}

/// Split into uniformly bounded, nonempty integer tiles, rejecting excessive allocation first.
pub fn supersample_tiles(
    output: Extent2D,
    scale: u32,
    dimension: u32,
    maximum_tiles: usize,
) -> Option<Vec<RasterTile>> {
    if scale == 0 || output.width() > dimension || output.height() > dimension {
        return None;
    }
    let capacity = dimension.checked_div(scale).filter(|&value| value > 0)?;
    let columns = output.width().div_ceil(capacity);
    let rows = output.height().div_ceil(capacity);
    let count = usize::try_from(u64::from(columns) * u64::from(rows)).ok()?;
    if count > maximum_tiles {
        return None;
    }
    let mut tiles = Vec::with_capacity(count);
    for row in 0..rows {
        for column in 0..columns {
            let left = (u64::from(output.width()) * u64::from(column) / u64::from(columns)) as u32;
            let right = (u64::from(output.width()) * u64::from(column + 1) / u64::from(columns)) as u32;
            let top = (u64::from(output.height()) * u64::from(row) / u64::from(rows)) as u32;
            let bottom = (u64::from(output.height()) * u64::from(row + 1) / u64::from(rows)) as u32;
            tiles.push(RasterTile {
                origin: [left, top],
                extent: Extent2D::new(right - left, bottom - top).ok()?,
            });
        }
    }
    Some(tiles)
}

impl ProjectiveMapping {
    /// Map tile-local coordinates through the original full-output homography.
    pub fn in_viewport(self, viewport: UnitRect) -> Result<Self, MappingError> {
        if !viewport.is_valid() {
            return Err(MappingError);
        }
        let mut inverse = self.inverse_rows();
        for row in &mut inverse {
            row[2] += row[0] * viewport.x + row[1] * viewport.y;
            row[0] *= viewport.width;
            row[1] *= viewport.height;
        }
        Self::from_viewport_rows(self.crop(), inverse)
    }
}

#[cfg(test)]
mod tests;
