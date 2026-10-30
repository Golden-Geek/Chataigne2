//! Checked inverse projective transforms prepared before GPU submission.

use super::UnitRect;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, TS)]
/// A point in normalized target coordinates, independent of physical display units.
pub struct Point {
    /// Horizontal fraction.
    pub x: f32,
    /// Vertical fraction.
    pub y: f32,
}

/// Ordered corners in normalized target coordinates; off-target and mirrored quads are valid.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct Quad {
    /// Top-left, top-right, bottom-right, bottom-left source-corner destinations.
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Invalid or numerically unstable crop/perspective geometry.
pub struct MappingError;

impl std::fmt::Display for MappingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("mapping requires a contained crop and a finite, convex, numerically stable quad")
    }
}
impl std::error::Error for MappingError {}

impl Quad {
    /// Reject nonfinite, crossed, concave or collapsed corner sequences.
    pub fn validate(self) -> Result<(), MappingError> {
        if self
            .corners
            .iter()
            .any(|point| !point.x.is_finite() || !point.y.is_finite())
        {
            return Err(MappingError);
        }
        let mut sign = 0.0_f64;
        for index in 0..4 {
            let [a, b, c] = [
                self.corners[index],
                self.corners[(index + 1) % 4],
                self.corners[(index + 2) % 4],
            ];
            let ab = [f64::from(b.x) - f64::from(a.x), f64::from(b.y) - f64::from(a.y)];
            let bc = [f64::from(c.x) - f64::from(b.x), f64::from(c.y) - f64::from(b.y)];
            let cross = ab[0] * bc[1] - ab[1] * bc[0];
            if cross.abs() < 1e-8 || (sign != 0.0 && cross.signum() != sign) {
                return Err(MappingError);
            }
            sign = cross.signum();
        }
        Ok(())
    }
}

/// Private coefficients ensure arbitrary, singular matrices cannot enter an admitted layer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectiveMapping {
    crop: UnitRect,
    inverse: [[f32; 4]; 3],
}

impl ProjectiveMapping {
    /// Prepare and validate inverse coefficients, including GPU f32 corner correspondence.
    pub fn new(crop: UnitRect, quad: Quad) -> Result<Self, MappingError> {
        if !crop.is_valid() {
            return Err(MappingError);
        }
        quad.validate()?;
        // Solve square-to-quad with its finite corner fixing h33=1, then invert. Fixing
        // inverse h33 instead would exclude valid off-target maps whose pole is at the origin.
        let [p0, p1, p2, p3] = quad.corners.map(|p| [f64::from(p.x), f64::from(p.y)]);
        let dx = [p1[0] - p2[0], p3[0] - p2[0], p0[0] - p1[0] + p2[0] - p3[0]];
        let dy = [p1[1] - p2[1], p3[1] - p2[1], p0[1] - p1[1] + p2[1] - p3[1]];
        let divisor = dx[0] * dy[1] - dx[1] * dy[0];
        let g = (dx[2] * dy[1] - dx[1] * dy[2]) / divisor;
        let h = (dx[0] * dy[2] - dx[2] * dy[0]) / divisor;
        let [a, b, c, d, e, f, i, j, k] = [
            p1[0] - p0[0] + g * p1[0],
            p3[0] - p0[0] + h * p3[0],
            p0[0],
            p1[1] - p0[1] + g * p1[1],
            p3[1] - p0[1] + h * p3[1],
            p0[1],
            g,
            h,
            1.0,
        ];
        let determinant = a * (e * k - f * j) - b * (d * k - f * i) + c * (d * j - e * i);
        let inverse = [
            [e * k - f * j, c * j - b * k, b * f - c * e],
            [f * i - d * k, a * k - c * i, c * d - a * f],
            [d * j - e * i, b * i - a * j, a * e - b * d],
        ]
        .map(|row| {
            [
                (row[0] / determinant) as f32,
                (row[1] / determinant) as f32,
                (row[2] / determinant) as f32,
                0.0,
            ]
        });
        if inverse.iter().flatten().any(|value| !value.is_finite()) {
            return Err(MappingError);
        }
        let mapping = Self { crop, inverse };
        // Validate the actual f32 GPU coefficients, including a pole-free positive branch.
        for (point, expected) in quad
            .corners
            .into_iter()
            .zip([[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]])
        {
            let local = mapping.local(point).ok_or(MappingError)?;
            if (local[0] - expected[0]).abs() > 1e-4 || (local[1] - expected[1]).abs() > 1e-4 {
                return Err(MappingError);
            }
        }
        Ok(mapping)
    }

    fn local(self, point: Point) -> Option<[f32; 2]> {
        let value = self.inverse.map(|row| row[0] * point.x + row[1] * point.y + row[2]);
        (value.iter().all(|value| value.is_finite()) && value[2] > 0.0)
            .then(|| [value[0] / value[2], value[1] / value[2]])
    }

    /// CPU reference using the same admitted f32 transform. Outside the quad is transparent.
    pub fn source_coordinate(self, point: Point) -> Option<[f32; 2]> {
        let [x, y] = self.local(point)?;
        if !(0.0..=1.0).contains(&x) || !(0.0..=1.0).contains(&y) {
            return None;
        }
        Some([self.crop.x + x * self.crop.width, self.crop.y + y * self.crop.height])
    }
    /// Normalized source rectangle, sampled after mapping target coordinates to the unit square.
    pub fn crop(self) -> UnitRect {
        self.crop
    }
    /// Padded inverse homography rows for a GPU constant buffer; padding is always zero.
    pub fn inverse_rows(self) -> [[f32; 4]; 3] {
        self.inverse
    }
}
