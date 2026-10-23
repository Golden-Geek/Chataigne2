use super::super::KeyVisual;
use image::{DynamicImage, GenericImageView, ImageReader, Limits};
use std::{
    fs::File,
    io::{Cursor, Read},
};
const MAX_FILE: usize = 8 * 1024 * 1024;
pub(super) fn compose(size: (usize, usize), visual: &KeyVisual) -> Result<DynamicImage, String> {
    visual.validate()?;
    let (width, height) = size;
    if width == 0 || height == 0 || width > 128 || height > 128 {
        return Err("Stream Deck image dimensions exceed 128x128".into());
    }
    let to_u8 = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let background = image::Rgba([to_u8(visual.color.0), to_u8(visual.color.1), to_u8(visual.color.2), 255]);
    let mut canvas = image::RgbaImage::from_pixel(width as u32, height as u32, background);
    if !visual.image.trim().is_empty() {
        let file = File::open(visual.image.trim()).map_err(|e| format!("Stream Deck image open failed: {e}"))?;
        let metadata = file
            .metadata()
            .map_err(|e| format!("Stream Deck image metadata failed: {e}"))?;
        if !metadata.is_file() || metadata.len() > MAX_FILE as u64 {
            return Err("Stream Deck image must be a regular file of at most 8 MiB".into());
        }
        let mut bytes = Vec::new();
        file.take(MAX_FILE as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("Stream Deck image read failed: {e}"))?;
        if bytes.len() > MAX_FILE {
            return Err("Stream Deck image grew beyond 8 MiB".into());
        }
        let mut reader = ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|e| format!("Stream Deck image format failed: {e}"))?;
        let mut limits = Limits::default();
        limits.max_image_width = Some(2048);
        limits.max_image_height = Some(2048);
        limits.max_alloc = Some(16 * 1024 * 1024);
        reader.limits(limits);
        let overlay = reader
            .decode()
            .map_err(|e| format!("Stream Deck image decode failed: {e}"))?
            .resize_exact(width as u32, height as u32, image::imageops::FilterType::Triangle);
        for (x, y, pixel) in overlay.pixels() {
            let alpha = f64::from(pixel[3]) / 255.0;
            let base = canvas.get_pixel(x, y);
            let blend = |fg: u8, bg: u8| (f64::from(fg) * alpha + f64::from(bg) * (1.0 - alpha)).round() as u8;
            canvas.put_pixel(
                x,
                y,
                image::Rgba([
                    blend(pixel[0], base[0]),
                    blend(pixel[1], base[1]),
                    blend(pixel[2], base[2]),
                    255,
                ]),
            );
        }
    }
    // Caption rendering still requires a bundled font; retain the existing hardware behavior.
    Ok(DynamicImage::ImageRgba8(canvas).to_rgb8().into())
}
#[cfg(test)]
mod tests;
