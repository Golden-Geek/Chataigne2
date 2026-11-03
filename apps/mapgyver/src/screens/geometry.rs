use super::protocol::{SurfaceDefinition, SurfaceGeometryEdit, UnitRect};

impl SurfaceGeometryEdit {
    /// Only the cloned candidate changes before validation; callers commit it atomically.
    pub(super) fn apply(
        self,
        definition: &mut SurfaceDefinition,
        extent: Option<golden_video::Extent2D>,
    ) -> Result<(), String> {
        match self {
            Self::QuadCornerPixels { corner, origin, pixels } => {
                let extent = extent.ok_or("output pixel extent is unavailable")?;
                let point = definition
                    .quad
                    .corners
                    .get_mut(usize::from(corner))
                    .ok_or("corner index must be 0..3")?;
                *point = super::protocol::Point {
                    x: origin.x + pixels.x as f32 / extent.width() as f32,
                    y: origin.y + pixels.y as f32 / extent.height() as f32,
                };
            }
            Self::QuadCorner { corner, point } => {
                *definition
                    .quad
                    .corners
                    .get_mut(usize::from(corner))
                    .ok_or("corner index must be 0..3")? = point;
            }
            Self::CropCorner { corner, point } => {
                if !point.x.is_finite() || !point.y.is_finite() {
                    return Err("crop positions must be finite".into());
                }
                let crop = definition.crop;
                let right = crop.x + crop.width;
                let bottom = crop.y + crop.height;
                // Keep the opposite corner fixed. Adjacent representable coordinates
                // stop a dragged corner at the other edge without producing a zero extent.
                let left = point.x.clamp(0.0, right.next_down());
                let top = point.y.clamp(0.0, bottom.next_down());
                let right = point.x.clamp(crop.x.next_up().min(1.0), 1.0);
                let bottom = point.y.clamp(crop.y.next_up().min(1.0), 1.0);
                definition.crop = match corner {
                    0 => UnitRect {
                        x: left,
                        y: top,
                        width: crop.x + crop.width - left,
                        height: crop.y + crop.height - top,
                    },
                    1 => UnitRect {
                        x: crop.x,
                        y: top,
                        width: right - crop.x,
                        height: crop.y + crop.height - top,
                    },
                    2 => UnitRect {
                        x: crop.x,
                        y: crop.y,
                        width: right - crop.x,
                        height: bottom - crop.y,
                    },
                    3 => UnitRect {
                        x: left,
                        y: crop.y,
                        width: crop.x + crop.width - left,
                        height: bottom - crop.y,
                    },
                    _ => return Err("crop corner index must be 0..3".into()),
                };
            }
            Self::CropOrigin { point } => {
                if !point.x.is_finite() || !point.y.is_finite() {
                    return Err("crop positions must be finite".into());
                }
                definition.crop.x = super::surface_node::bounded_crop_origin(point.x, definition.crop.width);
                definition.crop.y = super::surface_node::bounded_crop_origin(point.y, definition.crop.height);
            }
        }
        definition.validate()
    }
}
