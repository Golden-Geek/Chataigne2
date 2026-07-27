mod identity;
mod negotiation;
mod profile;
mod supervisor;

pub use identity::{AudioDeviceMatch, AudioDeviceSelection, match_device_selection, profile_key_for};
pub use negotiation::{
    ChannelCountPolicy, DeviceNegotiator, SampleFormatPolicy, SampleRatePolicy, StreamNegotiationRequest,
};
pub use profile::{AudioDeviceProfile, DeviceProfileStore};
pub use supervisor::{DeviceSupervisor, DeviceSupervisorConfig, DeviceSwitchPhase, RetryBackoff, SupervisorDirection};

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    AudioDeviceId, AudioDeviceProfileKey, AudioError, AudioErrorCategory, BackendId, FrameCount, PhysicalChannelKey,
    SampleRate,
};

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
    I8,
    F32,
    F64,
    I16,
    I24,
    I32,
    I64,
    U8,
    U16,
    U24,
    U32,
    U64,
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
    Preparing,
    Primed,
    Switching,
    Recovering,
    Ready,
    Failed,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
pub struct AudioDeviceFingerprint {
    pub vendor: Option<String>,
    pub product: Option<String>,
    pub serial: Option<String>,
    pub transport: Option<String>,
    pub backend_path: Option<String>,
    pub input_channels: u16,
    pub output_channels: u16,
    pub properties: BTreeMap<String, String>,
}

impl AudioDeviceFingerprint {
    #[must_use]
    pub fn has_identifying_metadata(&self) -> bool {
        self.vendor.is_some()
            || self.product.is_some()
            || self.serial.is_some()
            || self.transport.is_some()
            || self.backend_path.is_some()
            || !self.properties.is_empty()
    }
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
    pub stable_id: bool,
    pub fingerprint: AudioDeviceFingerprint,
    pub profile_key: AudioDeviceProfileKey,
    pub input_channels: Vec<PhysicalChannelDescriptor>,
    pub output_channels: Vec<PhysicalChannelDescriptor>,
    pub supported_configurations: Vec<SupportedStreamConfiguration>,
    pub is_system_default_input: bool,
    pub is_system_default_output: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
pub struct AudioDeviceCatalogEntry {
    pub target: AudioDeviceTargetId,
    pub label: String,
}

impl From<&AudioDeviceDescriptor> for AudioDeviceCatalogEntry {
    fn from(device: &AudioDeviceDescriptor) -> Self {
        Self {
            target: device.target.clone(),
            label: device.label.clone(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct AudioDeviceInventory {
    pub catalog: Vec<AudioDeviceCatalogEntry>,
    pub devices: Vec<AudioDeviceDescriptor>,
}

impl AudioDeviceInventory {
    #[must_use]
    pub fn from_devices(devices: Vec<AudioDeviceDescriptor>) -> Self {
        let catalog = devices.iter().map(AudioDeviceCatalogEntry::from).collect();
        Self { catalog, devices }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
pub struct SupportedBufferFrames {
    pub min: u32,
    pub max: u32,
    pub preferred: u32,
}

impl SupportedBufferFrames {
    pub fn validate(self) -> Result<(), AudioError> {
        FrameCount::new(self.min)?;
        FrameCount::new(self.max)?;
        FrameCount::new(self.preferred)?;
        if self.min > self.max || !(self.min..=self.max).contains(&self.preferred) {
            return Err(AudioError::invalid_configuration(
                "supported buffer range must contain its preferred frame count",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
pub struct SupportedStreamConfiguration {
    pub direction: AudioDirection,
    pub channels: u16,
    pub sample_format: AudioSampleFormat,
    pub min_sample_rate: u32,
    pub max_sample_rate: u32,
    pub buffer_frames: SupportedBufferFrames,
}

impl SupportedStreamConfiguration {
    pub fn validate(&self) -> Result<(), AudioError> {
        if self.channels == 0 {
            return Err(AudioError::invalid_configuration(
                "supported stream channel count must be greater than zero",
            ));
        }
        SampleRate::new(self.min_sample_rate)?;
        SampleRate::new(self.max_sample_rate)?;
        if self.min_sample_rate > self.max_sample_rate {
            return Err(AudioError::invalid_configuration(
                "supported sample-rate minimum exceeds its maximum",
            ));
        }
        self.buffer_frames.validate()
    }
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
    pub selected_label: Option<String>,
    pub profile_key: Option<AudioDeviceProfileKey>,
    pub active_target: Option<AudioDeviceTargetId>,
    pub readiness: AudioDeviceReadiness,
    pub permission: AudioPermissionState,
    pub recovery_policy: AudioRecoveryPolicy,
    pub retry_attempt: u32,
    #[cfg_attr(feature = "codegen", ts(type = "number | null"))]
    pub next_retry_ms: Option<u64>,
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
            selected_label: None,
            profile_key: None,
            active_target: None,
            readiness: AudioDeviceReadiness::Disabled,
            permission: AudioPermissionState::Unknown,
            recovery_policy: AudioRecoveryPolicy::WaitForSelected,
            retry_attempt: 0,
            next_retry_ms: None,
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
    #[cfg_attr(feature = "codegen", ts(type = "number"))]
    pub inventory_revision: u64,
    pub backends: Vec<AudioBackendStatus>,
    pub device_catalog: Vec<AudioDeviceCatalogEntry>,
    pub devices: Vec<AudioDeviceDescriptor>,
    pub input: AudioStreamStatus,
    pub output: AudioStreamStatus,
    pub engine_sample_rate: u32,
    pub buffer_policy: AudioBufferPolicy,
}

impl Default for AudioDeviceInspectorState {
    fn default() -> Self {
        Self {
            discovery_in_progress: false,
            inventory_revision: 0,
            backends: Vec::new(),
            device_catalog: Vec::new(),
            devices: Vec::new(),
            input: AudioStreamStatus::disabled(AudioDirection::Input),
            output: AudioStreamStatus::disabled(AudioDirection::Output),
            engine_sample_rate: SampleRate::default().get(),
            buffer_policy: AudioBufferPolicy::Automatic,
        }
    }
}

#[cfg(test)]
mod tests;
