use super::protocol::{RectGeometryEdit, RectGeometryMode, SpatialRect, SpatialResizeHandle};

impl SpatialResizeHandle {
    pub(crate) fn position(self) -> [f64; 2] {
        match self {
            Self::TopLeft => [0., 0.],
            Self::Top => [0.5, 0.],
            Self::TopRight => [1., 0.],
            Self::Right => [1., 0.5],
            Self::BottomRight => [1., 1.],
            Self::Bottom => [0.5, 1.],
            Self::BottomLeft => [0., 1.],
            Self::Left => [0., 0.5],
        }
    }
}

impl RectGeometryEdit {
    pub(crate) fn apply(self) -> Result<SpatialRect, String> {
        let mut position = self.origin.position.map(f64::from);
        let mut dimensions = [self.origin.size.width(), self.origin.size.height()].map(f64::from);
        if self.origin.position.into_iter().any(|value| !value.is_finite())
            || dimensions.into_iter().any(|value| value > 16384.0)
            || [self.start.x, self.start.y, self.point.x, self.point.y]
                .into_iter()
                .any(|value| !value.is_finite())
        {
            return Err("geometry requires finite coordinates and dimensions in 1..16384".into());
        }
        let delta = [
            f64::from(self.point.x) - f64::from(self.start.x),
            f64::from(self.point.y) - f64::from(self.start.y),
        ];
        match self.mode {
            RectGeometryMode::Translate => {
                for axis in 0..2 {
                    position[axis] += delta[axis];
                }
            }
            RectGeometryMode::Resize { handle } => {
                let handle = handle.position();
                for axis in 0..2 {
                    if handle[axis] == 0.5 {
                        continue;
                    }
                    let size = (dimensions[axis] + delta[axis] * (2.0 * handle[axis] - 1.0)).round();
                    if !(1.0..=16384.0).contains(&size) {
                        return Err(
                            "resize dimensions must stay in 1..16384; crossing an opposite edge is invalid".into(),
                        );
                    }
                    if handle[axis] == 0.0 {
                        position[axis] += dimensions[axis] - size;
                    }
                    dimensions[axis] = size;
                }
            }
        }
        let position = position.map(|value| value as f32);
        if position.into_iter().any(|value| !value.is_finite()) {
            return Err("geometry translation overflow".into());
        }
        Ok(SpatialRect {
            position,
            size: golden_video::Extent2D::new(dimensions[0] as u32, dimensions[1] as u32)
                .map_err(|error| error.to_string())?,
        })
    }
}
