use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum VideoPlaybackMode {
    Playing,
    #[default]
    Paused,
    Stopped,
}
impl VideoPlaybackMode {
    pub fn name(self) -> &'static str {
        match self {
            Self::Playing => "Playing",
            Self::Paused => "Paused",
            Self::Stopped => "Stopped",
        }
    }
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "Playing" => Ok(Self::Playing),
            "Paused" => Ok(Self::Paused),
            "Stopped" => Ok(Self::Stopped),
            _ => Err("unknown Video playback mode".into()),
        }
    }
}

/// Read projection of real Media Parameters; never an opaque persistence record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct VideoTransportSettings {
    pub mode: VideoPlaybackMode,
    pub position_ms: f64,
    pub rate: f64,
    pub looping: bool,
}
impl Default for VideoTransportSettings {
    fn default() -> Self {
        Self {
            mode: VideoPlaybackMode::Paused,
            position_ms: 0.0,
            rate: 1.0,
            looping: false,
        }
    }
}
impl VideoTransportSettings {
    pub fn validate(&self) -> Result<(), String> {
        if !self.position_ms.is_finite() || !(0.0..=9_000_000_000_000.0).contains(&self.position_ms) {
            return Err("Video seek position must be finite milliseconds in 0..9000000000000".into());
        }
        if !self.rate.is_finite() || !(-16.0..=16.0).contains(&self.rate) {
            return Err("Video rate must be finite in -16..16".into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum VideoStepDirection {
    Previous,
    Next,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
pub enum VideoEdit {
    Play,
    Pause,
    Stop,
    Seek {
        position_ms: f64,
    },
    Rate {
        rate: f64,
    },
    Loop {
        looping: bool,
    },
    Step {
        channel: super::super::protocol::ChannelId,
        direction: VideoStepDirection,
    },
}

/// Runtime readiness is separate from accepted authored playback intent.
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct VideoTransportSnapshot {
    pub media: super::super::protocol::MediaId,
    pub settings: VideoTransportSettings,
    pub ready: bool,
    pub position_ms: f64,
    pub playing: bool,
}

/// Prepared selected-channel timestamps, read without opening or waiting for a file.
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct VideoFrameState {
    pub reference: super::super::protocol::ChannelReference,
    pub position_ms: f64,
    pub previous_ms: Option<f64>,
    pub previous_ready: bool,
    pub next_ms: Option<f64>,
}

/// Source readiness is independent of transport intent and renderer/device readiness.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum VideoSourceState {
    Unconfigured,
    Loading,
    Prepared,
    Ready,
    Unavailable,
    Recovering,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum VideoSourceErrorKind {
    Missing,
    Unsupported,
    Decode,
    Capacity,
    Io,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct VideoSourceError {
    pub kind: VideoSourceErrorKind,
    pub message: String,
}
impl VideoSourceError {
    pub(crate) fn new(kind: VideoSourceErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into().chars().take(256).collect(),
        }
    }
}

/// A bounded read of one actual channel. Prepared means timing only, not decoded pixels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct VideoSourceStatus {
    pub reference: super::super::protocol::ChannelReference,
    pub state: VideoSourceState,
    pub duration_ms: Option<f64>,
    pub error: Option<VideoSourceError>,
    pub retrying: bool,
}
impl VideoSourceStatus {
    pub(crate) fn preview_message(&self) -> Option<String> {
        if let Some(error) = &self.error {
            let retry = if self.retrying { " Retrying automatically." } else { "" };
            return Some(format!("{}{retry}", error.message));
        }
        match self.state {
            VideoSourceState::Unconfigured => Some("No Video File assigned".into()),
            VideoSourceState::Loading => Some("Loading Video source".into()),
            _ => None,
        }
    }
}
