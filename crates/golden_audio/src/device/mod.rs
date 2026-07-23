use serde::{Deserialize, Serialize};

use crate::{AudioDeviceId, AudioError, AudioErrorCategory, BackendId, FrameCount, PhysicalChannelKey, SampleRate};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
pub enum AudioDirection {
    Input,
    Output,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
pub enum AudioDeviceTargetId {
    SystemDefault { backend: BackendId },
    Device { backend: BackendId, device: AudioDeviceId },
}

impl AudioDeviceTargetId {
    #[must_use]
    pub fn backend(&self) -> &BackendId {
        match self {
            Self::SystemDefault { backend } | Self::Device { backend, .. } => backend,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
pub enum AudioRecoveryPolicy {
    #[default]
    WaitForSelected,
    FollowSystemDefault,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "frames", rename_all = "snake_case")]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
pub enum AudioBufferPolicy {
    #[default]
    Automatic,
    Fixed(u32),
}

impl AudioBufferPolicy {
    pub fn validate(self) -> Result<(), AudioError> {
        if let Self::Fixed(frames) = self {
            FrameCount::new(frames)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
pub enum AudioSampleFormat {
    F32,
    F64,
    I16,
    I24,
    I32,
    U16,
    U24,
    U32,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
pub enum AudioPermissionState {
    NotRequired,
    #[default]
    Unknown,
    Granted,
    Denied,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
pub enum AudioBackendState {
    #[default]
    Compiled,
    Available,
    Unavailable,
    MissingServer,
    MissingDriver,
    Failed,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
pub enum AudioDeviceReadiness {
    Disabled,
    #[default]
    Discovering,
    Missing,
    Unavailable,
    Busy,
    PermissionDenied,
    Ready,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
pub struct PhysicalChannelDescriptor {
    pub key: PhysicalChannelKey,
    pub label: String,
    pub position: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
pub struct AudioDeviceDescriptor {
    pub target: AudioDeviceTargetId,
    pub label: String,
    pub fingerprint: Option<String>,
    pub input_channels: Vec<PhysicalChannelDescriptor>,
    pub output_channels: Vec<PhysicalChannelDescriptor>,
    pub is_system_default_input: bool,
    pub is_system_default_output: bool,
}

impl AudioDeviceDescriptor {
    #[must_use]
    pub fn supports(&self, direction: AudioDirection) -> bool {
        match direction {
            AudioDirection::Input => !self.input_channels.is_empty(),
            AudioDirection::Output => !self.output_channels.is_empty(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
pub struct AudioBackendStatus {
    pub backend: BackendId,
    pub label: String,
    pub state: AudioBackendState,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
pub struct NegotiatedStreamFormat {
    pub sample_rate: u32,
    pub channels: u16,
    pub sample_format: AudioSampleFormat,
    pub buffer_frames: u32,
    pub estimated_latency_ms: f32,
}

impl NegotiatedStreamFormat {
    pub fn validate(&self) -> Result<(), AudioError> {
        SampleRate::new(self.sample_rate)?;
        FrameCount::new(self.buffer_frames)?;
        if self.channels == 0 {
            return Err(AudioError::invalid_configuration(
                "negotiated stream channel count must be greater than zero",
            ));
        }
        if !self.estimated_latency_ms.is_finite() || self.estimated_latency_ms < 0.0 {
            return Err(AudioError::invalid_configuration(
                "estimated stream latency must be finite and non-negative",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
pub struct AudioStreamStatus {
    pub direction: AudioDirection,
    pub enabled: bool,
    pub selected_target: Option<AudioDeviceTargetId>,
    pub active_target: Option<AudioDeviceTargetId>,
    pub readiness: AudioDeviceReadiness,
    pub permission: AudioPermissionState,
    pub format: Option<NegotiatedStreamFormat>,
    pub error: Option<AudioInspectorError>,
}

impl AudioStreamStatus {
    #[must_use]
    pub fn disabled(direction: AudioDirection) -> Self {
        Self {
            direction,
            enabled: false,
            selected_target: None,
            active_target: None,
            readiness: AudioDeviceReadiness::Disabled,
            permission: AudioPermissionState::Unknown,
            format: None,
            error: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
pub struct AudioInspectorError {
    pub category: AudioErrorCategory,
    pub message: String,
    pub technical_detail: Option<String>,
}

impl From<&AudioError> for AudioInspectorError {
    fn from(error: &AudioError) -> Self {
        Self {
            category: error.category,
            message: error.message.clone(),
            technical_detail: (!error.context.is_empty()).then(|| {
                error
                    .context
                    .iter()
                    .map(|(key, value)| format!("{key}: {value}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            }),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
pub struct AudioDeviceInspectorState {
    pub discovery_in_progress: bool,
    pub backends: Vec<AudioBackendStatus>,
    pub devices: Vec<AudioDeviceDescriptor>,
    pub input: AudioStreamStatus,
    pub output: AudioStreamStatus,
    pub recovery_policy: AudioRecoveryPolicy,
    pub engine_sample_rate: u32,
    pub buffer_policy: AudioBufferPolicy,
}

impl Default for AudioDeviceInspectorState {
    fn default() -> Self {
        Self {
            discovery_in_progress: false,
            backends: Vec::new(),
            devices: Vec::new(),
            input: AudioStreamStatus::disabled(AudioDirection::Input),
            output: AudioStreamStatus::disabled(AudioDirection::Output),
            recovery_policy: AudioRecoveryPolicy::WaitForSelected,
            engine_sample_rate: SampleRate::default().get(),
            buffer_policy: AudioBufferPolicy::Automatic,
        }
    }
}
