//! Metadata-only software image spans, separate from decoder/allocator-private storage.
use super::ffmpeg;

/// Addressed rows of one software image plane, including its actual row padding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct SoftwarePlaneLayout {
    /// Signed FFmpeg row stride; negative strides preserve bottom-up orientation.
    pub stride: i32,
    /// Actual rows, with format-specific chroma subsampling.
    pub rows: u32,
    /// Minimum bytes per row for this format/width, excluding alignment padding.
    pub row_bytes: u32,
    /// Absolute row stride times rows; not a unique backing-allocation measurement.
    pub span_bytes: u64,
}

/// Bounded software-plane metadata. No pixel pointer, allocation or GPU transfer escapes.
/// Plane spans can share backing storage; allocator padding/private codec buffers are separate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct SoftwareFrameLayout {
    /// FFmpeg's static pixel-format name.
    pub format: &'static str,
    /// Actual image width.
    pub width: u32,
    /// Actual image height.
    pub height: u32,
    /// Up to four planes in format order; unused entries are absent.
    pub planes: [Option<SoftwarePlaneLayout>; 4],
    /// Sum of the addressed per-plane spans, not total decoder/heap storage.
    pub span_bytes: u64,
}

impl SoftwareFrameLayout {
    pub(super) fn inspect(image: &ffmpeg::frame::Video) -> Option<Self> {
        let format = image.format();
        let descriptor = format.descriptor()?;
        // SAFETY: FFmpeg owns a static format descriptor and the live borrowed AVFrame.
        // Only metadata is read; hardware/implicit-palette storage is not a CPU plane span.
        let frame = unsafe {
            let flags = (*descriptor.as_ptr()).flags;
            if flags & (ffmpeg::ffi::AV_PIX_FMT_FLAG_HWACCEL | ffmpeg::ffi::AV_PIX_FMT_FLAG_PAL) as u64 != 0 {
                return None;
            }
            &*image.as_ptr()
        };
        let width = i32::try_from(image.width()).ok().filter(|width| *width > 0)?;
        if image.height() == 0 {
            return None;
        }
        // SAFETY: the descriptor validates this pixel format; no image data is accessed.
        let count = usize::try_from(unsafe { ffmpeg::ffi::av_pix_fmt_count_planes(format.into()) }).ok()?;
        if count == 0 || count > 4 || count != image.planes() {
            return None;
        }
        let mut planes = [None; 4];
        let mut span_bytes = 0u64;
        for (index, plane) in planes.iter_mut().enumerate().take(count) {
            if frame.data[index].is_null() {
                return None;
            }
            let stride = frame.linesize[index];
            // SAFETY: same validated format, positive representable width and bounded plane.
            let row_bytes =
                u32::try_from(unsafe { ffmpeg::ffi::av_image_get_linesize(format.into(), width, index as i32) })
                    .ok()
                    .filter(|bytes| *bytes > 0)?;
            if stride.unsigned_abs() < row_bytes {
                return None;
            }
            let rows = image.plane_height(index);
            let span = u64::from(stride.unsigned_abs()).checked_mul(u64::from(rows))?;
            span_bytes = span_bytes.checked_add(span)?;
            *plane = Some(SoftwarePlaneLayout {
                stride,
                rows,
                row_bytes,
                span_bytes: span,
            });
        }
        Some(Self {
            format: descriptor.name(),
            width: image.width(),
            height: image.height(),
            planes,
            span_bytes,
        })
    }
}

#[cfg(test)]
mod tests;
