use std::{
    fs::File,
    path::{Path, PathBuf},
    sync::Arc,
};

use symphonia::{
    core::{
        codecs::audio::{AudioDecoder, AudioDecoderOptions},
        errors::Error as SymphoniaError,
        formats::{FormatOptions, FormatReader, TrackType, probe::Hint},
        io::{MediaSourceStream, MediaSourceStreamOptions},
        meta::MetadataOptions,
    },
    default::{get_codecs, get_probe},
};

use crate::{AudioError, AudioErrorCategory, SampleRate, assert_not_realtime};

use super::{
    AudioFileFormat, AudioSourceFingerprint, ResidentAssetKey, ResidentAudioAsset, asset::decode_error,
    audio_file_format_for_extension, resample::resample_planar,
};

#[derive(Clone, Debug, PartialEq)]
pub struct AudioFileProbe {
    pub path: PathBuf,
    pub format: Option<AudioFileFormat>,
    pub track: u32,
    pub channels: u16,
    pub sample_rate: SampleRate,
    pub source_frames: Option<u64>,
    pub duration_seconds: Option<f64>,
    pub source_bytes: u64,
}

impl AudioFileProbe {
    #[must_use]
    pub fn estimated_decoded_bytes(&self, destination_rate: SampleRate) -> Option<u64> {
        let source_frames = u128::from(self.source_frames?);
        let destination_frames = source_frames
            .saturating_mul(u128::from(destination_rate.get()))
            .saturating_add(u128::from(self.sample_rate.get()) / 2)
            / u128::from(self.sample_rate.get());
        let bytes = destination_frames
            .saturating_mul(u128::from(self.channels))
            .saturating_mul(std::mem::size_of::<f32>() as u128);
        u64::try_from(bytes).ok()
    }
}

pub fn probe_audio_file(path: impl AsRef<Path>) -> Result<AudioFileProbe, AudioError> {
    assert_not_realtime("audio file probing");
    let session = DecodingSession::open(path.as_ref())?;
    Ok(session.probe)
}

pub fn decode_audio_file(
    path: impl AsRef<Path>,
    engine_sample_rate: SampleRate,
) -> Result<Arc<ResidentAudioAsset>, AudioError> {
    assert_not_realtime("resident audio file decoding");
    let session = DecodingSession::open(path.as_ref())?;
    let source = AudioSourceFingerprint::from_path(path.as_ref())?;
    decode_session(session, source, engine_sample_rate)
}

pub(crate) fn decode_session(
    session: DecodingSession,
    source: AudioSourceFingerprint,
    engine_sample_rate: SampleRate,
) -> Result<Arc<ResidentAudioAsset>, AudioError> {
    decode_session_cancellable(session, source, engine_sample_rate, || false)?.ok_or_else(|| {
        AudioError::new(
            AudioErrorCategory::InternalInvariant,
            "uncancellable resident decode was cancelled",
        )
    })
}

pub(crate) fn decode_session_cancellable(
    mut session: DecodingSession,
    source: AudioSourceFingerprint,
    engine_sample_rate: SampleRate,
    mut is_cancelled: impl FnMut() -> bool,
) -> Result<Option<Arc<ResidentAudioAsset>>, AudioError> {
    let probe = session.probe.clone();
    let channels = usize::from(probe.channels);
    let reserve_frames = probe
        .source_frames
        .and_then(|frames| usize::try_from(frames).ok())
        .unwrap_or(0);
    let mut channel_samples = (0..channels)
        .map(|_| Vec::with_capacity(reserve_frames))
        .collect::<Vec<_>>();
    while !is_cancelled() {
        let Some(chunk) = session.next_planar_chunk()? else {
            break;
        };
        for (destination, source) in channel_samples.iter_mut().zip(chunk) {
            destination.extend_from_slice(&source);
        }
    }
    if is_cancelled() {
        return Ok(None);
    }
    let source_frames = channel_samples.first().map_or(0, Vec::len);
    if source_frames == 0 {
        return Err(decode_error("audio source decoded no frames", &probe.path));
    }
    let mut planar = Vec::with_capacity(channels.saturating_mul(source_frames));
    for channel in channel_samples {
        if channel.len() != source_frames {
            return Err(decode_error(
                "decoded audio channel lengths are inconsistent",
                &probe.path,
            ));
        }
        planar.extend_from_slice(&channel);
    }
    let (samples, frames) = resample_planar(&planar, channels, source_frames, probe.sample_rate, engine_sample_rate)?;
    let asset = ResidentAudioAsset::new(
        ResidentAssetKey {
            source,
            track: probe.track,
            engine_sample_rate,
        },
        probe.channels,
        frames,
        samples,
    )?;
    Ok(Some(Arc::new(asset)))
}

pub(crate) struct DecodingSession {
    pub probe: AudioFileProbe,
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
}

impl std::fmt::Debug for DecodingSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DecodingSession")
            .field("probe", &self.probe)
            .field("track_id", &self.track_id)
            .finish_non_exhaustive()
    }
}

impl DecodingSession {
    pub fn open(path: &Path) -> Result<Self, AudioError> {
        assert_not_realtime("audio decoder session creation");
        let file =
            File::open(path).map_err(|error| decode_error(format!("failed to open audio source: {error}"), path))?;
        let source_bytes = file.metadata().map(|metadata| metadata.len()).unwrap_or(0);
        let stream = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());
        let mut hint = Hint::new();
        if let Some(extension) = path.extension().and_then(|extension| extension.to_str()) {
            hint.with_extension(extension);
        }
        let format = get_probe()
            .probe(&hint, stream, FormatOptions::default(), MetadataOptions::default())
            .map_err(|error| symphonia_error("failed to probe audio source", path, error))?;
        let track = format.default_track(TrackType::Audio).ok_or_else(|| {
            AudioError::new(
                AudioErrorCategory::UnsupportedFormat,
                "audio source contains no decodable audio track",
            )
            .with_context("path", path.display().to_string())
        })?;
        let track_id = track.id;
        let codec_parameters = track
            .codec_params
            .as_ref()
            .and_then(|parameters| parameters.audio())
            .ok_or_else(|| {
                AudioError::new(
                    AudioErrorCategory::UnsupportedFormat,
                    "audio track has no usable codec parameters",
                )
                .with_context("path", path.display().to_string())
            })?;
        let sample_rate = SampleRate::new(
            codec_parameters
                .sample_rate
                .ok_or_else(|| decode_error("audio track does not declare a sample rate", path))?,
        )?;
        let channels = codec_parameters
            .channels
            .as_ref()
            .map_or(0, |channels| channels.count());
        let channels = u16::try_from(channels)
            .ok()
            .filter(|channels| *channels > 0)
            .ok_or_else(|| decode_error("audio track does not declare a supported channel count", path))?;
        let source_frames = track.num_frames;
        let duration_seconds = track.duration.zip(track.time_base).map(|(duration, time_base)| {
            duration.get() as f64 * f64::from(time_base.numer.get()) / f64::from(time_base.denom.get())
        });
        let decoder = get_codecs()
            .make_audio_decoder(codec_parameters, &AudioDecoderOptions::default())
            .map_err(|error| symphonia_error("failed to create audio decoder", path, error))?;
        let path = path.to_path_buf();
        let detected_format = path
            .extension()
            .and_then(|extension| extension.to_str())
            .and_then(audio_file_format_for_extension);
        Ok(Self {
            probe: AudioFileProbe {
                path,
                format: detected_format,
                track: track_id,
                channels,
                sample_rate,
                source_frames,
                duration_seconds,
                source_bytes,
            },
            format,
            decoder,
            track_id,
        })
    }

    pub fn next_planar_chunk(&mut self) -> Result<Option<Vec<Vec<f32>>>, AudioError> {
        loop {
            let packet = self
                .format
                .next_packet()
                .map_err(|error| symphonia_error("failed while reading audio packets", &self.probe.path, error))?;
            let Some(packet) = packet else {
                return Ok(None);
            };
            if packet.track_id != self.track_id {
                continue;
            }
            let decoded = self
                .decoder
                .decode(&packet)
                .map_err(|error| symphonia_error("failed to decode audio packet", &self.probe.path, error))?;
            if decoded.num_planes() != usize::from(self.probe.channels) {
                return Err(decode_error(
                    "decoded audio channel count changed during playback",
                    &self.probe.path,
                ));
            }
            let mut planes = Vec::new();
            decoded.copy_to_vecs_planar::<f32>(&mut planes);
            return Ok(Some(planes));
        }
    }
}

fn symphonia_error(context: &str, path: &Path, error: SymphoniaError) -> AudioError {
    let category = match error {
        SymphoniaError::Unsupported(_) => AudioErrorCategory::UnsupportedFormat,
        _ => AudioErrorCategory::DecodeFailed,
    };
    AudioError::new(category, format!("{context}: {error}")).with_context("path", path.display().to_string())
}
