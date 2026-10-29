//! Immutable CPU images prepared outside renderer/control threads and uploaded at admission.

use thiserror::Error;

use crate::TextureDescriptor;

/// A tightly packed, immutable RGBA8/BGRA8 image. Source storage/color/alpha is explicit;
/// conversion into the renderer's linear premultiplied targets happens while sampling.
#[derive(Debug)]
pub struct ImageData {
    descriptor: TextureDescriptor,
    pixels: Box<[u8]>,
}

/// CPU image validation failure before any native allocation.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ImageError {
    /// The image exceeds the native texture dimension floor supported by this runtime.
    #[error("image dimension exceeds 16384 texels")]
    Dimension,
    /// The source byte count cannot be represented in this process.
    #[error("image storage size is not representable")]
    SizeOverflow,
    /// Tight four-byte texels do not match the declared image extent.
    #[error("image pixels do not match their tightly packed descriptor")]
    Layout,
}

impl ImageData {
    /// Validates and takes ownership of an already budgeted decoder allocation.
    pub fn new(descriptor: TextureDescriptor, pixels: Vec<u8>) -> Result<Self, ImageError> {
        if descriptor.extent.width() > 16_384 || descriptor.extent.height() > 16_384 {
            return Err(ImageError::Dimension);
        }
        let bytes = descriptor
            .byte_len()
            .ok()
            .and_then(|bytes| usize::try_from(bytes).ok())
            .ok_or(ImageError::SizeOverflow)?;
        if pixels.len() != bytes {
            return Err(ImageError::Layout);
        }
        Ok(Self {
            descriptor,
            pixels: pixels.into_boxed_slice(),
        })
    }

    /// Declared storage, extent, color encoding and alpha convention.
    pub fn descriptor(&self) -> TextureDescriptor {
        self.descriptor
    }
    /// Tightly packed immutable source texels; no native handles are exposed.
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }
    /// Owned value and exact immutable pixel storage, excluding the preparation lease envelope.
    pub fn retained_bytes(&self) -> usize {
        size_of::<Self>() + self.pixels.len()
    }
}

#[cfg(test)]
pub(crate) mod tests;
