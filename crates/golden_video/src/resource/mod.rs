use std::num::NonZeroU32;

use thiserror::Error;

/// Nonempty texture extent in texels, independent of CSS units or desktop DPI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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

/// Initial supported storage formats; channel layout does not imply transfer or alpha semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PixelFormat {
    /// Eight-bit red, green, blue and alpha channels.
    Rgba8Unorm,
    /// Eight-bit blue, green, red and alpha channels.
    Bgra8Unorm,
}

impl PixelFormat {
    /// Tight logical storage per texel, excluding backend alignment and pool multiplicity.
    pub const fn bytes_per_pixel(self) -> u64 {
        match self {
            Self::Rgba8Unorm | Self::Bgra8Unorm => 4,
        }
    }
}

/// Transfer encoding of stored RGB channels. Alpha always represents linear coverage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ColorEncoding {
    /// Linear-light RGB.
    Linear,
    /// Standard sRGB transfer encoding; a consumer must decode before compositing.
    Srgb,
}

/// Explicit source alpha convention; never inferred from the storage format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
