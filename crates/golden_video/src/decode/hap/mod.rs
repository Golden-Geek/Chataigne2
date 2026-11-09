//! HAP reference decode preserves GPU block compression. It never expands pixels to RGBA.
use super::DecodeError;
use crate::Extent2D;
use std::{
    ffi::{c_uint, c_ulong, c_void},
    sync::Arc,
};
mod demux;
mod snappy;
#[allow(non_camel_case_types, non_snake_case, non_upper_case_globals, dead_code)]
mod reference {
    include!(concat!(env!("OUT_DIR"), "/hap_bindings.rs"));
}

/// Compressed GPU texture interpretation, distinct from render-target pixel formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HapFormat {
    /// HAP: BC1 RGB, opaque alpha even when the block uses the three-color mode.
    Bc1Rgb,
    /// HAP Alpha: BC3 straight RGBA.
    Bc3Rgba,
    /// HAP Q: BC3 scaled YCoCg; a GPU shader reconstructs RGB.
    Bc3YCoCg,
}
impl HapFormat {
    /// Exact padded4x4 block storage, including odd visible dimensions.
    pub fn byte_len(self, extent: Extent2D) -> Option<usize> {
        let blocks = u64::from(extent.width()).div_ceil(4) * u64::from(extent.height()).div_ceil(4);
        usize::try_from(blocks.checked_mul(if self == Self::Bc1Rgb { 8 } else { 16 })?).ok()
    }
}
/// Immutable reference-decoded blocks. The visible extent may exclude padded edge texels.
#[derive(Clone)]
pub struct HapFrame {
    extent: Extent2D,
    format: HapFormat,
    blocks: Arc<Vec<u8>>,
}
impl HapFrame {
    /// Visible coded image size.
    pub fn extent(&self) -> Extent2D {
        self.extent
    }
    /// GPU block format; YCoCg is never mislabeled as ordinary RGBA.
    pub fn format(&self) -> HapFormat {
        self.format
    }
    /// Complete tightly packed block rows, not decompressed CPU pixels.
    pub fn blocks(&self) -> &[u8] {
        &self.blocks
    }
    pub(super) fn decode(packet: &[u8], extent: Extent2D) -> Result<Self, DecodeError> {
        let length = c_ulong::try_from(packet.len()).map_err(|_| DecodeError::LimitExceeded)?;
        let mut count = 0;
        let mut format = 0;
        let mut chunks = 0;
        // SAFETY: admitted immutable packet and valid scalar outputs. Reference parsing
        // checks section bounds; local C patches validate chunk input/output slices too.
        unsafe {
            check(reference::HapGetFrameTextureCount(
                packet.as_ptr().cast(),
                length,
                &mut count,
            ))?;
            if count != 1 {
                return Err(DecodeError::UnsupportedCodec);
            }
            check(reference::HapGetFrameTextureFormat(
                packet.as_ptr().cast(),
                length,
                0,
                &mut format,
            ))?;
            check(reference::HapGetFrameTextureChunkCount(
                packet.as_ptr().cast(),
                length,
                0,
                &mut chunks,
            ))?;
        }
        if !(1..=1024).contains(&chunks) {
            return Err(DecodeError::LimitExceeded);
        }
        let original_format = format;
        let format = match i32::try_from(format) {
            Ok(reference::HapTextureFormat_HapTextureFormat_RGB_DXT1) => HapFormat::Bc1Rgb,
            Ok(reference::HapTextureFormat_HapTextureFormat_RGBA_DXT5) => HapFormat::Bc3Rgba,
            Ok(reference::HapTextureFormat_HapTextureFormat_YCoCg_DXT5) => HapFormat::Bc3YCoCg,
            _ => return Err(DecodeError::UnsupportedCodec),
        };
        let expected = format.byte_len(extent).ok_or(DecodeError::LimitExceeded)?;
        let capacity = c_ulong::try_from(expected).map_err(|_| DecodeError::LimitExceeded)?;
        let mut blocks = Vec::new();
        blocks
            .try_reserve_exact(expected)
            .map_err(|_| DecodeError::LimitExceeded)?;
        blocks.resize(expected, 0);
        let mut used = 0;
        let mut decoded_format = 0;
        // SAFETY: disjoint admitted buffers; synchronous callback completes all work before
        // returning. The packet is retained throughout every C/Snappy call.
        unsafe {
            check(reference::HapDecode(
                packet.as_ptr().cast(),
                length,
                0,
                Some(serial),
                std::ptr::null_mut(),
                blocks.as_mut_ptr().cast(),
                capacity,
                &mut used,
                &mut decoded_format,
            ))?;
        }
        if used as usize != expected || decoded_format != original_format {
            return Err(DecodeError::Hap("decoded block extent mismatch".into()));
        }
        Ok(Self {
            extent,
            format,
            blocks: Arc::new(blocks),
        })
    }
}
fn check(result: c_uint) -> Result<(), DecodeError> {
    if result == 0 {
        Ok(())
    } else {
        Err(DecodeError::Hap(format!("reference decoder error {result}")))
    }
}
unsafe extern "C" fn serial(
    function: reference::HapDecodeWorkFunction,
    data: *mut c_void,
    count: c_uint,
    _: *mut c_void,
) {
    if let Some(function) = function {
        for index in 0..count {
            unsafe { function(data, index) };
        }
    }
}

#[cfg(test)]
mod tests;
