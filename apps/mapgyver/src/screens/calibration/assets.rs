use golden_video::{AlphaMode, ColorEncoding, Extent2D, PixelFormat, TextureDescriptor, image::ImageData};
use std::{
    io::Cursor,
    sync::{Arc, Weak},
};

/// Preparation-owner memoization only. Live plans/textures own the pixels, not this cache.
#[derive(Default)]
pub(crate) struct CalibrationAssets {
    logo: Weak<ImageData>,
}
impl CalibrationAssets {
    pub(super) fn logo(&mut self) -> Result<Arc<ImageData>, String> {
        if let Some(image) = self.logo.upgrade() {
            return Ok(image);
        }
        let mut decoder = png::Decoder::new_with_limits(
            Cursor::new(include_bytes!("../../../assets/mapgyver-icon.png")),
            png::Limits { bytes: 2 * 1024 * 1024 },
        );
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        decoder.set_ignore_text_chunk(true);
        let mut reader = decoder.read_info().map_err(|error| error.to_string())?;
        let info = reader.info();
        if info.width == 0 || info.height == 0 || info.width > 512 || info.height > 512 {
            return Err("Calibration bitmap exceeds its fixed 512x512 decode bound".into());
        }
        let bytes = reader.output_buffer_size().ok_or("Calibration bitmap size overflow")?;
        if bytes > 512 * 512 * 4 {
            return Err("Calibration bitmap exceeds its fixed pixel budget".into());
        }
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(bytes)
            .map_err(|_| "Calibration bitmap allocation is unavailable")?;
        pixels.resize(bytes, 0);
        let output = reader.next_frame(&mut pixels).map_err(|error| error.to_string())?;
        if output.color_type != png::ColorType::Rgba || output.bit_depth != png::BitDepth::Eight {
            return Err("Generated calibration bitmap must contain RGBA8 pixels".into());
        }
        pixels.truncate(output.buffer_size());
        let image = Arc::new(
            ImageData::new(
                TextureDescriptor {
                    extent: Extent2D::new(output.width, output.height).map_err(|error| error.to_string())?,
                    format: PixelFormat::Rgba8Unorm,
                    color_encoding: ColorEncoding::Srgb,
                    alpha_mode: AlphaMode::Straight,
                },
                pixels,
            )
            .map_err(|error| error.to_string())?,
        );
        self.logo = Arc::downgrade(&image);
        Ok(image)
    }
}
