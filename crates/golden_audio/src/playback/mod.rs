//! Backend-neutral file playback, cache, scheduling, and callback-safe voice mixing.

mod asset;
mod cache;
#[cfg(feature = "playback")]
mod decoder;
mod format;
#[cfg(feature = "playback")]
mod resample;
#[cfg(feature = "playback")]
mod scheduler;
mod streaming;
mod voice;

pub use asset::{AudioSourceFingerprint, ResidentAssetKey, ResidentAudioAsset};
pub use cache::{AssetCache, CacheInsertResult, CacheObservation};
#[cfg(feature = "playback")]
pub use decoder::{AudioFileProbe, decode_audio_file, probe_audio_file};
pub use format::{
    AudioFileFormat, AudioFileFormatDescriptor, audio_file_format_for_extension, supported_audio_extensions,
    supported_audio_formats,
};
#[cfg(feature = "playback")]
pub use scheduler::{
    PlaybackPreparation, PlaybackPreparationFailure, PlaybackPreparationResult, PlaybackScheduler,
    PlaybackSchedulerConfig, PlaybackSchedulerRequest,
};
pub use streaming::{
    StreamPlaybackReader, StreamPlaybackState, StreamPlaybackWriter, StreamWriteError, StreamWriteResult,
    streaming_playback_ring,
};
pub use voice::{
    DefaultPlaybackRoute, PlaybackRenderEvent, PlaybackRenderMetrics, PlaybackRendererRetirement, PlaybackVoice,
    PlaybackVoiceController, PlaybackVoiceRenderer, PlaybackVoiceSource, default_playback_routes, playback_voice_pool,
};

#[cfg(test)]
mod tests;
