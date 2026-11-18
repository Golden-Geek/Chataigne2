//! Worker-owned FFmpeg demux and bounded, immutable plain-text subtitle timelines.
use super::{DecodeError, StreamTiming, TimeBase, ffmpeg, timing::stream_timing};
use std::{
    ffi::CStr,
    fs::File,
    io::{Cursor, Read},
    path::Path,
    time::Duration,
};

mod text;
mod timeline;
pub use timeline::{SubtitleCue, SubtitleTimeline};

const MAX_TRACKS: usize = 64;
const MAX_TAG_CHARS: usize = 80;
const MAX_LOCAL_BYTES: u64 = 8 * 1024 * 1024;
const MAX_PACKET_BYTES: usize = 64 * 1024;
const MAX_SCAN_PACKETS: usize = 2_000_000;

/// Text codecs whose decoded dialogue can be shown using caller-owned typography.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubtitleCodec {
    /// SubRip text and line breaks.
    SubRip,
    /// WebVTT cue text. Authored styling and positioning are discarded.
    WebVtt,
    /// Advanced SubStation Alpha dialogue. Styling and vector drawing are discarded.
    Ass,
    /// MPEG-4 Timed Text, commonly embedded in MP4/MOV.
    MovText,
}

impl SubtitleCodec {
    /// Short display label suitable for a track catalog.
    pub const fn name(self) -> &'static str {
        match self {
            Self::SubRip => "SubRip",
            Self::WebVtt => "WebVTT",
            Self::Ass => "ASS",
            Self::MovText => "MovText",
        }
    }

    fn from_id(id: ffmpeg::codec::Id) -> Option<Self> {
        use ffmpeg::codec::Id;
        let codec = match id {
            Id::SUBRIP => Self::SubRip,
            Id::WEBVTT => Self::WebVtt,
            Id::ASS => Self::Ass,
            Id::MOV_TEXT => Self::MovText,
            _ => return None,
        };
        ffmpeg::decoder::find(id).map(|_| codec)
    }
}

/// One supported embedded text stream. An authored choice can use its exact stream index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubtitleTrack {
    /// Index in the original media container, including its video/audio streams.
    pub stream_index: usize,
    /// Optional language tag, cleaned for display.
    pub language: Option<String>,
    /// Optional stream title, cleaned for display.
    pub title: Option<String>,
    /// Supported decoder class.
    pub codec: SubtitleCodec,
}

/// Failures are isolated to the selected subtitle source and can be retried by its worker.
#[derive(Debug, thiserror::Error)]
pub enum SubtitleError {
    /// The path does not resolve to a regular file.
    #[error("subtitle source is not an existing regular file")]
    NotAFile,
    /// The local file is not a supported format or the requested embedded index is absent.
    #[error("unsupported subtitle format or unavailable text track")]
    Unsupported,
    /// Subtitle dialogue could not be decoded as UTF-8 text.
    #[error("subtitle text is not valid UTF-8 dialogue")]
    InvalidText,
    /// The subtitle stream did not describe a positive, representable presentation interval.
    #[error("invalid subtitle timestamp or duration")]
    InvalidTime,
    /// Parsing, storage, packet search or simultaneously active cue admission was exceeded.
    #[error("subtitle admission limit exceeded")]
    LimitExceeded,
    /// A local file could not be read.
    #[error("subtitle file: {0}")]
    Io(#[from] std::io::Error),
    /// The selected media does not expose valid video timing.
    #[error(transparent)]
    VideoTiming(#[from] super::DecodeError),
    /// The decoder, demuxer or cancellation callback stopped subtitle loading.
    #[error("subtitle FFmpeg: {0}")]
    Ffmpeg(#[from] ffmpeg::Error),
}

/// Catalog at most 64 supported text streams in container order, without decoding cues.
pub fn probe_tracks(
    path: &Path,
    cancelled: impl FnMut() -> bool + Send + 'static,
) -> Result<Vec<SubtitleTrack>, SubtitleError> {
    let input = open_media(path, cancelled)?;
    Ok(tracks(&input))
}

/// Read video duration and supported subtitle streams from a single container probe.
pub fn probe_timing_and_tracks(
    path: &Path,
    cancelled: impl FnMut() -> bool + Send + 'static,
) -> Result<(StreamTiming, Vec<SubtitleTrack>), DecodeError> {
    let input = open_media(path, cancelled)?;
    Ok((stream_timing(&input)?, tracks(&input)))
}

/// Decode one exact embedded text stream on a file worker. Cue times are relative to the
/// video stream's origin in `video_time_base`, before any playback cropping or looping.
pub fn load_embedded(
    path: &Path,
    stream_index: usize,
    video_origin_ticks: i64,
    video_time_base: TimeBase,
    cancelled: impl FnMut() -> bool + Send + 'static,
) -> Result<SubtitleTimeline, SubtitleError> {
    let origin = ScaledTime::new(i128::from(video_origin_ticks), video_time_base)?;
    read_cues(open_media(path, cancelled)?, stream_index, origin)
}

/// Decode a UTF-8 `.srt` or `.vtt` sidecar, including UTF-8 BOM. Its clock starts at source zero.
pub fn load_local(
    path: &Path,
    cancelled: impl FnMut() -> bool + Send + 'static,
) -> Result<SubtitleTimeline, SubtitleError> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    let (filename, format, codec) = if extension.eq_ignore_ascii_case("srt") {
        ("subtitles.srt", "srt", SubtitleCodec::SubRip)
    } else if extension.eq_ignore_ascii_case("vtt") {
        ("subtitles.vtt", "webvtt", SubtitleCodec::WebVtt)
    } else {
        return Err(SubtitleError::Unsupported);
    };
    if !path.is_file() {
        return Err(SubtitleError::NotAFile);
    }
    let mut data = Vec::new();
    File::open(path)?.take(MAX_LOCAL_BYTES + 1).read_to_end(&mut data)?;
    if data.len() as u64 > MAX_LOCAL_BYTES {
        return Err(SubtitleError::LimitExceeded);
    }
    if std::str::from_utf8(&data).is_err() || data.contains(&0) {
        return Err(SubtitleError::InvalidText);
    }
    ffmpeg::init()?;
    let stream = ffmpeg::format::context::StreamIo::from_read_seek(Cursor::new(data))?;
    let mut options = ffmpeg::Dictionary::new();
    options.set("format_whitelist", format);
    let input = ffmpeg::format::input_from_stream_with_interrupt(stream, Some(filename), Some(options), cancelled)?;
    let index = input
        .streams()
        .find(|stream| SubtitleCodec::from_id(stream.parameters().id()) == Some(codec))
        .ok_or(SubtitleError::Unsupported)?
        .index();
    read_cues(input, index, ScaledTime::ZERO)
}

fn open_media(
    path: &Path,
    cancelled: impl FnMut() -> bool + Send + 'static,
) -> Result<ffmpeg::format::context::Input, DecodeError> {
    if !path.is_file() {
        return Err(DecodeError::NotAFile);
    }
    ffmpeg::init()?;
    let mut options = ffmpeg::Dictionary::new();
    options.set("protocol_whitelist", "file");
    Ok(ffmpeg::format::input_with_interrupt_and_dictionary(
        path, cancelled, options,
    )?)
}

fn tracks(input: &ffmpeg::format::context::Input) -> Vec<SubtitleTrack> {
    input
        .streams()
        .filter_map(|stream| {
            let codec = SubtitleCodec::from_id(stream.parameters().id())?;
            let metadata = stream.metadata();
            Some(SubtitleTrack {
                stream_index: stream.index(),
                language: tag(&metadata, c"language"),
                title: tag(&metadata, c"title"),
                codec,
            })
        })
        .take(MAX_TRACKS)
        .collect()
}

fn tag(metadata: &ffmpeg::dictionary::Ref<'_>, key: &CStr) -> Option<String> {
    // SAFETY: the entry and its NUL-terminated value borrow FFmpeg's live stream dictionary.
    // Check UTF-8 before creating a Rust str and copy only bounded display text.
    let entry = unsafe { ffmpeg::ffi::av_dict_get(metadata.as_ptr(), key.as_ptr(), std::ptr::null(), 0).as_ref()? };
    if entry.value.is_null() {
        return None;
    }
    let raw = unsafe { CStr::from_ptr(entry.value) }.to_str().ok()?;
    let clean = raw
        .trim()
        .chars()
        .filter(|ch| !ch.is_control())
        .take(MAX_TAG_CHARS)
        .collect::<String>();
    (!clean.is_empty()).then_some(clean)
}

fn read_cues(
    mut input: ffmpeg::format::context::Input,
    stream_index: usize,
    origin: ScaledTime,
) -> Result<SubtitleTimeline, SubtitleError> {
    let stream = input.stream(stream_index).ok_or(SubtitleError::Unsupported)?;
    SubtitleCodec::from_id(stream.parameters().id()).ok_or(SubtitleError::Unsupported)?;
    let base = stream.time_base();
    let time_base = TimeBase {
        numerator: u32::try_from(base.numerator()).map_err(|_| SubtitleError::InvalidTime)?,
        denominator: u32::try_from(base.denominator()).map_err(|_| SubtitleError::InvalidTime)?,
    };
    let mut decoder = ffmpeg::codec::context::Context::from_parameters(stream.parameters())?.decoder();
    decoder.set_packet_time_base(base);
    let mut decoder = decoder.subtitle()?;
    let mut cues = timeline::CueBuilder::default();
    for _ in 0..MAX_SCAN_PACKETS {
        let mut packet = ffmpeg::Packet::empty();
        match packet.read(&mut input) {
            Ok(()) => {}
            Err(ffmpeg::Error::Eof) => return cues.finish(),
            Err(error) => return Err(error.into()),
        }
        if packet.stream() != stream_index || packet.size() == 0 {
            continue;
        }
        if packet.size() > MAX_PACKET_BYTES {
            return Err(SubtitleError::LimitExceeded);
        }
        let mut output = text::SubtitleOutput(ffmpeg::Subtitle::new());
        if decoder.decode(&packet, &mut output.0)?
            && let Some(text) = output.plain_dialogue()?
            && let Some((start, end)) = cue_interval(&packet, &output.0, time_base, origin)?
        {
            cues.push(SubtitleCue { start, end, text })?;
        }
    }
    Err(SubtitleError::LimitExceeded)
}

fn cue_interval(
    packet: &ffmpeg::Packet,
    subtitle: &ffmpeg::Subtitle,
    time_base: TimeBase,
    origin: ScaledTime,
) -> Result<Option<(Duration, Duration)>, SubtitleError> {
    let pts = i128::from(packet.pts().ok_or(SubtitleError::InvalidTime)?);
    let packet_start = ScaledTime::new(pts, time_base)?.relative_to(origin);
    let start = packet_start + i128::from(subtitle.start()) * 1_000_000;
    // The packet retains the exact stream time base. The decoder display offset is
    // only used for streams missing a packet duration, avoiding millisecond rounding.
    let end = if packet.duration() > 0 {
        ScaledTime::new(pts + i128::from(packet.duration()), time_base)?.relative_to(origin)
    } else {
        packet_start + i128::from(subtitle.end()) * 1_000_000
    };
    if end <= 0 {
        return Ok(None);
    }
    if start >= end {
        return Err(SubtitleError::InvalidTime);
    }
    let nanos = |value| {
        i64::try_from(value)
            .map(|value| Duration::from_nanos(value as u64))
            .map_err(|_| SubtitleError::InvalidTime)
    };
    Ok(Some((nanos(start.max(0))?, nanos(end)?)))
}

/// Exact signed time split before comparing different stream time bases. Only the small
/// fractional remainders are brought to a common denominator, so no huge tick products.
#[derive(Clone, Copy)]
struct ScaledTime {
    nanos: i128,
    remainder: i128,
    denominator: i128,
}

impl ScaledTime {
    const ZERO: Self = Self {
        nanos: 0,
        remainder: 0,
        denominator: 1,
    };

    fn new(ticks: i128, time_base: TimeBase) -> Result<Self, SubtitleError> {
        let denominator = i128::from(time_base.denominator);
        if time_base.numerator == 0 || denominator == 0 {
            return Err(SubtitleError::InvalidTime);
        }
        let numerator = ticks
            .checked_mul(i128::from(time_base.numerator) * 1_000_000_000)
            .ok_or(SubtitleError::InvalidTime)?;
        Ok(Self {
            nanos: numerator.div_euclid(denominator),
            remainder: numerator.rem_euclid(denominator),
            denominator,
        })
    }

    fn relative_to(self, origin: Self) -> i128 {
        // First representable nanosecond on/after the exact boundary. This convention
        // makes the same half-open range valid for forward, reverse, and paused seeks.
        self.nanos - origin.nanos
            + i128::from(self.remainder * origin.denominator > origin.remainder * self.denominator)
    }
}

#[cfg(test)]
mod tests;
