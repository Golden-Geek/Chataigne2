use super::protocol::{SurfaceDefinition, SurfaceGeometryEdit, UnitRect};

impl SurfaceGeometryEdit {
    /// Only the cloned candidate changes before validation; callers commit it atomically.
    pub(super) fn apply(self, definition: &mut SurfaceDefinition) -> Result<(), String> {
        match self {
            Self::QuadCorner { corner, point } => {
                *definition
                    .quad
                    .corners
                    .get_mut(usize::from(corner))
                    .ok_or("corner index must be 0..3")? = point;
            }
            Self::CropCorner { corner, point } => {
                let crop = definition.crop;
                let right = crop.x + crop.width;
                let bottom = crop.y + crop.height;
                definition.crop = match corner {
                    0 => UnitRect {
                        x: point.x,
                        y: point.y,
                        width: right - point.x,
                        height: bottom - point.y,
                    },
                    1 => UnitRect {
                        x: crop.x,
                        y: point.y,
                        width: point.x - crop.x,
                        height: bottom - point.y,
                    },
                    2 => UnitRect {
                        x: crop.x,
                        y: crop.y,
                        width: point.x - crop.x,
                        height: point.y - crop.y,
                    },
                    3 => UnitRect {
                        x: point.x,
                        y: crop.y,
                        width: right - point.x,
                        height: point.y - crop.y,
                    },
                    _ => return Err("crop corner index must be 0..3".into()),
                };
            }
            Self::CropOrigin { point } => {
                definition.crop.x = point.x;
                definition.crop.y = point.y;
            }
        }
        definition.validate()
    }
}
