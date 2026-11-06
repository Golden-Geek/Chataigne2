//! Finite ASCII coverage preparation, called by a preparation owner, never per frame.
use super::ImageData;
use crate::{AlphaMode, ColorEncoding, Extent2D, PixelFormat, TextureDescriptor};

/// Printable ASCII in a 16-column, six-row grid, with a transparent guard in every cell.
/// The caller owns font selection/licensing and chooses raster density for its destination.
pub struct FontAtlas {
    image: ImageData,
    cell: [u32; 2],
}
impl FontAtlas {
    /// Rasterize 95 printable glyphs into at most 16 MiB. Font files are capped at 4 MiB;
    /// requested scale is pixels per em (2..=256). No parser or outline cache is retained.
    pub fn rasterize(bytes: &[u8], pixels_per_em: u16) -> Result<Self, String> {
        if bytes.len() > 4 * 1024 * 1024 || !(2..=256).contains(&pixels_per_em) {
            return Err("Canvas font exceeds its input or raster scale bound".into());
        }
        let font =
            fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).map_err(|error| error.to_string())?;
        let scale = f32::from(pixels_per_em);
        let mut left = 0;
        let mut right = 0;
        let mut bottom = 0;
        let mut top = 0;
        for code in 32..=126u8 {
            if !font.has_glyph(char::from(code)) {
                return Err("Canvas font lacks printable ASCII coverage".into());
            }
            let m = font.metrics(char::from(code), scale);
            if m.width > 512
                || m.height > 512
                || m.xmin.unsigned_abs() > 512
                || m.ymin.unsigned_abs() > 512
                || !m.advance_width.is_finite()
                || m.advance_width.abs() > 512.
            {
                return Err("Canvas glyph exceeds its finite cell bound".into());
            }
            left = left.min(m.xmin);
            right = right.max(m.xmin + m.width as i32).max(m.advance_width.ceil() as i32);
            bottom = bottom.min(m.ymin);
            top = top.max(m.ymin + m.height as i32);
        }
        let cell = [(right - left + 2) as u32, (top - bottom + 2) as u32];
        let extent = Extent2D::new(cell[0] * 16, cell[1] * 6).map_err(|error| error.to_string())?;
        let size = extent.width() as usize * extent.height() as usize * 4;
        if size > 16 * 1024 * 1024 {
            return Err("Canvas glyph atlas exceeds 16 MiB".into());
        }
        let mut pixels = vec![0; size];
        for code in 32..=126u8 {
            let (m, coverage) = font.rasterize(char::from(code), scale);
            let slot = u32::from(code - 32);
            let x = slot % 16 * cell[0] + (m.xmin - left + 1) as u32;
            let y = slot / 16 * cell[1] + (top - m.ymin - m.height as i32 + 1) as u32;
            for row in 0..m.height {
                for column in 0..m.width {
                    let target = ((y as usize + row) * extent.width() as usize + x as usize + column) * 4;
                    pixels[target..target + 4].copy_from_slice(&[255, 255, 255, coverage[row * m.width + column]]);
                }
            }
        }
        Ok(Self {
            image: ImageData::new(
                TextureDescriptor {
                    extent,
                    format: PixelFormat::Rgba8Unorm,
                    color_encoding: ColorEncoding::Srgb,
                    alpha_mode: AlphaMode::Straight,
                },
                pixels,
            )
            .map_err(|error| error.to_string())?,
            cell,
        })
    }
    /// The common fixed advance/line cell, including its transparent guard.
    pub fn cell(&self) -> [u32; 2] {
        self.cell
    }
    /// Immutable coverage pixels; white straight RGB, linear alpha.
    pub fn image(&self) -> &ImageData {
        &self.image
    }
}
