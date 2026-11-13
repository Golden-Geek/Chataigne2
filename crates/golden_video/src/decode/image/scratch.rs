//! One explicitly allocated RGBA output frame; swscale never allocates its plane.
use super::{ConversionError, ffmpeg};
use crate::image::ImageStorageBudget;

pub(super) fn create(
    width: u32,
    height: u32,
    budget: &ImageStorageBudget,
) -> Result<ffmpeg::frame::Video, ConversionError> {
    let row = (width as usize).checked_mul(4).ok_or(ConversionError::Capacity)?;
    // Align rows for SIMD and retain padded rows/tail for vectorized scaler writes.
    // The complete requested allocation, rather than just visible rows, is charged.
    let stride = row.div_ceil(64).checked_mul(64).ok_or(ConversionError::Capacity)?;
    let rows = (height as usize)
        .div_ceil(32)
        .checked_mul(32)
        .ok_or(ConversionError::Capacity)?;
    let bytes = stride
        .checked_mul(rows)
        .and_then(|bytes| bytes.checked_add(64))
        .ok_or(ConversionError::Capacity)?;
    let lease = budget.reserve(bytes).ok_or(ConversionError::Capacity)?;
    let buffer = super::super::buffer::allocate(lease);
    if buffer.is_null() {
        return Err(ConversionError::Failed(
            "software Video scratch allocation failed".into(),
        ));
    }
    let mut output = ffmpeg::frame::Video::empty();
    output.set_format(ffmpeg::format::Pixel::RGBA);
    output.set_width(width);
    output.set_height(height);
    // SAFETY: uniquely owned empty frame takes this complete reference-counted buffer.
    // Dimensions were validated by the caller; its stride fits a <=16384-texel RGBA row.
    unsafe {
        let frame = &mut *output.as_mut_ptr();
        frame.buf[0] = buffer;
        frame.data[0] = (*buffer).data;
        frame.linesize[0] = stride as i32;
        frame.extended_data = frame.data.as_mut_ptr();
    }
    Ok(output)
}
