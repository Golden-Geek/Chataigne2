use thiserror::Error;

/// Valid SDR linear-light, premultiplied RGBA color.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinearColor([f32; 4]);

impl LinearColor {
    /// Decode a bounded straight-alpha sRGB byte color at an I/O/presentation boundary.
    pub fn from_srgba8(rgba: [u8; 4]) -> Self {
        let alpha = f32::from(rgba[3]) / 255.;
        let mut linear = [0.; 4];
        for index in 0..3 {
            let value = f32::from(rgba[index]) / 255.;
            linear[index] = (if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }) * alpha;
        }
        linear[3] = alpha;
        Self(linear)
    }

    /// Transparent black in the internal compositing representation.
    pub const TRANSPARENT: Self = Self([0.0; 4]);

    /// Validates finite unit-range premultiplied components, including RGB not exceeding alpha.
    pub fn premultiplied(rgba: [f32; 4]) -> Result<Self, ColorError> {
        validate_unit_components(&rgba)?;
        if rgba[..3].iter().any(|component| *component > rgba[3]) {
            return Err(ColorError::NotPremultiplied);
        }
        Ok(Self(rgba))
    }

    /// Validates straight linear-light components and multiplies RGB by coverage alpha.
    pub fn from_straight(red: f32, green: f32, blue: f32, alpha: f32) -> Result<Self, ColorError> {
        validate_unit_components(&[red, green, blue, alpha])?;
        Ok(Self([red * alpha, green * alpha, blue * alpha, alpha]))
    }

    /// Linear premultiplied RGBA, suitable for a renderer clear operation.
    pub const fn components(self) -> [f32; 4] {
        self.0
    }
}

/// Finite unit-range layer opacity, separate from a texture's authored alpha convention.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Opacity(f32);

impl Opacity {
    /// Preserve the input texture's full coverage.
    pub const OPAQUE: Self = Self(1.0);
    /// Suppress the input texture's coverage.
    pub const TRANSPARENT: Self = Self(0.0);

    /// Rejects nonfinite/out-of-range values rather than silently clamping authored intent.
    pub fn new(value: f32) -> Result<Self, ColorError> {
        validate_unit_components(&[value])?;
        Ok(Self(value))
    }

    /// Unit-range opacity.
    pub const fn get(self) -> f32 {
        self.0
    }
}

fn validate_unit_components(components: &[f32]) -> Result<(), ColorError> {
    if components.iter().any(|component| !component.is_finite()) {
        return Err(ColorError::NonFinite);
    }
    if components.iter().any(|component| !(0.0..=1.0).contains(component)) {
        return Err(ColorError::OutOfRange);
    }
    Ok(())
}

/// Invalid SDR color or opacity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum ColorError {
    /// NaN/infinity cannot enter deterministic render plans.
    #[error("color and opacity components must be finite")]
    NonFinite,
    /// The initial SDR contract excludes values outside the unit interval.
    #[error("color and opacity components must be between zero and one")]
    OutOfRange,
    /// Premultiplied RGB cannot exceed its coverage alpha.
    #[error("premultiplied color components cannot exceed alpha")]
    NotPremultiplied,
}

#[cfg(test)]
mod tests;
