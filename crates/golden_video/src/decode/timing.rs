use super::{DecodeError, TimeBase, ffmpeg, metadata::raw_tag, webp};
use std::path::Path;

/// Container timing for its selected video stream, without retaining a frame decoder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamTiming {
    /// Duration in original stream ticks, from stream or supported container metadata.
    pub duration: Option<i64>,
    /// Seconds per stream tick.
    pub time_base: TimeBase,
}

/// Probe local-file timing on an IO owner. Cancellation interrupts FFmpeg demux probing.
pub fn probe_timing_with_interrupt(
    path: &Path,
    interrupt: impl FnMut() -> bool + Send + 'static,
) -> Result<StreamTiming, DecodeError> {
    let (input, animation) = webp::open_video(path, interrupt)?;
    stream_timing(&input, animation.as_ref())
}

pub(super) fn stream_timing(
    input: &ffmpeg::format::context::Input,
    animation: Option<&webp::Animation>,
) -> Result<StreamTiming, DecodeError> {
    let stream = input
        .streams()
        .best(ffmpeg::media::Type::Video)
        .ok_or(ffmpeg::Error::StreamNotFound)?;
    let base = stream.time_base();
    if base.numerator() <= 0 || base.denominator() <= 0 {
        return Err(ffmpeg::Error::InvalidData.into());
    }
    Ok(StreamTiming {
        duration: stream_duration(input, &stream, animation),
        time_base: TimeBase {
            numerator: base.numerator() as u32,
            denominator: base.denominator() as u32,
        },
    })
}

pub(super) fn stream_duration(
    input: &ffmpeg::format::context::Input,
    stream: &ffmpeg::Stream<'_>,
    animation: Option<&webp::Animation>,
) -> Option<i64> {
    if let Some(animation) = animation {
        return Some(animation.duration_ms);
    }
    let declared = stream.duration();
    if declared > 0 {
        return Some(declared);
    }
    let format = input.format();
    if format.name() == "webp_pipe" && stream.parameters().id() == ffmpeg::codec::Id::WEBP {
        // The demuxer publishes the still image at tick zero with one tick of duration.
        return Some(1);
    }
    if !format.name().split(',').any(|format| format == "matroska") {
        return None;
    }
    let base = stream.time_base();
    let to_ticks = |micros: i64| {
        let divisor = i128::from(ffmpeg::ffi::AV_TIME_BASE) * i128::from(base.numerator());
        i64::try_from((i128::from(micros) * i128::from(base.denominator())).checked_div(divisor)?).ok()
    };
    let origin = (stream.start_time() != ffmpeg::ffi::AV_NOPTS_VALUE).then(|| stream.start_time());
    let metadata = stream.metadata();
    if let Some(tag) = raw_tag(&metadata, c"DURATION").filter(|tag| tag.to_bytes().len() <= 32) {
        let mut micros = 0;
        // SAFETY: a bounded NUL-terminated tag is live for the call, with a writable result.
        if unsafe { ffmpeg::ffi::av_parse_time(&mut micros, tag.as_ptr(), 1) } == 0
            && let Some(duration) = to_ticks(micros).and_then(|end| end.checked_sub(origin.unwrap_or(0)))
            && duration > 0
        {
            return Some(duration);
        }
    }
    // Matroska's container duration covers all tracks. Prefer the muxed stream end above
    // so a longer audio or subtitle track cannot extend a tagged video stream.
    let duration = to_ticks(input.duration()).filter(|duration| *duration > 0)?;
    // SAFETY: the borrowed, initialized format context lives for this read.
    let container_start = unsafe { (*input.as_ptr()).start_time };
    let offset = if container_start != ffmpeg::ffi::AV_NOPTS_VALUE {
        origin
            .and_then(|start| start.checked_sub(to_ticks(container_start)?))
            .unwrap_or(0)
            .max(0)
    } else {
        0
    };
    duration.checked_sub(offset).filter(|duration| *duration > 0)
}
