//! Browser-friendly encoding of bounded CPU preview frames.
use crate::Extent2D;
use image_codec::{ExtendedColorType, ImageError, RgbImage, codecs::jpeg::JpegEncoder, imageops};

/// Encodes a tightly packed BGRA capture. The capture may have been resampled into a fixed GPU
/// buffer. Preserve the source aspect inside that bound before encoding.
pub fn encode_preview_jpeg(
    bgra: &[u8],
    capture: Extent2D,
    content: Extent2D,
    quality: u8,
) -> Result<Vec<u8>, ImageError> {
    let mut rgb = Vec::with_capacity(bgra.len() / 4 * 3);
    for pixel in bgra.chunks_exact(4) {
        rgb.extend_from_slice(&[pixel[2], pixel[1], pixel[0]]);
    }
    let image = RgbImage::from_raw(capture.width(), capture.height(), rgb).ok_or_else(|| {
        ImageError::Parameter(image_codec::error::ParameterError::from_kind(
            image_codec::error::ParameterErrorKind::DimensionMismatch,
        ))
    })?;
    let (cw, ch) = (u64::from(capture.width()), u64::from(capture.height()));
    let (iw, ih) = (u64::from(content.width()), u64::from(content.height()));
    let (width, height) = if cw * ih <= ch * iw {
        (cw, (cw * ih / iw).max(1))
    } else {
        ((ch * iw / ih).max(1), ch)
    };
    let (width, height) = (width as u32, height as u32);
    let image = if (image.width(), image.height()) == (width, height) {
        image
    } else {
        imageops::resize(&image, width, height, imageops::FilterType::Triangle)
    };
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, quality).encode(image.as_raw(), width, height, ExtendedColorType::Rgb8)?;
    Ok(jpeg)
}
