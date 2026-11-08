//! File demux and software video decode for an owning worker, never an engine callback.
//!
//! Frames retain FFmpeg's reference-counted storage and original time base. They are not
//! implicitly converted, uploaded or read back from a GPU. A media transport and the native
//! hardware-frame importer are separate clients of this boundary.

use ffmpeg_next as ffmpeg;
use std::path::Path;

/// Explicit admission limits, chosen by the owning runtime before opening a decoder.
#[derive(Clone, Copy, Debug)]
pub struct DecodeLimits {
    /// Maximum number of pixels admitted by both stream metadata and libavcodec.
    pub max_pixels: u64,
    /// Maximum compressed packet payload accepted from the demuxer.
    pub max_packet_bytes: usize,
    /// Maximum packets inspected while producing a single frame.
    pub max_packets_per_frame: usize,
}

/// Original stream units; no inferred frame rate or floating-point clock conversion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeBase {
    /// Seconds numerator per stream tick.
    pub numerator: u32,
    /// Seconds denominator per stream tick.
    pub denominator: u32,
}

/// Stable stream metadata, captured before the decoder is opened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamInfo {
    /// Name of the selected FFmpeg codec.
    pub codec: String,
    /// Width of the coded image in pixels.
    pub width: u32,
    /// Height of the coded image in pixels.
    pub height: u32,
    /// Units used by every returned presentation timestamp.
    pub time_base: TimeBase,
    /// Declared first presentation timestamp, if the container provides one.
    pub start: Option<i64>,
    /// Declared stream duration in ticks, if the container provides one.
    pub duration: Option<i64>,
}

/// One owned decoded image. Retaining it does not retain a borrow of the decoder.
pub struct DecodedFrame {
    timestamp: i64,
    image: ffmpeg::frame::Video,
}

impl DecodedFrame {
    /// Best-effort presentation timestamp in the stream's original time base.
    pub fn timestamp(&self) -> i64 {
        self.timestamp
    }

    /// Reference-counted software planes, pixel format and color interpretation metadata.
    /// A native importer must interpret these explicitly; this API performs no conversion.
    pub fn image(&self) -> &ffmpeg::frame::Video {
        &self.image
    }
}

/// Recoverable open/read failure. The worker can drop and reopen this local-file owner.
#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    /// Limits must be positive and representable by libavcodec.
    #[error("invalid video decode limits")]
    InvalidLimits,
    /// Network endpoints and missing/non-regular paths are not local decode inputs.
    #[error("video path is not an existing regular file")]
    NotAFile,
    /// A decoded stream, frame or packet exceeds an explicitly admitted bound.
    #[error("video decode admission limit exceeded")]
    LimitExceeded,
    /// A video without presentation timestamps cannot drive an exact media timeline.
    #[error("decoded video frame has no presentation timestamp")]
    MissingTimestamp,
    /// A failed decoder must be reopened rather than reused after partial input consumption.
    #[error("video decoder failed; reopen the file")]
    ReopenRequired,
    /// FFmpeg open, demux or decode failure; never silently rewritten to clean EOF.
    #[error("FFmpeg: {0}")]
    Ffmpeg(#[from] ffmpeg::Error),
}

/// Single-owner software decoder. Call it on the file/decode worker, never the main thread.
///
/// Admission bounds cover image dimensions, compressed packet acceptance and per-call packet
/// work. They do not claim to cap every temporary allocation inside third-party demuxers.
/// Worker queue admission, cancellation, seeking and native GPU frames are separate concerns.
pub struct SoftwareDecoder {
    input: ffmpeg::format::context::Input,
    decoder: ffmpeg::decoder::Video,
    stream: usize,
    info: StreamInfo,
    limits: DecodeLimits,
    state: DecodeState,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DecodeState {
    Reading,
    Draining,
    Ended,
    Failed,
}

impl SoftwareDecoder {
    /// Open one local file and its best video stream with exactly one codec thread.
    pub fn open(path: &Path, limits: DecodeLimits) -> Result<Self, DecodeError> {
        if limits.max_pixels == 0
            || limits.max_pixels > i64::MAX as u64
            || limits.max_packet_bytes == 0
            || limits.max_packets_per_frame == 0
        {
            return Err(DecodeError::InvalidLimits);
        }
        if !path.is_file() {
            return Err(DecodeError::NotAFile);
        }
        ffmpeg::init()?;
        let input = ffmpeg::format::input(path)?;
        let stream = input
            .streams()
            .best(ffmpeg::media::Type::Video)
            .ok_or(ffmpeg::Error::StreamNotFound)?;
        let index = stream.index();
        let base = stream.time_base();
        if base.numerator() <= 0 || base.denominator() <= 0 {
            return Err(ffmpeg::Error::InvalidData.into());
        }
        let mut context = ffmpeg::codec::context::Context::from_parameters(stream.parameters())?;
        // SAFETY: this is our uniquely owned, unopened AVCodecContext. libavcodec enforces
        // max_pixels before allocating decoded images, including later resolution changes.
        unsafe {
            (*context.as_mut_ptr()).max_pixels = limits.max_pixels as i64;
        }
        context.set_threading(ffmpeg::codec::threading::Config::count(1));
        let decoder = context.decoder().video()?;
        admit_extent(decoder.width(), decoder.height(), limits.max_pixels)?;
        let timestamp = |value| (value != ffmpeg::ffi::AV_NOPTS_VALUE).then_some(value);
        let info = StreamInfo {
            codec: decoder.id().name().to_owned(),
            width: decoder.width(),
            height: decoder.height(),
            time_base: TimeBase {
                numerator: base.numerator() as u32,
                denominator: base.denominator() as u32,
            },
            start: timestamp(stream.start_time()),
            duration: timestamp(stream.duration()),
        };
        Ok(Self {
            input,
            decoder,
            stream: index,
            info,
            limits,
            state: DecodeState::Reading,
        })
    }

    /// Metadata for this decoder's selected stream.
    pub fn info(&self) -> &StreamInfo {
        &self.info
    }

    /// Decode one presentation-ordered frame, including delayed frames at end of stream.
    /// Returns `None` only for clean decoder EOF. Errors require dropping/reopening the owner.
    pub fn next_frame(&mut self) -> Result<Option<DecodedFrame>, DecodeError> {
        if self.state == DecodeState::Failed {
            return Err(DecodeError::ReopenRequired);
        }
        let result = self.receive_next();
        if result.is_err() {
            self.state = DecodeState::Failed;
        }
        result
    }

    fn receive_next(&mut self) -> Result<Option<DecodedFrame>, DecodeError> {
        if self.state == DecodeState::Ended {
            return Ok(None);
        }
        let mut packets = 0;
        loop {
            let mut image = ffmpeg::frame::Video::empty();
            match self.decoder.receive_frame(&mut image) {
                Ok(()) => {
                    admit_extent(image.width(), image.height(), self.limits.max_pixels)?;
                    let timestamp = image.timestamp().ok_or(DecodeError::MissingTimestamp)?;
                    return Ok(Some(DecodedFrame { timestamp, image }));
                }
                Err(ffmpeg::Error::Eof) => {
                    self.state = DecodeState::Ended;
                    return Ok(None);
                }
                Err(ffmpeg::Error::Other { errno }) if errno == ffmpeg::error::EAGAIN => {}
                Err(error) => return Err(error.into()),
            }
            if self.state == DecodeState::Draining {
                return Err(ffmpeg::Error::InvalidData.into());
            }
            loop {
                if packets == self.limits.max_packets_per_frame {
                    return Err(DecodeError::LimitExceeded);
                }
                packets += 1;
                let mut packet = ffmpeg::Packet::empty();
                // PacketIter hides some demux failures as EOF; read directly so corrupt or
                // interrupted input cannot masquerade as successful transport completion.
                match packet.read(&mut self.input) {
                    Ok(()) => {
                        if packet.size() > self.limits.max_packet_bytes {
                            return Err(DecodeError::LimitExceeded);
                        }
                        if packet.stream() == self.stream {
                            self.decoder.send_packet(&packet)?;
                            break;
                        }
                    }
                    Err(ffmpeg::Error::Eof) => {
                        self.decoder.send_eof()?;
                        self.state = DecodeState::Draining;
                        break;
                    }
                    Err(error) => return Err(error.into()),
                }
            }
        }
    }
}

fn admit_extent(width: u32, height: u32, max_pixels: u64) -> Result<(), DecodeError> {
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > max_pixels {
        Err(DecodeError::LimitExceeded)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
