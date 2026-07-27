use std::path::Path;

use golden_audio::{
    AnalysisObservationSnapshot, AudioDeviceInspectorState, ChannelObservation, ConfigGeneration, PlaybackObservation,
    RenderRuntimeObservation,
};
use serde::{Deserialize, Serialize};
use ts_rs::{Config, TS};

pub const SOUND_CARD_TELEMETRY_TOPIC: &str = "chataigne.sound_card.telemetry";
pub const SOUND_CARD_UI_CONTROL_TOPIC: &str = "chataigne.sound_card.ui.control";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, rename_all = "snake_case")]
pub enum SoundCardPlaybackLifecycle {
    Playing,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct SoundCardPlaybackVoiceDto {
    pub playback_id: String,
    pub path: String,
    pub voice: String,
    pub lifecycle: SoundCardPlaybackLifecycle,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export, tag = "kind", rename_all = "snake_case")]
pub enum SoundCardUiControlRequest {
    StopFile {
        playback_id: String,
    },
    StopAllFiles,
    ConnectRoute {
        direction: golden_audio::AudioDirection,
        physical_channel: String,
        app_channel_uuid: String,
    },
    DisconnectRoute {
        direction: golden_audio::AudioDirection,
        physical_channel: String,
        app_channel_uuid: String,
    },
    RenameChannel {
        direction: golden_audio::AudioDirection,
        app_channel_uuid: String,
        label: String,
    },
}

/// Latest-wins UI projection emitted by a Chataigne Sound Card module.
///
/// This app-owned envelope keeps product telemetry separate from the generic
/// Golden device-inspector and analysis contracts embedded within it.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct SoundCardUiTelemetryDto {
    pub generation: ConfigGeneration,
    #[ts(type = "number")]
    pub render_frame: u64,
    pub device: AudioDeviceInspectorState,
    pub inputs: Vec<ChannelObservation>,
    pub outputs: Vec<ChannelObservation>,
    pub input_global_max_rms: f32,
    pub output_global_max_rms: f32,
    pub global_max_rms: f32,
    pub playback: PlaybackObservation,
    pub runtime: RenderRuntimeObservation,
    pub playback_source_channel_limit: u16,
    pub playback_voices: Vec<SoundCardPlaybackVoiceDto>,
    #[ts(type = "number")]
    pub dropped_event_count: u64,
    #[ts(type = "number")]
    pub queue_pressure_count: u64,
    pub analysis: AnalysisObservationSnapshot,
}

pub fn export_sound_card_contract(output_dir: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = output_dir.as_ref();
    golden_audio::contract::export_device_contract(output_dir)?;
    let config = Config::new().with_out_dir(output_dir.to_path_buf());
    SoundCardPlaybackLifecycle::export_all(&config)?;
    SoundCardPlaybackVoiceDto::export_all(&config)?;
    SoundCardUiControlRequest::export_all(&config)?;
    SoundCardUiTelemetryDto::export_all(&config)?;
    std::fs::write(
        output_dir.join("SoundCardTopics.ts"),
        format!(
            "export const SOUND_CARD_TELEMETRY_TOPIC = {SOUND_CARD_TELEMETRY_TOPIC:?} as const;\n\
             export const SOUND_CARD_UI_CONTROL_TOPIC = {SOUND_CARD_UI_CONTROL_TOPIC:?} as const;\n"
        ),
    )?;

    let index = std::fs::read_to_string(output_dir.join("index.ts"))?;
    std::fs::write(
        output_dir.join("index.ts"),
        format!(
            "{index}export type {{ SoundCardPlaybackLifecycle }} from './SoundCardPlaybackLifecycle';\n\
             export type {{ SoundCardPlaybackVoiceDto }} from './SoundCardPlaybackVoiceDto';\n\
             export type {{ SoundCardUiControlRequest }} from './SoundCardUiControlRequest';\n\
             export type {{ SoundCardUiTelemetryDto }} from './SoundCardUiTelemetryDto';\n\
             export {{ SOUND_CARD_TELEMETRY_TOPIC, SOUND_CARD_UI_CONTROL_TOPIC }} from './SoundCardTopics';\n"
        ),
    )?;
    Ok(())
}
