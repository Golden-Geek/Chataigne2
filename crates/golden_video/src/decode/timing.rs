use super::{DecodeError, TimeBase, ffmpeg};
use std::path::Path;

/// Container timing for its selected video stream, without retaining a frame decoder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamTiming {
    /// Declared duration in original stream ticks, if present.
    pub duration: Option<i64>,
    /// Seconds per stream tick.
    pub time_base: TimeBase,
}

/// Probe local-file timing on an IO owner. Cancellation interrupts FFmpeg demux probing.
pub fn probe_timing_with_interrupt(
    path: &Path,
    interrupt: impl FnMut() -> bool + Send + 'static,
) -> Result<StreamTiming, DecodeError> {
    if !path.is_file() {
        return Err(DecodeError::NotAFile);
    }
    ffmpeg::init()?;
    let input = ffmpeg::format::input_with_interrupt(&path, interrupt)?;
    let stream = input
        .streams()
        .best(ffmpeg::media::Type::Video)
        .ok_or(ffmpeg::Error::StreamNotFound)?;
    let base = stream.time_base();
    if base.numerator() <= 0 || base.denominator() <= 0 {
        return Err(ffmpeg::Error::InvalidData.into());
    }
    Ok(StreamTiming {
        duration: (stream.duration() != ffmpeg::ffi::AV_NOPTS_VALUE).then_some(stream.duration()),
        time_base: TimeBase {
            numerator: base.numerator() as u32,
            denominator: base.denominator() as u32,
        },
    })
}
