//! Explicit SDR software conversion on the decoder worker; never a hardware-frame readback.
use super::{DecodedFrame, ffmpeg};
use crate::image::ImageStorageBudget;
use crate::{AlphaMode, ColorEncoding, Extent2D, PixelFormat, TextureDescriptor, image::ImageData};
mod scratch;
use ffmpeg::{
    software::scaling::{Context, Flags},
    util::color::{Range, Space, TransferCharacteristic as Transfer},
};

/// Explicit conversion failure; callers need not parse display strings to report support.
#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
    /// Hardware frames require a GPU importer; software conversion never reads them back.
    #[error("hardware Video storage requires a native importer")]
    HardwareStorage,
    /// HAP remains compressed and must use its GPU block importer.
    #[error("compressed Video storage requires a native importer")]
    CompressedStorage,
    /// The stream declares a color matrix outside the implemented conversion contract.
    #[error("software Video color matrix is not yet supported")]
    UnsupportedMatrix,
    /// The stream declares a transfer function outside the implemented conversion contract.
    #[error("software Video transfer is not yet supported")]
    UnsupportedTransfer,
    /// Scratch or retained images exhaust the caller's allocation-time payload ceiling.
    #[error("software Video image storage capacity retained")]
    Capacity,
    /// Scaler setup, conversion or output admission failed.
    #[error("{0}")]
    Failed(String),
}

/// Worker-local reusable software scaler. Output is straight-alpha sRGB RGBA8.
pub struct SoftwareImageConverter {
    conversion: Option<Conversion>,
    budget: ImageStorageBudget,
}
struct Conversion {
    scaler: Context,
    output: ffmpeg::frame::Video,
}
impl SoftwareImageConverter {
    /// Explicit bound shared by scratch and every separately retained output image.
    /// Codec/scaler-private allocations and heap overhead are separate.
    pub fn new(max_bytes: u64) -> Result<Self, ConversionError> {
        Ok(Self {
            conversion: None,
            budget: ImageStorageBudget::new(max_bytes).map_err(|_| ConversionError::Capacity)?,
        })
    }
    /// Read-only observer that does not keep scratch or output pixels alive.
    pub fn storage_budget(&self) -> ImageStorageBudget {
        self.budget.clone()
    }
    /// Current worker-owned conversion scratch layout, absent until allocation.
    /// Reported spans exclude private scaler/allocator storage and published tight images.
    pub fn scratch_layout(&self) -> Option<super::SoftwareFrameLayout> {
        self.conversion
            .as_ref()
            .and_then(|conversion| super::SoftwareFrameLayout::inspect(&conversion.output))
    }
    /// Convert supported SDR frames explicitly. HDR/wide-gamut metadata is rejected rather
    /// than silently interpreted as sRGB. Untagged SDR follows the documented BT.709 default.
    pub fn convert(&mut self, frame: &DecodedFrame) -> Result<ImageData, ConversionError> {
        if frame.hap_frame().is_some() {
            return Err(ConversionError::CompressedStorage);
        }
        let input = frame.software_image().ok_or(ConversionError::HardwareStorage)?;
        let space = input.color_space();
        if !matches!(space, Space::RGB | Space::BT709 | Space::Unspecified) {
            return Err(ConversionError::UnsupportedMatrix);
        }
        let transfer = input.color_transfer_characteristic();
        if !matches!(
            transfer,
            Transfer::BT709 | Transfer::SMPTE170M | Transfer::Unspecified | Transfer::IEC61966_2_1 | Transfer::Linear
        ) {
            return Err(ConversionError::UnsupportedTransfer);
        }
        let failed = ConversionError::Failed;
        let extent = Extent2D::new(input.width(), input.height()).map_err(|e| failed(e.to_string()))?;
        if input.width() > 16384 || input.height() > 16384 {
            return Err(ConversionError::Capacity);
        }
        let row = input.width() as usize * 4;
        let bytes = row
            .checked_mul(input.height() as usize)
            .ok_or(ConversionError::Capacity)?;
        if self.conversion.as_ref().is_none_or(|conversion| {
            let d = conversion.scaler.input();
            d.format != input.format() || d.width != input.width() || d.height != input.height()
        }) {
            // Free obsolete scratch before replacement admission. Output readers retain
            // their independent bytes and must compete with the replacement's storage.
            self.conversion = None;
        }
        let image_lease = self.budget.reserve(bytes).ok_or(ConversionError::Capacity)?;
        if self.conversion.is_none() {
            self.conversion = Some(Conversion {
                scaler: Context::get(
                    input.format(),
                    input.width(),
                    input.height(),
                    ffmpeg::format::Pixel::RGBA,
                    input.width(),
                    input.height(),
                    Flags::BILINEAR,
                )
                .map_err(|e| failed(e.to_string()))?,
                output: scratch::create(input.width(), input.height(), &self.budget)?,
            });
        }
        let Conversion { scaler, output } = self.conversion.as_mut().unwrap();
        // SAFETY: live uniquely owned SwsContext and FFmpeg's static coefficient table.
        // RGB output is full range; input full/limited range is read from the actual frame.
        let color = unsafe {
            let coefficients = ffmpeg::ffi::sws_getCoefficients(ffmpeg::ffi::SWS_CS_ITU709);
            ffmpeg::ffi::sws_setColorspaceDetails(
                scaler.as_mut_ptr(),
                coefficients,
                i32::from(input.color_range() == Range::JPEG),
                coefficients,
                1,
                0,
                1 << 16,
                1 << 16,
            )
        };
        if color < 0 {
            return Err(failed("software Video colorspace setup failed".into()));
        }
        // A populated output prevents FFmpeg's wrapper from implicitly allocating scratch.
        // Only separately owned, already admitted tight pixels escape this worker.
        scaler.run(input, output).map_err(|e| failed(e.to_string()))?;
        let mut pixels = Vec::with_capacity(bytes);
        let table = match transfer {
            Transfer::IEC61966_2_1 => None,
            Transfer::Linear => Some(linear_table()),
            _ => Some(bt709_table()),
        };
        for bytes in output.data(0).chunks(output.stride(0)).take(input.height() as usize) {
            let start = pixels.len();
            pixels.extend_from_slice(&bytes[..row]);
            if let Some(table) = table {
                for rgba in pixels[start..].chunks_exact_mut(4) {
                    rgba[0] = table[rgba[0] as usize];
                    rgba[1] = table[rgba[1] as usize];
                    rgba[2] = table[rgba[2] as usize];
                }
            }
        }
        ImageData::new(
            TextureDescriptor {
                extent,
                format: PixelFormat::Rgba8Unorm,
                color_encoding: ColorEncoding::Srgb,
                alpha_mode: AlphaMode::Straight,
            },
            pixels,
        )
        .and_then(|image| image.with_storage(image_lease))
        .map_err(|e| failed(e.to_string()))
    }
}
fn encoded(linear: f64) -> u8 {
    let encoded = if linear <= 0.0031308 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    (encoded.clamp(0.0, 1.0) * 255.0).round() as u8
}
fn linear_table() -> &'static [u8; 256] {
    static TABLE: std::sync::OnceLock<[u8; 256]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| std::array::from_fn(|i| encoded(i as f64 / 255.0)))
}
fn bt709_table() -> &'static [u8; 256] {
    static TABLE: std::sync::OnceLock<[u8; 256]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        std::array::from_fn(|i| {
            let x = i as f64 / 255.0;
            encoded(if x < 0.081 {
                x / 4.5
            } else {
                ((x + 0.099) / 1.099).powf(1.0 / 0.45)
            })
        })
    })
}

#[cfg(test)]
mod tests;
