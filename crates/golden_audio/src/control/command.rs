use std::{path::PathBuf, time::Duration};

use serde::{Deserialize, Serialize};

use crate::{AudioChannelId, AudioConfiguration, AudioDeviceTargetId, ConfigGeneration, GainDb, PlaybackId};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PlayFileRequest {
    pub path: PathBuf,
    pub playback_id: PlaybackId,
    pub gain: GainDb,
    #[serde(default)]
    pub start_offset: Duration,
    #[serde(default = "default_force_restart")]
    pub force_restart: bool,
}

impl PlayFileRequest {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>, playback_id: PlaybackId) -> Self {
        Self {
            path: path.into(),
            playback_id,
            gain: GainDb::UNITY,
            start_offset: Duration::ZERO,
            force_restart: default_force_restart(),
        }
    }

    #[must_use]
    pub fn with_gain(mut self, gain: GainDb) -> Self {
        self.gain = gain;
        self
    }

    #[must_use]
    pub fn with_start_offset(mut self, start_offset: Duration) -> Self {
        self.start_offset = start_offset;
        self
    }

    #[must_use]
    pub fn with_force_restart(mut self, force_restart: bool) -> Self {
        self.force_restart = force_restart;
        self
    }
}

const fn default_force_restart() -> bool {
    true
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
    /// Replaces the set of exact device targets whose capabilities should be
    /// materialized from the lightweight catalog.
    ///
    /// Hosts may make device probing expensive or exclusive. Callers should
    /// submit only targets currently selected in their editor or configuration.
    SetDeviceProbeInterests {
        targets: Vec<AudioDeviceTargetId>,
    },
    SetEnabled(bool),
    Shutdown,
}
