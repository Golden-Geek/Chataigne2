use serde::{Deserialize, Serialize};

use crate::{AnalysisTapId, AudioChannelId, ConfigGeneration};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
pub struct ChannelObservation {
    pub channel: AudioChannelId,
    pub rms_linear: f32,
    pub rms_dbfs: f32,
    pub peak_dbfs: f32,
    pub clipped: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
pub struct PitchObservation {
    pub valid: bool,
    pub frequency_hz: f32,
    pub confidence: f32,
    pub midi_note: i16,
    pub note_name: String,
    pub cents: f32,
}

impl PitchObservation {
    #[must_use]
    pub fn invalid() -> Self {
        Self {
            valid: false,
            frequency_hz: 0.0,
            confidence: 0.0,
            midi_note: 0,
            note_name: String::new(),
            cents: 0.0,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
pub struct SpectrumBandObservation {
    pub index: u16,
    pub low_hz: f32,
    pub center_hz: f32,
    pub high_hz: f32,
    pub amplitude_linear: f32,
    pub amplitude_dbfs: f32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
pub struct SpectrumObservation {
    pub fft_size: u32,
    pub bands: Vec<SpectrumBandObservation>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum AnalysisResult {
    Pitch(PitchObservation),
    Spectrum(SpectrumObservation),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
pub struct AnalysisTapObservation {
    pub tap: AnalysisTapId,
    pub source: AudioChannelId,
    pub enabled: bool,
    pub result: Option<AnalysisResult>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
pub struct AnalysisDiagnosticsObservation {
    pub captured_frames: u64,
    pub processed_frames: u64,
    pub dropped_frames: u64,
    pub stale_frames: u64,
    pub worker_time_micros: u64,
    pub maximum_worker_time_micros: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
pub struct AnalysisObservationSnapshot {
    pub generation: ConfigGeneration,
    pub render_frame: u64,
    pub inputs: Vec<ChannelObservation>,
    pub outputs: Vec<ChannelObservation>,
    pub input_global_max_rms: f32,
    pub output_global_max_rms: f32,
    pub global_max_rms: f32,
    pub taps: Vec<AnalysisTapObservation>,
    pub diagnostics: AnalysisDiagnosticsObservation,
}

impl Default for AnalysisObservationSnapshot {
    fn default() -> Self {
        Self {
            generation: ConfigGeneration::INITIAL,
            render_frame: 0,
            inputs: Vec::new(),
            outputs: Vec::new(),
            input_global_max_rms: 0.0,
            output_global_max_rms: 0.0,
            global_max_rms: 0.0,
            taps: Vec::new(),
            diagnostics: AnalysisDiagnosticsObservation::default(),
        }
    }
}
