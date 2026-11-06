use super::ImageData;
use crate::{AlphaMode, ColorEncoding, Extent2D, PixelFormat, TextureDescriptor, plan::UnitRect};
use std::sync::Arc;

/// One admitted image containing up to 32 vertically packed RGBA8 straight/sRGB assets.
/// Exact image storage and sharing use the ordinary plan admission and retirement path.
pub struct ImageAtlas {
    image: Arc<ImageData>,
    regions: Vec<UnitRect>,
}
impl ImageAtlas {
    /// Pack with a one-texel transparent guard around each asset, under a 16 MiB ceiling.
    pub fn pack(images: &[&ImageData]) -> Result<Self, String> {
        if images.is_empty()
            || images.len() > 32
            || images.iter().any(|image| {
                let d = image.descriptor();
                d.format != PixelFormat::Rgba8Unorm
                    || d.color_encoding != ColorEncoding::Srgb
                    || d.alpha_mode != AlphaMode::Straight
            })
        {
            return Err("Canvas atlas requires 1..=32 straight/sRGB RGBA8 images".into());
        }
        let width = images.iter().map(|i| i.descriptor().extent.width() + 2).max().unwrap();
        let height: u32 = images.iter().map(|i| i.descriptor().extent.height() + 2).sum();
        let extent = Extent2D::new(width, height).map_err(|error| error.to_string())?;
        let bytes = u64::from(width) * u64::from(height) * 4;
        if width > 16384 || height > 16384 || bytes > 16 * 1024 * 1024 {
            return Err("Canvas atlas exceeds its dimension or 16 MiB bound".into());
        }
        let mut pixels = vec![0; bytes as usize];
        let mut regions = Vec::with_capacity(images.len());
        let mut y = 1;
        for image in images {
            let source = image.descriptor().extent;
            let stride = source.width() as usize * 4;
            for row in 0..source.height() as usize {
                let target = ((y as usize + row) * width as usize + 1) * 4;
                pixels[target..target + stride].copy_from_slice(&image.pixels()[row * stride..(row + 1) * stride]);
            }
            regions.push(UnitRect {
                x: 1. / width as f32,
                y: y as f32 / height as f32,
                width: source.width() as f32 / width as f32,
                height: source.height() as f32 / height as f32,
            });
            y += source.height() + 2;
        }
        Ok(Self {
            image: Arc::new(
                ImageData::new(
                    TextureDescriptor {
                        extent,
                        format: PixelFormat::Rgba8Unorm,
                        color_encoding: ColorEncoding::Srgb,
                        alpha_mode: AlphaMode::Straight,
                    },
                    pixels,
                )
                .map_err(|error| error.to_string())?,
            ),
            regions,
        })
    }
    /// Shared immutable storage; not an independent media producer.
    pub fn image(&self) -> &Arc<ImageData> {
        &self.image
    }
    /// Packed source coordinates in input order.
    pub fn region(&self, index: usize) -> Option<UnitRect> {
        self.regions.get(index).copied()
    }
}
