//! Metadata from one bounded, interruptible local demux context.
use super::{DecodeError, StreamTiming, ffmpeg, subtitles, timing::stream_timing, webp};
use std::{ffi::CStr, path::Path};

const MAX_TRACKS: usize = 64;
const MAX_TAG_CHARS: usize = 80;

/// One supported embedded audio stream, identified by its container stream index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioTrack {
    /// Exact stream index in the input, including other stream types.
    pub stream_index: usize,
    /// Optional human-readable name.
    pub title: Option<String>,
    /// Optional language identifier.
    pub language: Option<String>,
    /// FFmpeg codec name for an unnamed stream.
    pub codec: String,
}

/// Timing and track catalogs from the same input, before selecting a decoder.
#[derive(Debug)]
pub struct MediaMetadata {
    /// Timing of the selected video stream.
    pub video: StreamTiming,
    /// Supported embedded audio streams in container order.
    pub audio: Vec<AudioTrack>,
    /// Supported text subtitle streams in container order.
    pub subtitles: Vec<subtitles::SubtitleTrack>,
}

/// Inspect a local media file without preparing PCM or video frames.
pub fn probe_media(
    path: &Path,
    interrupt: impl FnMut() -> bool + Send + 'static,
) -> Result<MediaMetadata, DecodeError> {
    let (input, animation) = webp::open_video(path, interrupt)?;
    let video = stream_timing(&input, animation.as_ref())?;
    let audio = input
        .streams()
        .filter(|stream| supported_audio(stream))
        .take(MAX_TRACKS)
        .map(|stream| {
            let metadata = stream.metadata();
            AudioTrack {
                stream_index: stream.index(),
                title: tag(&metadata, c"title"),
                language: tag(&metadata, c"language"),
                codec: stream.parameters().id().name().into(),
            }
        })
        .collect();
    Ok(MediaMetadata {
        video,
        audio,
        subtitles: subtitles::tracks(&input),
    })
}

pub(super) fn open_local(
    path: &Path,
    interrupt: impl FnMut() -> bool + Send + 'static,
) -> Result<ffmpeg::format::context::Input, DecodeError> {
    if !path.is_file() {
        return Err(DecodeError::NotAFile);
    }
    ffmpeg::init()?;
    let mut options = ffmpeg::Dictionary::new();
    options.set("protocol_whitelist", "file");
    Ok(ffmpeg::format::input_with_interrupt_and_dictionary(
        path, interrupt, options,
    )?)
}

pub(super) fn supported_audio(stream: &ffmpeg::Stream<'_>) -> bool {
    let parameters = stream.parameters();
    let base = stream.time_base();
    parameters.medium() == ffmpeg::media::Type::Audio
        && base.numerator() > 0
        && base.denominator() > 0
        && ffmpeg::decoder::find(parameters.id()).is_some()
}

pub(super) fn raw_tag<'a>(metadata: &'a ffmpeg::dictionary::Ref<'_>, key: &CStr) -> Option<&'a CStr> {
    // SAFETY: the entry and its NUL-terminated value borrow FFmpeg's live stream dictionary.
    let entry = unsafe { ffmpeg::ffi::av_dict_get(metadata.as_ptr(), key.as_ptr(), std::ptr::null(), 0).as_ref()? };
    (!entry.value.is_null()).then(|| unsafe { CStr::from_ptr(entry.value) })
}

pub(super) fn tag(metadata: &ffmpeg::dictionary::Ref<'_>, key: &CStr) -> Option<String> {
    let raw = raw_tag(metadata, key)?.to_str().ok()?;
    let clean = raw
        .trim()
        .chars()
        .filter(|ch| !ch.is_control())
        .take(MAX_TAG_CHARS)
        .collect::<String>();
    (!clean.is_empty()).then_some(clean)
}
