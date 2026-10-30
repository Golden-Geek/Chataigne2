use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Normalized source crop, applied before the destination perspective mapping.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct Crop {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Default for Crop {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        }
    }
}

impl Crop {
    pub fn validate(self) -> Result<(), String> {
        if !(golden_video::plan::UnitRect {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
        })
        .is_valid()
        {
            return Err("crop must have finite positive extents contained in the source unit square".into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct Quad {
    pub corners: [Point; 4],
}

impl Default for Quad {
    fn default() -> Self {
        Self {
            corners: [
                Point { x: 0.0, y: 0.0 },
                Point { x: 1.0, y: 0.0 },
                Point { x: 1.0, y: 1.0 },
                Point { x: 0.0, y: 1.0 },
            ],
        }
    }
}

impl Quad {
    pub fn validate(self) -> Result<(), String> {
        if self
            .corners
            .iter()
            .any(|point| !point.x.is_finite() || !point.y.is_finite())
        {
            return Err("quad coordinates must be finite".into());
        }
        // Consistent strict turns reject crossed, concave and collapsed quads. Off-screen
        // corners and mirrored winding are valid; output clipping belongs to rendering.
        let mut sign = 0.0_f64;
        for index in 0..4 {
            let [a, b, c] = [
                self.corners[index],
                self.corners[(index + 1) % 4],
                self.corners[(index + 2) % 4],
            ];
            let cross = (f64::from(b.x) - f64::from(a.x)) * (f64::from(c.y) - f64::from(b.y))
                - (f64::from(b.y) - f64::from(a.y)) * (f64::from(c.x) - f64::from(b.x));
            if cross.abs() < 1e-8 || (sign != 0.0 && cross.signum() != sign) {
                return Err("quad must be convex, uncrossed and nondegenerate".into());
            }
            sign = cross.signum();
        }
        Ok(())
    }
}
