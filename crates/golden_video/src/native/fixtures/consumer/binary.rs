//! Bounded black/white binary markers, decoded on the GPU and independently checked after readback.
use serde::{Deserialize, Serialize};

const MAGIC: u32 = 0x56464331;

/// Pixel coordinates and threshold for an explicitly authored binary marker.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BinaryFrameCodeSpec {
    /// Number of least-significant-first luminance bits, from one through32.
    pub bits: u8,
    /// Exact first bit texel, in source pixels.
    pub origin: [u32; 2],
    /// Horizontal texel spacing, greater than zero.
    pub step: u32,
    /// Separate white sentinel texel, outside the bit positions.
    pub sentinel: [u32; 2],
    /// Inclusive eight-bit luminance threshold, between zero and255.
    pub threshold: u8,
}

/// A retained malformed row never becomes an inferred frame number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum BinaryFrameCodeError {
    /// Coordinates, spacing, count or threshold are not admitted.
    #[error("invalid binary marker specification or extent")]
    InvalidSpec,
    /// Raw GPU row has invalid identity/count or nonzero unused pixel slots.
    #[error("binary marker header or padding is corrupt")]
    Header,
    /// The expected white visible marker is absent.
    #[error("opaque white binary marker sentinel is absent")]
    Sentinel,
    /// The saved intensities do not reproduce the saved GPU code.
    #[error("GPU code differs from independently decoded pixel intensities")]
    DecodeMismatch,
}

/// Decoded marker payload. Pixel intensities remain in the enclosing raw GPU row.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BinaryMarker {
    /// Actual code or retained validation failure; never inferred from time or row order.
    pub binary_frame_code: Result<u32, BinaryFrameCodeError>,
}

impl BinaryFrameCodeSpec {
    /// Checks the complete bounded sampling region before any device or log allocation.
    pub fn validate(self, extent: crate::Extent2D) -> Result<(), BinaryFrameCodeError> {
        let overlaps = self.step > 0
            && self.sentinel[1] == self.origin[1]
            && self.sentinel[0]
                .checked_sub(self.origin[0])
                .is_some_and(|distance| distance % self.step == 0 && distance / self.step < u32::from(self.bits));
        let valid = !overlaps
            && (1..=32).contains(&self.bits)
            && self.step > 0
            && (1..255).contains(&self.threshold)
            && u64::from(self.origin[0]) + u64::from(self.bits.saturating_sub(1)) * u64::from(self.step)
                < u64::from(extent.width())
            && self.origin[1] < extent.height()
            && self.sentinel[0] < extent.width()
            && self.sentinel[1] < extent.height();
        if valid {
            Ok(())
        } else {
            Err(BinaryFrameCodeError::InvalidSpec)
        }
    }

    pub(super) fn shader(self) -> String {
        format!(
            "#define CODE_BITS {}u\n#define CODE_X {}u\n#define CODE_Y {}u\n#define CODE_STEP {}u\n\
             #define SENTINEL_X {}u\n#define SENTINEL_Y {}u\n#define CODE_THRESHOLD {}u\n{}",
            self.bits,
            self.origin[0],
            self.origin[1],
            self.step,
            self.sentinel[0],
            self.sentinel[1],
            self.threshold,
            include_str!("binary.hlsl")
        )
    }

    pub(super) fn decode(self, words: [u32; 12]) -> Result<u32, BinaryFrameCodeError> {
        if words[0] != MAGIC || words[3] != u32::from(self.bits) || !(1..=32).contains(&self.bits) {
            return Err(BinaryFrameCodeError::Header);
        }
        if words[2].to_le_bytes().iter().any(|value| *value < self.threshold) {
            return Err(BinaryFrameCodeError::Sentinel);
        }
        let mut code = 0;
        for bit in 0..32 {
            let luminance = ((words[4 + bit / 4] >> ((bit % 4) * 8)) & 255) as u8;
            if bit < self.bits as usize {
                if luminance >= self.threshold {
                    code |= 1 << bit;
                }
            } else if luminance != 0 {
                return Err(BinaryFrameCodeError::Header);
            }
        }
        if words[1] != code {
            return Err(BinaryFrameCodeError::DecodeMismatch);
        }
        Ok(code)
    }
}

#[cfg(test)]
mod tests;
