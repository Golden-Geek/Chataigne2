use serde::{Deserialize, Serialize};
use std::time::Duration;
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
    /// Crop-relative milliseconds.
    pub position_ms: f64,
    /// Authored source time in milliseconds.
    pub start_ms: f64,
    /// Authored exclusive source end in milliseconds. None follows the source end.
    pub end_ms: Option<f64>,
    pub speed: f64,
    pub looping: bool,
    /// Source time of the repeat entry. Zero follows the crop start.
    pub loop_start_ms: f64,
    /// Authored duration in milliseconds. Loop off keeps the value for later.
    pub auto_smooth_loop_ms: f64,
    pub play_at_load: bool,
    pub on_activate: VideoOnActivate,
    pub on_deactivate: VideoOnDeactivate,
}
impl Default for VideoTransportSettings {
    fn default() -> Self {
        Self {
            mode: VideoPlaybackMode::Paused,
            position_ms: 0.0,
            start_ms: 0.0,
            end_ms: None,
            speed: 1.0,
            looping: false,
            loop_start_ms: 0.0,
            auto_smooth_loop_ms: 0.0,
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
        if !self.auto_smooth_loop_ms.is_finite() || !(0.0..=9_000_000_000_000.0).contains(&self.auto_smooth_loop_ms) {
            return Err("Video Auto-Smooth Loop must be finite milliseconds in 0..9000000000000".into());
        }
        if !(0.0..=9_000_000_000_000.0).contains(&self.loop_start_ms) {
            return Err("Video Loop Start must be finite milliseconds in 0..9000000000000".into());
        }
        if !(0.0..=9_000_000_000_000.0).contains(&self.start_ms) {
            return Err("Video Start must be finite milliseconds in 0..9000000000000".into());
        }
        if self.end_ms.is_some_and(|end| {
            !(0.0..=9_000_000_000_000.0).contains(&end)
                || Duration::from_secs_f64(end / 1000.0) <= Duration::from_secs_f64(self.start_ms / 1000.0)
        }) {
            return Err("Video End must be finite milliseconds after Start and at most 9000000000000".into());
        }
        Ok(())
    }

    pub(crate) fn validate_crop_start_change(
        &self,
        previous_start_ms: f64,
        source_duration_ms: Option<f64>,
    ) -> Result<(), String> {
        let Some(duration) = source_duration_ms.filter(|duration| *duration > 0.0) else {
            return Ok(());
        };
        // Relinking may shorten the effective range while preserving the authored crop.
        // Only bound a new Start, leaving an already authored range editable after relinking.
        if self.start_ms != previous_start_ms && self.start_ms >= duration {
            return Err("Video Start must precede the end of the available source".into());
        }
        Ok(())
    }

    pub(crate) fn validate_loop_start_change(
        &self,
        previous: f64,
        source_duration_ms: Option<f64>,
    ) -> Result<(), String> {
        if self.loop_start_ms == previous || self.loop_start_ms == 0.0 {
            return Ok(());
        }
        let start = Duration::try_from_secs_f64(self.loop_start_ms / 1_000.0)
            .map_err(|_| "Video Loop Start must be finite and nonnegative")?;
        let end = [self.end_ms, source_duration_ms]
            .into_iter()
            .flatten()
            .filter(|item| *item > 0.0)
            .reduce(f64::min);
        if end.is_some_and(|end| Duration::from_secs_f64(end / 1_000.0) <= start) {
            return Err("Video Loop Start must precede the available End".into());
        }
        Ok(())
    }
}

pub(crate) fn crop_end_from_parameter(end_ms: f64) -> Option<f64> {
    (end_ms != 0.0).then_some(end_ms)
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
    Crop {
        start_ms: f64,
        end_ms: Option<f64>,
    },
    Speed {
        speed: f64,
    },
    Loop {
        looping: bool,
    },
    LoopStart {
        start_ms: f64,
    },
    AutoSmoothLoop {
        duration_ms: f64,
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

/// Exact source choice for one Video channel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
pub enum VideoSubtitleTrack {
    #[default]
    None,
    File,
    Embedded {
        index: u32,
    },
}

impl VideoSubtitleTrack {
    pub(crate) fn key(self) -> String {
        match self {
            Self::None => "None".into(),
            Self::File => "File".into(),
            Self::Embedded { index } => format!("Embedded:{index}"),
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "None" => Ok(Self::None),
            "File" => Ok(Self::File),
            value => {
                let index = value
                    .strip_prefix("Embedded:")
                    .and_then(|index| index.parse().ok())
                    .ok_or("Unknown Video subtitle track")?;
                let track = Self::Embedded { index };
                if track.key() != value {
                    return Err("Invalid Video subtitle stream index".into());
                }
                Ok(track)
            }
        }
    }
}

/// The selected stream number remains exact when discovery changes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct VideoSubtitleTrackInfo {
    pub index: u32,
    pub label: String,
}

pub(crate) fn subtitle_track_options(
    tracks: &[VideoSubtitleTrackInfo],
) -> golden_core::parameter::ParameterEnumOptions {
    let mut choices = crate::parameters::enum_options(&["None", "File"]);
    choices[1].label = "Local File".into();
    choices.extend(tracks.iter().map(|track| {
        let key = VideoSubtitleTrack::Embedded { index: track.index }.key();
        golden_core::parameter::ParameterEnumOption {
            variant_id: key.clone(),
            value: golden_core::parameter::ParamValue::Enum(key),
            label: track.label.clone(),
            tags: vec![],
            ordering: None,
        }
    }));
    choices.into()
}

/// Projection of the typed Subtitles folder; not used as a persistence record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct VideoSubtitleSettings {
    pub enabled: bool,
    pub track: VideoSubtitleTrack,
    /// Empty, project-relative, or native absolute local file path.
    pub file: String,
    /// Font size as a percentage of source height.
    pub size: f64,
    pub font: String,
    /// Normalized horizontal center and bottom edge, with the origin at top left.
    pub position: [f64; 2],
}

impl Default for VideoSubtitleSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            track: VideoSubtitleTrack::None,
            file: String::new(),
            size: 5.0,
            font: "sans-serif".into(),
            position: [0.5, 0.9],
        }
    }
}

impl VideoSubtitleSettings {
    pub(crate) fn validate(&self) -> Result<(), String> {
        super::super::assets::parse_file(&self.file)?;
        if !self.file.is_empty()
            && !std::path::Path::new(&self.file)
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("srt") || extension.eq_ignore_ascii_case("vtt"))
        {
            return Err("Video subtitle File must be an .srt or .vtt file".into());
        }
        if !(1.0..=25.0).contains(&self.size) {
            return Err("Video subtitle Size must be 1–25 percent of source height".into());
        }
        if self.font.trim().is_empty() || self.font.chars().count() > 128 || self.font.chars().any(char::is_control) {
            return Err("Video subtitle Font must contain a family name of at most 128 characters".into());
        }
        if self.position.iter().any(|value| !(0.0..=1.0).contains(value)) {
            return Err("Video subtitle Positioning must be normalized in each axis".into());
        }
        Ok(())
    }
}

/// Runtime readiness is separate from accepted authored playback intent.
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct VideoTransportSnapshot {
    pub media: super::super::protocol::MediaId,
    pub settings: VideoTransportSettings,
    pub ready: bool,
    pub position_ms: f64,
    /// Effective crop length, once a source duration is available.
    pub length_ms: Option<f64>,
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
