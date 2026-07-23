use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{AudioChannelId, AudioConfiguration, ConfigGeneration, GainDb, PlaybackId};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PlayFileRequest {
    pub path: PathBuf,
    pub playback_id: PlaybackId,
    pub gain: GainDb,
}

impl PlayFileRequest {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>, playback_id: PlaybackId) -> Self {
        Self {
            path: path.into(),
            playback_id,
            gain: GainDb::UNITY,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[non_exhaustive]
pub enum AudioCommand {
    ApplyConfiguration {
        generation: ConfigGeneration,
        config: Box<AudioConfiguration>,
    },
    PlayFile(PlayFileRequest),
    StopFile {
        playback_id: PlaybackId,
    },
    StopAllFiles,
    SetMasterGain {
        gain: GainDb,
    },
    SetChannelGain {
        channel: AudioChannelId,
        gain: GainDb,
    },
    SetEnabled(bool),
    Shutdown,
}
