use std::path::Path;

use golden_audio::{AnalysisObservationSnapshot, AudioDeviceInspectorState, ChannelObservation, ConfigGeneration};
use serde::{Deserialize, Serialize};
use ts_rs::{Config, TS};

pub const SOUND_CARD_TELEMETRY_TOPIC: &str = "chataigne.sound_card.telemetry";

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
    pub active_voice_count: u16,
    pub loading_voice_count: u16,
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
    SoundCardUiTelemetryDto::export_all(&config)?;

    let index = std::fs::read_to_string(output_dir.join("index.ts"))?;
    std::fs::write(
        output_dir.join("index.ts"),
        format!("{index}export type {{ SoundCardUiTelemetryDto }} from './SoundCardUiTelemetryDto';\n"),
    )?;
    Ok(())
}
