//! File demux and explicitly selected video decode on an owning worker.
//!
//! Frames retain FFmpeg's reference-counted storage and original time base. They are not
//! implicitly converted, uploaded or read back from a GPU. A media transport and the native
//! hardware-frame importer are separate clients of this boundary.

use ffmpeg_next as ffmpeg;
use std::path::Path;
mod image;
pub use image::{ConversionError, SoftwareImageConverter};
mod timing;
pub use timing::{StreamTiming, probe_timing_with_interrupt};
#[cfg(all(windows, feature = "d3d11"))]
mod d3d11;
#[cfg(all(windows, feature = "d3d11"))]
pub use d3d11::{D3d11Frame, D3d11Storage};

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
    storage: FrameStorage,
}
enum FrameStorage {
    Software(ffmpeg::frame::Video),
    #[cfg(all(windows, feature = "d3d11"))]
    D3d11(D3d11Frame),
}

impl DecodedFrame {
    /// Best-effort presentation timestamp in the stream's original time base.
    pub fn timestamp(&self) -> i64 {
        self.timestamp
    }

    /// Software planes only. Hardware storage cannot be mistaken for CPU pixel pointers.
    pub fn software_image(&self) -> Option<&ffmpeg::frame::Video> {
        match &self.storage {
            FrameStorage::Software(image) => Some(image),
            #[cfg(all(windows, feature = "d3d11"))]
            FrameStorage::D3d11(_) => None,
        }
    }

    /// Worker-local hardware frame and its pool lease, without any CPU transfer.
    #[cfg(all(windows, feature = "d3d11"))]
    pub fn d3d11_frame(&self) -> Option<&D3d11Frame> {
        match &self.storage {
            FrameStorage::Software(_) => None,
            FrameStorage::D3d11(frame) => Some(frame),
        }
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
    /// The selected stream's codec has no available decoder in this build.
    #[error("no decoder is available for this video codec")]
    UnsupportedCodec,
    /// This codec does not advertise the requested hardware interface.
    #[error("codec does not support the selected hardware decoder")]
    UnsupportedHardware,
    /// A hardware context, adapter identity or decoded surface failed validation.
    #[error("hardware video decode: {0}")]
    Hardware(String),
    /// A decoded stream, frame or packet exceeds an explicitly admitted bound.
    #[error("video decode admission limit exceeded")]
    LimitExceeded,
    /// A video without presentation timestamps cannot drive an exact media timeline.
    #[error("decoded video frame has no presentation timestamp")]
    MissingTimestamp,
    /// A failed decoder must be reopened rather than reused after partial input consumption.
    #[error("video decoder failed; reopen the file")]
    ReopenRequired,
    /// A seek is outside the container's declared stream interval.
    #[error("video seek timestamp is outside the declared stream")]
    OutsideStream,
    /// FFmpeg open, demux or decode failure; never silently rewritten to clean EOF.
    #[error("FFmpeg: {0}")]
    Ffmpeg(#[from] ffmpeg::Error),
}

/// Single-owner file decoder. The ordinary open methods explicitly select software.
///
/// Admission bounds cover image dimensions, compressed packet acceptance and per-call packet
/// work. They do not claim to cap every temporary allocation inside third-party demuxers.
/// Worker queue admission and retained frame budgets remain the owning runtime's responsibility.
pub struct Decoder {
    input: ffmpeg::format::context::Input,
    decoder: ffmpeg::decoder::Video,
    stream: usize,
    info: StreamInfo,
    limits: DecodeLimits,
    state: DecodeState,
    kind: DecoderKind,
}

enum DecoderKind {
    Software,
    #[cfg(all(windows, feature = "d3d11"))]
    D3d11(d3d11::HardwareDevice),
}
impl DecoderKind {
    fn configure(&self, context: &mut ffmpeg::codec::context::Context) -> Result<(), DecodeError> {
        match self {
            Self::Software => {
                let _ = context;
                Ok(())
            }
            #[cfg(all(windows, feature = "d3d11"))]
            Self::D3d11(device) => device.configure(context),
        }
    }
    fn frame(&self, image: ffmpeg::frame::Video) -> Result<FrameStorage, DecodeError> {
        match self {
            Self::Software => {
                // Validate the actual format before exposing CPU planes, independently of
                // the requested backend. No unexpected hardware pointer reaches a scaler.
                let descriptor = unsafe { ffmpeg::ffi::av_pix_fmt_desc_get(image.format().into()) };
                if descriptor.is_null()
                    || unsafe { (*descriptor).flags } & ffmpeg::ffi::AV_PIX_FMT_FLAG_HWACCEL as u64 != 0
                {
                    return Err(DecodeError::UnsupportedHardware);
                }
                Ok(FrameStorage::Software(image))
            }
            #[cfg(all(windows, feature = "d3d11"))]
            Self::D3d11(device) => device.frame(image).map(FrameStorage::D3d11),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DecodeState {
    Reading,
    Draining,
    Ended,
    Failed,
}

impl Decoder {
    /// Open one local file and its best video stream with exactly one codec thread.
    pub fn open(path: &Path, limits: DecodeLimits) -> Result<Self, DecodeError> {
        Self::open_with_interrupt(path, limits, || false)
    }

    /// Open with an owning worker's cancellation callback, also used during demux reads.
    pub fn open_with_interrupt(
        path: &Path,
        limits: DecodeLimits,
        interrupt: impl FnMut() -> bool + Send + 'static,
    ) -> Result<Self, DecodeError> {
        Self::open_configured(path, limits, interrupt, |_, _| Ok(DecoderKind::Software))
    }

    fn open_configured(
        path: &Path,
        limits: DecodeLimits,
        interrupt: impl FnMut() -> bool + Send + 'static,
        create_device: impl FnOnce(&ffmpeg::format::context::Input, usize) -> Result<DecoderKind, DecodeError>,
    ) -> Result<Self, DecodeError> {
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
        let input = ffmpeg::format::input_with_interrupt(&path, interrupt)?;
        let stream = input
            .streams()
            .best(ffmpeg::media::Type::Video)
            .ok_or(ffmpeg::Error::StreamNotFound)?;
        let index = stream.index();
        let base = stream.time_base();
        if base.numerator() <= 0 || base.denominator() <= 0 {
            return Err(ffmpeg::Error::InvalidData.into());
        }
        let kind = create_device(&input, index)?;
        let decoder = open_decoder(&input, index, limits, &kind)?;
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
            kind,
        })
    }

    /// Metadata for this decoder's selected stream.
    pub fn info(&self) -> &StreamInfo {
        &self.info
    }

    /// Seek to a keyframe at/before an original stream timestamp with a fresh codec owner.
    /// The worker then decodes forward to its exact timestamp. This never estimates a frame
    /// index from FPS. A failed demux seek poisons the owner; an out-of-range timestamp does
    /// not disturb its current state. Clean EOF can be rewound without reopening the file.
    pub fn seek_before(&mut self, timestamp: i64) -> Result<(), DecodeError> {
        if self.state == DecodeState::Failed {
            return Err(DecodeError::ReopenRequired);
        }
        if let Some(start) = self.info.start {
            let after_end = self
                .info
                .duration
                .filter(|duration| *duration > 0)
                .is_some_and(|duration| i128::from(timestamp) > i128::from(start) + i128::from(duration));
            if timestamp < start || after_end {
                return Err(DecodeError::OutsideStream);
            }
        }
        // A discontinuity replaces codec-owned reorder/drain state rather than depending on
        // avcodec_flush_buffers to discard every delayed frame. The admitted HEVC fixture
        // emits an old drained frame after flush. Keep one demux/file owner; decoded images
        // already handed to consumers retain their independent reference-counted storage.
        let decoder = match open_decoder(&self.input, self.stream, self.limits, &self.kind) {
            Ok(decoder) => decoder,
            Err(error) => {
                self.state = DecodeState::Failed;
                return Err(error);
            }
        };
        // Keep the selected stream's exact time base, rather than converting through the
        // high-level Input::seek API's global microsecond coordinate.
        // SAFETY: the uniquely owned live input retains the validated stream index. FFmpeg
        // performs its own demux-buffer flush; no borrowed packet/frame refers to that input.
        let result = unsafe {
            ffmpeg::ffi::av_seek_frame(
                self.input.as_mut_ptr(),
                self.stream as i32,
                timestamp,
                ffmpeg::ffi::AVSEEK_FLAG_BACKWARD,
            )
        };
        if result < 0 {
            self.state = DecodeState::Failed;
            return Err(ffmpeg::Error::from(result).into());
        }
        self.decoder = decoder;
        self.state = DecodeState::Reading;
        Ok(())
    }

    /// Decode one presentation-ordered frame, including delayed frames at end of stream.
    /// Returns `None` only for clean decoder EOF. Errors require dropping/reopening the owner.
    pub fn next_frame(&mut self) -> Result<Option<DecodedFrame>, DecodeError> {
        if self.state == DecodeState::Failed {
            return Err(DecodeError::ReopenRequired);
        }
        let result = self.receive_next();
        #[cfg(all(windows, feature = "d3d11"))]
        let result = result.map_err(|error| {
            if let DecoderKind::D3d11(device) = &self.kind {
                return device.failure(error);
            }
            error
        });
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
                    let storage = self.kind.frame(image)?;
                    return Ok(Some(DecodedFrame { timestamp, storage }));
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

fn open_decoder(
    input: &ffmpeg::format::context::Input,
    index: usize,
    limits: DecodeLimits,
    kind: &DecoderKind,
) -> Result<ffmpeg::decoder::Video, DecodeError> {
    let stream = input.stream(index).ok_or(ffmpeg::Error::StreamNotFound)?;
    let mut context = ffmpeg::codec::context::Context::from_parameters(stream.parameters())?;
    // SAFETY: uniquely owned, unopened context; libavcodec enforces this bound before
    // allocating decoded images, including later in-stream resolution changes.
    unsafe {
        (*context.as_mut_ptr()).max_pixels = limits.max_pixels as i64;
    }
    context.set_threading(ffmpeg::codec::threading::Config::count(1));
    kind.configure(&mut context)?;
    let mut decoder = context.decoder();
    decoder.set_packet_time_base(stream.time_base());
    let decoder = decoder.video().map_err(|error| {
        if error == ffmpeg::Error::DecoderNotFound {
            DecodeError::UnsupportedCodec
        } else {
            error.into()
        }
    })?;
    admit_extent(decoder.width(), decoder.height(), limits.max_pixels)?;
    Ok(decoder)
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
