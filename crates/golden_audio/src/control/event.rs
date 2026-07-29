use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
    AudioBackendStatus, AudioError, AudioStreamStatus, CommandSequence, ConfigGeneration, DiagnosticEvent, PlaybackId,
    VoiceId,
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioQueueKind {
    Command,
    Event,
    PlanPublish,
    PlanReturn,
    RealtimeControl,
    VoiceReturn,
    AnalysisFree,
    AnalysisReady,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct QueuePressureEvent {
    pub queue: AudioQueueKind,
    pub occurrences: u64,
    pub capacity: usize,
}

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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackCommandKind {
    PlayFile,
    StopFile,
    StopAllFiles,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackCommandIgnoredReason {
    ForceRestartDisabled,
    PlaybackIdNotFound,
    NoPlaybacks,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlaybackCommandIgnored {
    PlayFileAlreadyActive {
        sequence: CommandSequence,
        playback_id: PlaybackId,
        path: PathBuf,
    },
    StopFileNotFound {
        sequence: CommandSequence,
        playback_id: PlaybackId,
    },
    StopAllFilesEmpty {
        sequence: CommandSequence,
    },
}

impl PlaybackCommandIgnored {
    #[must_use]
    pub fn play_file(sequence: CommandSequence, playback_id: PlaybackId, path: PathBuf) -> Self {
        Self::PlayFileAlreadyActive {
            sequence,
            playback_id,
            path,
        }
    }

    #[must_use]
    pub fn stop_file(sequence: CommandSequence, playback_id: PlaybackId) -> Self {
        Self::StopFileNotFound { sequence, playback_id }
    }

    #[must_use]
    pub const fn stop_all_files(sequence: CommandSequence) -> Self {
        Self::StopAllFilesEmpty { sequence }
    }

    #[must_use]
    pub const fn sequence(&self) -> CommandSequence {
        match self {
            Self::PlayFileAlreadyActive { sequence, .. }
            | Self::StopFileNotFound { sequence, .. }
            | Self::StopAllFilesEmpty { sequence } => *sequence,
        }
    }

    #[must_use]
    pub const fn command(&self) -> PlaybackCommandKind {
        match self {
            Self::PlayFileAlreadyActive { .. } => PlaybackCommandKind::PlayFile,
            Self::StopFileNotFound { .. } => PlaybackCommandKind::StopFile,
            Self::StopAllFilesEmpty { .. } => PlaybackCommandKind::StopAllFiles,
        }
    }

    #[must_use]
    pub const fn reason(&self) -> PlaybackCommandIgnoredReason {
        match self {
            Self::PlayFileAlreadyActive { .. } => PlaybackCommandIgnoredReason::ForceRestartDisabled,
            Self::StopFileNotFound { .. } => PlaybackCommandIgnoredReason::PlaybackIdNotFound,
            Self::StopAllFilesEmpty { .. } => PlaybackCommandIgnoredReason::NoPlaybacks,
        }
    }

    #[must_use]
    pub fn playback_id(&self) -> Option<&PlaybackId> {
        match self {
            Self::PlayFileAlreadyActive { playback_id, .. } | Self::StopFileNotFound { playback_id, .. } => {
                Some(playback_id)
            }
            Self::StopAllFilesEmpty { .. } => None,
        }
    }

    #[must_use]
    pub fn path(&self) -> Option<&PathBuf> {
        match self {
            Self::PlayFileAlreadyActive { path, .. } => Some(path),
            Self::StopFileNotFound { .. } | Self::StopAllFilesEmpty { .. } => None,
        }
    }
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
    DeviceInventoryChanged {
        revision: u64,
    },
    DeviceStatusChanged(AudioStreamStatus),
    PlaybackStarted(PlaybackInfo),
    PlaybackFinished(PlaybackInfo),
    PlaybackStopped(PlaybackStopInfo),
    PlaybackFailed(PlaybackFailure),
    PlaybackCommandIgnored(PlaybackCommandIgnored),
    QueuePressure(QueuePressureEvent),
    Diagnostic(DiagnosticEvent),
    ShutdownComplete,
}
