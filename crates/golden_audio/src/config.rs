use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{
    AnalysisProcessorConfiguration, AnalysisTapId, AudioBufferPolicy, AudioChannelId, AudioDeviceSelection, AudioError,
    AudioRecoveryPolicy, AudioRouteId, EngineLimits, FrameCount, PhysicalChannelKey, SampleRate,
};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct GainDb(f32);

impl GainDb {
    pub const SILENCE_DB: f32 = -120.0;
    pub const MAX_DB: f32 = 24.0;
    pub const SILENCE: Self = Self(Self::SILENCE_DB);
    pub const UNITY: Self = Self(0.0);

    pub fn new(value: f32) -> Result<Self, AudioError> {
        if !value.is_finite() || !(Self::SILENCE_DB..=Self::MAX_DB).contains(&value) {
            return Err(AudioError::invalid_configuration(format!(
                "gain {value} dB is outside {} through {} dB",
                Self::SILENCE_DB,
                Self::MAX_DB
            )));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> f32 {
        self.0
    }

    #[must_use]
    pub fn to_linear(self) -> f32 {
        if self.0 <= Self::SILENCE_DB {
            0.0
        } else {
            10.0_f32.powf(self.0 / 20.0)
        }
    }
}

impl Default for GainDb {
    fn default() -> Self {
        Self::UNITY
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AudioEngineConfig {
    pub sample_rate: SampleRate,
    pub internal_block_frames: FrameCount,
    pub gain_ramp_ms: f32,
    pub observation_hz: u16,
    pub rms_window_ms: f32,
}

impl AudioEngineConfig {
    pub fn validate(&self) -> Result<(), AudioError> {
        if !self.gain_ramp_ms.is_finite() || !(5.0..=20.0).contains(&self.gain_ramp_ms) {
            return Err(AudioError::invalid_configuration(
                "gain ramp must be finite and between 5 and 20 milliseconds",
            ));
        }
        if !(1..=60).contains(&self.observation_hz) {
            return Err(AudioError::invalid_configuration(
                "observation rate must be from 1 through 60 Hz",
            ));
        }
        if !self.rms_window_ms.is_finite() || !(5.0..=1_000.0).contains(&self.rms_window_ms) {
            return Err(AudioError::invalid_configuration(
                "RMS window must be finite and between 5 and 1,000 milliseconds",
            ));
        }
        Ok(())
    }
}

impl Default for AudioEngineConfig {
    fn default() -> Self {
        Self {
            sample_rate: SampleRate::default(),
            internal_block_frames: FrameCount::default(),
            gain_ramp_ms: 10.0,
            observation_hz: 30,
            rms_window_ms: 50.0,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DirectionConfiguration {
    pub enabled: bool,
    pub device: Option<AudioDeviceSelection>,
    pub recovery_policy: AudioRecoveryPolicy,
    pub buffer_policy: AudioBufferPolicy,
}

impl DirectionConfiguration {
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            device: None,
            recovery_policy: AudioRecoveryPolicy::WaitForSelected,
            buffer_policy: AudioBufferPolicy::Automatic,
        }
    }

    pub fn validate(&self, direction: &'static str) -> Result<(), AudioError> {
        self.buffer_policy.validate()?;
        if self.enabled && self.device.is_none() {
            return Err(AudioError::invalid_configuration(format!(
                "enabled {direction} requires a device target"
            )));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct VirtualInputChannel {
    pub id: AudioChannelId,
    pub label: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct VirtualOutputChannel {
    pub id: AudioChannelId,
    pub label: String,
    pub gain: GainDb,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct InputPatchRoute {
    pub id: AudioRouteId,
    pub source: PhysicalChannelKey,
    pub destination: AudioChannelId,
    pub gain: GainDb,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MonitorRoute {
    pub id: AudioRouteId,
    pub source: AudioChannelId,
    pub destination: AudioChannelId,
    pub gain: GainDb,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PlaybackRoute {
    pub id: AudioRouteId,
    pub source_channel: u16,
    pub destination: AudioChannelId,
    pub gain: GainDb,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct OutputPatchRoute {
    pub id: AudioRouteId,
    pub source: AudioChannelId,
    pub destination: PhysicalChannelKey,
    pub gain: GainDb,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
pub struct AnalysisTapConfiguration {
    pub id: AnalysisTapId,
    pub source: AudioChannelId,
    pub enabled: bool,
    pub processor: AnalysisProcessorConfiguration,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AudioConfiguration {
    pub enabled: bool,
    pub input: DirectionConfiguration,
    pub output: DirectionConfiguration,
    pub physical_inputs: Vec<PhysicalChannelKey>,
    pub physical_outputs: Vec<PhysicalChannelKey>,
    pub virtual_inputs: Vec<VirtualInputChannel>,
    pub virtual_outputs: Vec<VirtualOutputChannel>,
    pub input_patch: Vec<InputPatchRoute>,
    pub monitoring: Vec<MonitorRoute>,
    pub playback_patch: Vec<PlaybackRoute>,
    pub output_patch: Vec<OutputPatchRoute>,
    pub analysis_taps: Vec<AnalysisTapConfiguration>,
    pub master_gain: GainDb,
}

impl AudioConfiguration {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            enabled: true,
            input: DirectionConfiguration::disabled(),
            output: DirectionConfiguration::disabled(),
            physical_inputs: Vec::new(),
            physical_outputs: Vec::new(),
            virtual_inputs: Vec::new(),
            virtual_outputs: Vec::new(),
            input_patch: Vec::new(),
            monitoring: Vec::new(),
            playback_patch: Vec::new(),
            output_patch: Vec::new(),
            analysis_taps: Vec::new(),
            master_gain: GainDb::UNITY,
        }
    }

    pub fn validate(&self, limits: &EngineLimits) -> Result<(), AudioError> {
        limits.validate()?;
        self.input.validate("input")?;
        self.output.validate("output")?;
        if self.input.enabled && self.physical_inputs.is_empty() {
            return Err(AudioError::invalid_configuration(
                "enabled input requires a physical channel inventory",
            ));
        }
        if self.output.enabled && self.physical_outputs.is_empty() {
            return Err(AudioError::invalid_configuration(
                "enabled output requires a physical channel inventory",
            ));
        }
        validate_capacity(
            "physical input channels",
            self.physical_inputs.len(),
            usize::from(limits.max_physical_inputs),
        )?;
        validate_capacity(
            "physical output channels",
            self.physical_outputs.len(),
            usize::from(limits.max_physical_outputs),
        )?;
        validate_capacity(
            "virtual input channels",
            self.virtual_inputs.len(),
            usize::from(limits.max_virtual_inputs),
        )?;
        validate_capacity(
            "virtual output channels",
            self.virtual_outputs.len(),
            usize::from(limits.max_virtual_outputs),
        )?;
        validate_capacity(
            "analysis taps",
            self.analysis_taps.len(),
            usize::from(limits.max_analysis_taps),
        )?;

        let route_count =
            self.input_patch.len() + self.monitoring.len() + self.playback_patch.len() + self.output_patch.len();
        validate_capacity("routes", route_count, limits.max_routes as usize)?;

        let input_ids =
            collect_unique_channel_ids("virtual input", self.virtual_inputs.iter().map(|channel| channel.id))?;
        let output_ids =
            collect_unique_channel_ids("virtual output", self.virtual_outputs.iter().map(|channel| channel.id))?;
        let physical_input_keys = collect_unique_physical_channel_keys("physical input", self.physical_inputs.iter())?;
        let physical_output_keys =
            collect_unique_physical_channel_keys("physical output", self.physical_outputs.iter())?;
        let mut route_ids = HashSet::with_capacity(route_count);

        for route in &self.input_patch {
            validate_unique_route_id(&mut route_ids, route.id)?;
            validate_physical_channel_reference("input patch source", &route.source, &physical_input_keys)?;
            validate_channel_reference("input patch destination", route.destination, &input_ids)?;
            GainDb::new(route.gain.get())?;
        }
        for route in &self.monitoring {
            validate_unique_route_id(&mut route_ids, route.id)?;
            validate_channel_reference("monitor source", route.source, &input_ids)?;
            validate_channel_reference("monitor destination", route.destination, &output_ids)?;
            GainDb::new(route.gain.get())?;
        }
        for route in &self.playback_patch {
            validate_unique_route_id(&mut route_ids, route.id)?;
            validate_channel_reference("playback destination", route.destination, &output_ids)?;
            GainDb::new(route.gain.get())?;
        }
        for route in &self.output_patch {
            validate_unique_route_id(&mut route_ids, route.id)?;
            validate_channel_reference("output patch source", route.source, &output_ids)?;
            validate_physical_channel_reference("output patch destination", &route.destination, &physical_output_keys)?;
            GainDb::new(route.gain.get())?;
        }
        for channel in &self.virtual_outputs {
            GainDb::new(channel.gain.get())?;
        }
        GainDb::new(self.master_gain.get())?;

        let mut analysis_ids = HashSet::with_capacity(self.analysis_taps.len());
        for tap in &self.analysis_taps {
            if !analysis_ids.insert(tap.id) {
                return Err(AudioError::invalid_configuration(format!(
                    "duplicate analysis tap ID {}",
                    tap.id
                )));
            }
            validate_channel_reference("analysis source", tap.source, &input_ids)?;
        }
        Ok(())
    }
}

impl Default for AudioConfiguration {
    fn default() -> Self {
        Self::empty()
    }
}

fn validate_capacity(name: &str, actual: usize, maximum: usize) -> Result<(), AudioError> {
    if actual > maximum {
        return Err(AudioError::capacity_exceeded(format!(
            "{name} count {actual} exceeds configured limit {maximum}"
        )));
    }
    Ok(())
}

fn collect_unique_channel_ids(
    name: &str,
    ids: impl Iterator<Item = AudioChannelId>,
) -> Result<HashSet<AudioChannelId>, AudioError> {
    let mut collected = HashSet::new();
    for id in ids {
        if !collected.insert(id) {
            return Err(AudioError::invalid_configuration(format!(
                "duplicate {name} channel ID {id}"
            )));
        }
    }
    Ok(collected)
}

fn collect_unique_physical_channel_keys<'a>(
    name: &str,
    keys: impl Iterator<Item = &'a PhysicalChannelKey>,
) -> Result<HashSet<PhysicalChannelKey>, AudioError> {
    let mut collected = HashSet::new();
    for key in keys {
        if !collected.insert(key.clone()) {
            return Err(AudioError::invalid_configuration(format!(
                "duplicate {name} channel key {key}"
            )));
        }
    }
    Ok(collected)
}

fn validate_physical_channel_reference(
    name: &str,
    key: &PhysicalChannelKey,
    known: &HashSet<PhysicalChannelKey>,
) -> Result<(), AudioError> {
    if !known.contains(key) {
        return Err(AudioError::invalid_configuration(format!(
            "{name} references unavailable physical channel {key}"
        )));
    }
    Ok(())
}

fn validate_unique_route_id(route_ids: &mut HashSet<AudioRouteId>, id: AudioRouteId) -> Result<(), AudioError> {
    if !route_ids.insert(id) {
        return Err(AudioError::invalid_configuration(format!("duplicate route ID {id}")));
    }
    Ok(())
}

fn validate_channel_reference(
    role: &str,
    id: AudioChannelId,
    known: &HashSet<AudioChannelId>,
) -> Result<(), AudioError> {
    if !known.contains(&id) {
        return Err(AudioError::invalid_configuration(format!(
            "{role} references unknown channel {id}"
        )));
    }
    Ok(())
}
