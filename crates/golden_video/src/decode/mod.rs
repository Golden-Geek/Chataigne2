//! File demux and explicitly selected video decode on an owning worker.
//!
//! Frames retain FFmpeg's reference-counted storage and original time base. They are not
//! implicitly converted, uploaded or read back from a GPU. A media transport and the native
//! hardware-frame importer are separate clients of this boundary.

use ffmpeg_next as ffmpeg;
use std::path::Path;
mod buffer;
mod hap;
mod image;
mod layout;
mod parallelism;
mod software_pool;
pub use hap::{HapFormat, HapFrame};
pub use image::{ConversionError, SoftwareImageConverter};
pub use layout::{SoftwareFrameLayout, SoftwarePlaneLayout};
pub use parallelism::SoftwareDecodeThreads;
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
    /// Maximum allocated software decoded-plane payload, including idle pool buffers.
    /// Scaler, published images, codec-private state and demux allocations are separate.
    pub max_software_frame_bytes: u64,
    /// Maximum compressed packet payload accepted from the demuxer.
    pub max_packet_bytes: usize,
    /// Maximum packets inspected while producing a single frame.
    pub max_packets_per_frame: usize,
    /// Bounded slice workers for capable software codecs; never frame threading.
    pub software_threads: SoftwareDecodeThreads,
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

/// Storage selected before decoding, so owners can admit resources without parsing codec names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeStorage {
    /// CPU pixel planes, explicitly converted by a software consumer.
    Software,
    /// Compressed HAP blocks, requiring native texture sampling.
    Hap,
    /// Worker-local D3D11 decoder pool surfaces.
    #[cfg(all(windows, feature = "d3d11"))]
    D3d11,
}

/// One owned decoded image. Retaining it does not retain a borrow of the decoder.
pub struct DecodedFrame {
    timestamp: i64,
    storage: FrameStorage,
}
enum FrameStorage {
    Software(ffmpeg::frame::Video),
    Hap(HapFrame),
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
            FrameStorage::Hap(_) => None,
            #[cfg(all(windows, feature = "d3d11"))]
            FrameStorage::D3d11(_) => None,
        }
    }

    /// Actual software plane/stride spans, without reading pixels or retaining a frame.
    /// Hardware, compressed or unrepresentable layouts remain absent rather than zero.
    pub fn software_layout(&self) -> Option<SoftwareFrameLayout> {
        self.software_image().and_then(SoftwareFrameLayout::inspect)
    }

    /// Worker-local hardware frame and its pool lease, without any CPU transfer.
    #[cfg(all(windows, feature = "d3d11"))]
    pub fn d3d11_frame(&self) -> Option<&D3d11Frame> {
        match &self.storage {
            FrameStorage::Software(_) => None,
            FrameStorage::Hap(_) => None,
            FrameStorage::D3d11(frame) => Some(frame),
        }
    }
    /// HAP's reference-decoded GPU blocks, without software pixel expansion.
    pub fn hap_frame(&self) -> Option<&HapFrame> {
        match &self.storage {
            FrameStorage::Hap(frame) => Some(frame),
            _ => None,
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
    /// The codec cannot use the allocation-controlled software frame pool.
    #[error("codec cannot provide allocation-controlled software frames")]
    UnsupportedSoftwareStorage,
    /// Invalid reference-coded HAP payload, distinct from unsupported texture variants.
    #[error("HAP: {0}")]
    Hap(String),
    /// The codec, stream format or healthy device cannot provide the requested hardware storage.
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

/// Single-owner file decoder. Ordinary open preserves HAP blocks and otherwise selects software.
///
/// Admission bounds cover image dimensions, compressed packet acceptance and per-call packet
/// work. They do not claim to cap every temporary allocation inside third-party demuxers.
/// Worker queue admission and retained frame budgets remain the owning runtime's responsibility.
pub struct Decoder {
    input: ffmpeg::format::context::Input,
    decoder: Option<ffmpeg::decoder::Video>,
    stream: usize,
    info: StreamInfo,
    limits: DecodeLimits,
    allocation_pixel_limit: u64,
    state: DecodeState,
    kind: DecoderKind,
}

enum DecoderKind {
    Software(Box<software_pool::Allocator>),
    Hap,
    #[cfg(all(windows, feature = "d3d11"))]
    D3d11(d3d11::HardwareDevice),
}
impl DecoderKind {
    fn configure(&self, context: &mut ffmpeg::codec::context::Context) -> Result<(), DecodeError> {
        match self {
            Self::Software(allocator) => allocator.configure(context),
            Self::Hap => {
                let _ = context;
                Ok(())
            }
            #[cfg(all(windows, feature = "d3d11"))]
            Self::D3d11(device) => device.configure(context),
        }
    }
    fn frame(&self, image: ffmpeg::frame::Video) -> Result<FrameStorage, DecodeError> {
        match self {
            Self::Hap => Err(DecodeError::UnsupportedCodec),
            Self::Software(_) => {
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
    /// Open local video with admitted software slice parallelism and preserved HAP blocks.
    pub fn open(path: &Path, limits: DecodeLimits) -> Result<Self, DecodeError> {
        Self::open_with_interrupt(path, limits, || false)
    }

    /// Open with an owning worker's cancellation callback, also used during demux reads.
    pub fn open_with_interrupt(
        path: &Path,
        limits: DecodeLimits,
        interrupt: impl FnMut() -> bool + Send + 'static,
    ) -> Result<Self, DecodeError> {
        Self::open_configured(path, limits, interrupt, |input, index| {
            Ok(
                if input
                    .stream(index)
                    .ok_or(ffmpeg::Error::StreamNotFound)?
                    .parameters()
                    .id()
                    == ffmpeg::codec::Id::HAP
                {
                    DecoderKind::Hap
                } else {
                    DecoderKind::Software(Box::new(software_pool::Allocator::new(
                        limits.max_software_frame_bytes,
                    )?))
                },
            )
        })
    }

    fn open_configured(
        path: &Path,
        limits: DecodeLimits,
        interrupt: impl FnMut() -> bool + Send + 'static,
        create_device: impl FnOnce(&ffmpeg::format::context::Input, usize) -> Result<DecoderKind, DecodeError>,
    ) -> Result<Self, DecodeError> {
        if limits.max_pixels == 0
            || limits.max_pixels > i64::MAX as u64
            || limits.max_software_frame_bytes == 0
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
        let decoder = if matches!(kind, DecoderKind::Hap) {
            None
        } else {
            Some(open_decoder(&input, index, limits, &kind, limits.max_pixels)?)
        };
        // HAP demux needs no libavcodec pixel decoder or software texture expansion.
        let parameters = stream.parameters();
        let (width, height) = if let Some(decoder) = &decoder {
            (decoder.width(), decoder.height())
        } else {
            unsafe {
                (
                    u32::try_from((*parameters.as_ptr()).width).unwrap_or(0),
                    u32::try_from((*parameters.as_ptr()).height).unwrap_or(0),
                )
            }
        };
        admit_extent(width, height, limits.max_pixels)?;
        let timestamp = |value| (value != ffmpeg::ffi::AV_NOPTS_VALUE).then_some(value);
        let info = StreamInfo {
            codec: parameters.id().name().to_owned(),
            width,
            height,
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
            allocation_pixel_limit: limits.max_pixels,
            state: DecodeState::Reading,
            kind,
        })
    }

    /// Metadata for this decoder's selected stream.
    pub fn info(&self) -> &StreamInfo {
        &self.info
    }
    /// Actual software decoded-plane inventory, including idle pool storage. The observer
    /// does not retain pixels; native/HAP decoders have no software plane pool.
    pub fn software_frame_budget(&self) -> Option<crate::image::ImageStorageBudget> {
        match &self.kind {
            DecoderKind::Software(allocator) => Some(allocator.budget.clone()),
            _ => None,
        }
    }
    /// Tighten allocation-time software plane admission without increasing its ceiling or
    /// discarding retained frames. The same pool/budget survives codec replacement on seek.
    /// Scaler, published images and codec-private storage require separate owning admission.
    pub fn tighten_software_frame_limit(&mut self, bytes: u64) -> Result<(), DecodeError> {
        if self.state == DecodeState::Failed {
            return Err(DecodeError::ReopenRequired);
        }
        match &self.kind {
            DecoderKind::Software(allocator) => allocator.budget.tighten(bytes).map_err(|error| match error {
                crate::image::ImageError::Capacity => DecodeError::LimitExceeded,
                _ => DecodeError::InvalidLimits,
            }),
            _ => Err(DecodeError::InvalidLimits),
        }
    }
    /// Tighten future frame allocation/admission without increasing the original ceiling.
    /// Owners reserving storage from initial stream dimensions call this before decoding.
    /// Retained frames keep their storage; invalid requests leave this decoder unchanged.
    /// The new ceiling also applies to codec replacement after drain/EOF seeks.
    pub fn tighten_pixel_limit(&mut self, max_pixels: u64) -> Result<(), DecodeError> {
        if self.state == DecodeState::Failed {
            return Err(DecodeError::ReopenRequired);
        }
        if max_pixels == 0 || max_pixels > self.limits.max_pixels {
            return Err(DecodeError::InvalidLimits);
        }
        admit_extent(self.info.width, self.info.height, max_pixels)?;
        let allocation_pixels = if let Some(decoder) = &mut self.decoder {
            let padded = padded_pixels(decoder, self.info.width, self.info.height)?.max(max_pixels);
            self.allocation_pixel_limit.min(padded)
        } else {
            max_pixels
        };
        if let Some(decoder) = &mut self.decoder {
            // SAFETY: uniquely owned live context between synchronous decoder calls.
            // Frame threading is rejected at open; retained frames own separate references.
            unsafe { (*decoder.as_mut_ptr()).max_pixels = allocation_pixels as i64 };
        }
        self.limits.max_pixels = max_pixels;
        self.allocation_pixel_limit = allocation_pixels;
        Ok(())
    }
    /// Stable storage contract chosen at open, without decoding a frame or parsing a label.
    pub fn storage(&self) -> DecodeStorage {
        match self.kind {
            DecoderKind::Software(_) => DecodeStorage::Software,
            DecoderKind::Hap => DecodeStorage::Hap,
            #[cfg(all(windows, feature = "d3d11"))]
            DecoderKind::D3d11(_) => DecodeStorage::D3d11,
        }
    }

    /// Actual codec thread count for an available software decoder; HAP/hardware return None.
    /// This excludes the owning IO task and does not describe decoder storage bytes.
    pub fn software_decode_threads(&self) -> Option<usize> {
        self.decoder
            .as_ref()
            .filter(|_| matches!(self.kind, DecoderKind::Software(_)))
            .map(|decoder| decoder.threading().count)
    }

    /// Seek to a keyframe at/before an original stream timestamp.
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
        // Ordinary scrubbing keeps the admitted codec and hardware frame pool. Recreating
        // them per pointer update can starve the latest request. Drained HEVC can emit an
        // old delayed frame after flush, so a drain/EOF discontinuity still gets a new codec.
        // Consumers keep independent frame references across either path.
        let replacement = if self.state != DecodeState::Reading && !matches!(self.kind, DecoderKind::Hap) {
            match open_decoder(
                &self.input,
                self.stream,
                self.limits,
                &self.kind,
                self.allocation_pixel_limit,
            ) {
                Ok(decoder) => Some(decoder),
                Err(error) => {
                    self.state = DecodeState::Failed;
                    return Err(error);
                }
            }
        } else {
            None
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
        if let Some(decoder) = replacement {
            self.decoder = Some(decoder);
        } else if let Some(decoder) = &mut self.decoder {
            decoder.flush();
        }
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
        let result = result.map_err(|error| match &self.kind {
            DecoderKind::Software(allocator) => allocator.failure(error),
            _ => error,
        });
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
        if matches!(self.kind, DecoderKind::Hap) {
            return self.receive_hap();
        }
        if self.state == DecodeState::Ended {
            return Ok(None);
        }
        let mut packets = 0;
        loop {
            let mut image = ffmpeg::frame::Video::empty();
            match self.decoder.as_mut().unwrap().receive_frame(&mut image) {
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
                            self.decoder.as_mut().unwrap().send_packet(&packet)?;
                            break;
                        }
                    }
                    Err(ffmpeg::Error::Eof) => {
                        self.decoder.as_mut().unwrap().send_eof()?;
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
    allocation_pixel_limit: u64,
) -> Result<ffmpeg::decoder::Video, DecodeError> {
    let stream = input.stream(index).ok_or(ffmpeg::Error::StreamNotFound)?;
    let mut context = ffmpeg::codec::context::Context::from_parameters(stream.parameters())?;
    // SAFETY: uniquely owned, unopened context; libavcodec enforces this bound before
    // allocating decoded images, including later in-stream resolution changes.
    unsafe {
        (*context.as_mut_ptr()).max_pixels = allocation_pixel_limit as i64;
    }
    let threading = parallelism::configuration(&context, kind, limits.software_threads);
    let admitted_threads = threading.count;
    context.set_threading(threading);
    kind.configure(&mut context)?;
    let mut decoder = context.decoder();
    decoder.set_packet_time_base(stream.time_base());
    let decoder = decoder.video().map_err(|error| {
        let error = if error == ffmpeg::Error::DecoderNotFound {
            DecodeError::UnsupportedCodec
        } else {
            error.into()
        };
        #[cfg(all(windows, feature = "d3d11"))]
        if let DecoderKind::D3d11(device) = kind {
            return device.failure(error);
        }
        error
    })?;
    // SAFETY: immutable metadata on this live opened context. The wrapper's enum collapses
    // unknown bit combinations, so inspect the frame-thread flag before admitting the owner.
    let frame_threads = unsafe { (*decoder.as_ptr()).active_thread_type & ffmpeg::ffi::FF_THREAD_FRAME != 0 };
    if frame_threads || decoder.threading().count > admitted_threads {
        return Err(DecodeError::LimitExceeded);
    }
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

fn padded_pixels(decoder: &mut ffmpeg::decoder::Video, width: u32, height: u32) -> Result<u64, DecodeError> {
    let mut width = i32::try_from(width).map_err(|_| DecodeError::LimitExceeded)?;
    let mut height = i32::try_from(height).map_err(|_| DecodeError::LimitExceeded)?;
    let mut lines = [0i32; 8];
    // SAFETY: uniquely borrowed live context and complete AV_NUM_DATA_POINTERS output.
    // FFmpeg's allocation check rounds width to its stride alignment; keep that padding
    // separate from the logical returned-frame ceiling instead of rejecting valid odd widths.
    unsafe {
        ffmpeg::ffi::avcodec_align_dimensions2(decoder.as_mut_ptr(), &mut width, &mut height, lines.as_mut_ptr());
    }
    let width = u64::try_from(width).map_err(|_| DecodeError::LimitExceeded)?;
    let height = u64::try_from(height).map_err(|_| DecodeError::LimitExceeded)?;
    let align = u64::try_from(*lines.iter().max().unwrap())
        .map_err(|_| DecodeError::LimitExceeded)?
        .max(1);
    width
        .div_ceil(align)
        .checked_mul(align)
        .and_then(|width| width.checked_mul(height))
        .ok_or(DecodeError::LimitExceeded)
}

#[cfg(test)]
mod tests;
