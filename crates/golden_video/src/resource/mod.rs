use std::num::NonZeroU32;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

/// Nonempty texture extent in texels, independent of CSS units or desktop DPI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub struct Extent2D {
    width: NonZeroU32,
    height: NonZeroU32,
}

impl Extent2D {
    /// Rejects empty extents. Device-specific dimension and budget limits are checked by plans.
    pub const fn new(width: u32, height: u32) -> Result<Self, TextureError> {
        match (NonZeroU32::new(width), NonZeroU32::new(height)) {
            (Some(width), Some(height)) => Ok(Self { width, height }),
            _ => Err(TextureError::EmptyExtent { width, height }),
        }
    }

    /// Width in texels.
    pub const fn width(self) -> u32 {
        self.width.get()
    }

    /// Height in texels.
    pub const fn height(self) -> u32 {
        self.height.get()
    }
}

/// Typed color storage; channel layout does not imply transfer encoding or premultiplication.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub enum PixelFormat {
    /// Eight-bit red, green, blue and alpha channels.
    Rgba8Unorm,
    /// Eight-bit blue, green, red and alpha channels.
    Bgra8Unorm,
    /// Eight-bit blue, green and red with an unused byte; sampling supplies alpha one.
    Bgrx8Unorm,
    /// Signed normalized eight-bit RGBA channels.
    Rgba8Snorm,
    /// Unsigned normalized sixteen-bit RGBA channels.
    Rgba16Unorm,
    /// Signed normalized sixteen-bit RGBA channels.
    Rgba16Snorm,
    /// Packed unsigned normalized ten-bit RGB and two-bit alpha.
    Rgb10A2Unorm,
    /// IEEE 754 binary16 red, green, blue and alpha channels, eight little-endian bytes per texel.
    Rgba16Float,
    /// IEEE 754 binary32 RGBA channels, sixteen little-endian bytes per texel.
    Rgba32Float,
    /// Unsigned normalized red, one byte per texel.
    R8Unorm,
    /// Signed normalized red, one byte per texel.
    R8Snorm,
    /// Unsigned normalized red/green, two bytes per texel.
    Rg8Unorm,
    /// Signed normalized red/green, two bytes per texel.
    Rg8Snorm,
    /// Unsigned normalized red, two bytes per texel.
    R16Unorm,
    /// Signed normalized red, two bytes per texel.
    R16Snorm,
    /// Unsigned normalized red/green, four bytes per texel.
    Rg16Unorm,
    /// Signed normalized red/green, four bytes per texel.
    Rg16Snorm,
    /// IEEE 754 binary16 red, two bytes per texel.
    R16Float,
    /// IEEE 754 binary16 red/green, four bytes per texel.
    Rg16Float,
    /// IEEE 754 binary32 red, four bytes per texel.
    R32Float,
    /// Packed unsigned floating-point eleven-bit red/green and ten-bit blue.
    Rg11B10Float,
    /// Unsigned normalized coverage alpha, one byte per texel; RGB samples as zero.
    A8Unorm,
    /// IEEE 754 binary32 red/green, eight little-endian bytes per texel.
    Rg32Float,
    /// Packed blue/green/red normalized channels (5/6/5 bits), two bytes per texel.
    Bgr565Unorm,
    /// Packed blue/green/red/alpha normalized channels (5/5/5/1 bits), two bytes per texel.
    Bgra5551Unorm,
}

impl PixelFormat {
    /// Tight logical storage per texel, excluding backend alignment and pool multiplicity.
    pub const fn bytes_per_pixel(self) -> u64 {
        match self {
            Self::Rgba8Unorm | Self::Bgra8Unorm | Self::Bgrx8Unorm | Self::Rgba8Snorm | Self::Rgb10A2Unorm => 4,
            Self::Rgba16Float | Self::Rgba16Unorm | Self::Rgba16Snorm => 8,
            Self::Rgba32Float => 16,
            Self::R8Unorm | Self::R8Snorm | Self::A8Unorm => 1,
            Self::Rg8Unorm | Self::Rg8Snorm | Self::R16Unorm | Self::R16Snorm | Self::R16Float => 2,
            Self::Rg16Unorm | Self::Rg16Snorm | Self::Rg16Float | Self::R32Float | Self::Rg11B10Float => 4,
            Self::Rg32Float => 8,
            Self::Bgr565Unorm | Self::Bgra5551Unorm => 2,
        }
    }

    /// Whether native storage carries alpha; absent color components sample as zero,
    /// and storage without alpha supplies coverage one.
    pub const fn has_alpha(self) -> bool {
        matches!(
            self,
            Self::Rgba8Unorm
                | Self::Bgra8Unorm
                | Self::Rgba8Snorm
                | Self::Rgba16Unorm
                | Self::Rgba16Snorm
                | Self::Rgb10A2Unorm
                | Self::Rgba16Float
                | Self::Rgba32Float
                | Self::A8Unorm
                | Self::Bgra5551Unorm
        )
    }
}

/// Transfer encoding of stored RGB channels. Alpha always represents linear coverage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub enum ColorEncoding {
    /// Linear-light RGB.
    Linear,
    /// Standard sRGB transfer encoding; a consumer must decode before compositing.
    Srgb,
}

/// Explicit source alpha convention; endpoint discovery supplies its documented default.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub enum AlphaMode {
    /// Stored RGB is independent of coverage alpha.
    Straight,
    /// Stored RGB is multiplied by coverage in its declared encoding.
    ///
    /// An sRGB source must be unpremultiplied before decoding, then premultiplied in
    /// linear space. Renderer-internal textures use linear premultiplied RGB.
    Premultiplied,
    /// Ignore stored alpha and use full coverage.
    Opaque,
}

/// Declared logical image layout; contains no native interfaces or ownership handles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub struct TextureDescriptor {
    /// Nonempty extent in texels.
    pub extent: Extent2D,
    /// Stored channel order and precision.
    pub format: PixelFormat,
    /// Stored RGB transfer encoding.
    pub color_encoding: ColorEncoding,
    /// Stored RGB/alpha relationship.
    pub alpha_mode: AlphaMode,
}

impl TextureDescriptor {
    /// Checked tight logical size. Actual resource admission must also charge native padding,
    /// staging/bridge pools, prepared/live/in-flight/retiring generations and cache leases.
    pub fn byte_len(self) -> Result<u64, TextureError> {
        u64::from(self.extent.width())
            .checked_mul(u64::from(self.extent.height()))
            .and_then(|pixels| pixels.checked_mul(self.format.bytes_per_pixel()))
            .ok_or(TextureError::ByteSizeOverflow {
                width: self.extent.width(),
                height: self.extent.height(),
            })
    }

    /// Whether the descriptor satisfies the renderer's internal compositing contract.
    pub const fn is_compositing_target(self) -> bool {
        matches!(self.color_encoding, ColorEncoding::Linear) && matches!(self.alpha_mode, AlphaMode::Premultiplied)
    }
}

/// Invalid logical extent or unrepresentable storage charge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum TextureError {
    /// An absent/unresolved source must be represented separately, not as an empty texture.
    #[error("texture extent must be nonempty, got {width}x{height}")]
    EmptyExtent {
        /// Supplied width.
        width: u32,
        /// Supplied height.
        height: u32,
    },
    /// Storage size overflowed rather than wrapping to a smaller admission charge.
    #[error("texture byte size overflows for {width}x{height}")]
    ByteSizeOverflow {
        /// Supplied width.
        width: u32,
        /// Supplied height.
        height: u32,
    },
}

#[cfg(test)]
mod tests;
