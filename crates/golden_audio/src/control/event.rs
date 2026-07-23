use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
    AudioBackendStatus, AudioError, AudioStreamStatus, ConfigGeneration, DiagnosticEvent, PlaybackId, VoiceId,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PlaybackInfo {
    pub playback_id: PlaybackId,
    pub path: PathBuf,
    pub voice: VoiceId,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackStopReason {
    Requested,
    Replaced,
    StopAll,
    ModuleDisabled,
    EndOfFile,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PlaybackStopInfo {
    pub playback_id: PlaybackId,
    pub voice: Option<VoiceId>,
    pub reason: PlaybackStopReason,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PlaybackFailure {
    pub playback_id: PlaybackId,
    pub path: PathBuf,
    pub error: AudioError,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[non_exhaustive]
pub enum AudioEvent {
    ConfigurationApplied {
        generation: ConfigGeneration,
    },
    ConfigurationRejected {
        generation: ConfigGeneration,
        error: AudioError,
    },
    BackendStatusChanged(AudioBackendStatus),
    DeviceStatusChanged(AudioStreamStatus),
    PlaybackStarted(PlaybackInfo),
    PlaybackFinished(PlaybackInfo),
    PlaybackStopped(PlaybackStopInfo),
    PlaybackFailed(PlaybackFailure),
    Diagnostic(DiagnosticEvent),
    ShutdownComplete,
}
