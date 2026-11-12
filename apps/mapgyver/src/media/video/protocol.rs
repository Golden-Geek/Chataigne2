use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum VideoPlaybackMode {
    Playing,
    #[default]
    Paused,
    Stopped,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum VideoOnActivate {
    #[default]
    PlayFromStart,
    Resume,
    Nothing,
}
impl VideoOnActivate {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "PlayFromStart" => Ok(Self::PlayFromStart),
            "Resume" => Ok(Self::Resume),
            "Nothing" => Ok(Self::Nothing),
            _ => Err("unknown Video On Activate policy".into()),
        }
    }
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::PlayFromStart => "PlayFromStart",
            Self::Resume => "Resume",
            Self::Nothing => "Nothing",
        }
    }
    pub(crate) fn options() -> Vec<golden_core::parameter::ParameterEnumOption> {
        let mut options = crate::parameters::enum_options(&["PlayFromStart", "Resume", "Nothing"]);
        options[0].label = "Play from Start".into();
        options
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum VideoOnDeactivate {
    #[default]
    Stop,
    Pause,
    Nothing,
}
impl VideoOnDeactivate {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "Stop" => Ok(Self::Stop),
            "Pause" => Ok(Self::Pause),
            "Nothing" => Ok(Self::Nothing),
            _ => Err("unknown Video On Deactivate policy".into()),
        }
    }
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Stop => "Stop",
            Self::Pause => "Pause",
            Self::Nothing => "Nothing",
        }
    }
}
/// Authored transport options and current runtime intent; never an opaque persistence record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct VideoTransportSettings {
    pub mode: VideoPlaybackMode,
    pub position_ms: f64,
    pub speed: f64,
    pub looping: bool,
    pub play_at_load: bool,
    pub on_activate: VideoOnActivate,
    pub on_deactivate: VideoOnDeactivate,
}
impl Default for VideoTransportSettings {
    fn default() -> Self {
        Self {
            mode: VideoPlaybackMode::Paused,
            position_ms: 0.0,
            speed: 1.0,
            looping: false,
            play_at_load: false,
            on_activate: VideoOnActivate::default(),
            on_deactivate: VideoOnDeactivate::default(),
        }
    }
}
impl VideoTransportSettings {
    pub fn validate(&self) -> Result<(), String> {
        if !self.position_ms.is_finite() || !(0.0..=9_000_000_000_000.0).contains(&self.position_ms) {
            return Err("Video seek position must be finite milliseconds in 0..9000000000000".into());
        }
        if !self.speed.is_finite() || !(-16.0..=16.0).contains(&self.speed) {
            return Err("Video speed must be finite in -16..16".into());
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
    Speed {
        speed: f64,
    },
    Loop {
        looping: bool,
    },
    PlayAtLoad {
        enabled: bool,
    },
    OnActivate {
        policy: VideoOnActivate,
    },
    OnDeactivate {
        policy: VideoOnDeactivate,
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
