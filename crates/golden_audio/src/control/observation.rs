use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

use crate::{AnalysisObservationSnapshot, AudioDeviceInspectorState, ChannelObservation, ConfigGeneration};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
pub struct PlaybackObservation {
    pub loading_voices: u16,
    pub active_voices: u16,
    #[cfg_attr(feature = "codegen", ts(type = "number"))]
    pub command_queue_pressure_count: u64,
    #[cfg_attr(feature = "codegen", ts(type = "number"))]
    pub cache_entries: u64,
    #[cfg_attr(feature = "codegen", ts(type = "number"))]
    pub resident_bytes: u64,
    #[cfg_attr(feature = "codegen", ts(type = "number"))]
    pub cache_hits: u64,
    #[cfg_attr(feature = "codegen", ts(type = "number"))]
    pub cache_misses: u64,
    #[cfg_attr(feature = "codegen", ts(type = "number"))]
    pub cache_invalidations: u64,
    #[cfg_attr(feature = "codegen", ts(type = "number"))]
    pub cache_evictions: u64,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
pub struct RenderRuntimeObservation {
    #[cfg_attr(feature = "codegen", ts(type = "number"))]
    pub rendered_blocks: u64,
    #[cfg_attr(feature = "codegen", ts(type = "number"))]
    pub rendered_frames: u64,
    #[cfg_attr(feature = "codegen", ts(type = "number"))]
    pub render_time_micros: u64,
    #[cfg_attr(feature = "codegen", ts(type = "number"))]
    pub maximum_render_time_micros: u64,
    #[cfg_attr(feature = "codegen", ts(type = "number"))]
    pub deadline_miss_count: u64,
    #[cfg_attr(feature = "codegen", ts(type = "number"))]
    pub xrun_count: u64,
    #[cfg_attr(feature = "codegen", ts(type = "number"))]
    pub control_queue_pressure_count: u64,
    #[cfg_attr(feature = "codegen", ts(type = "number"))]
    pub input_underflow_count: u64,
    #[cfg_attr(feature = "codegen", ts(type = "number"))]
    pub input_overflow_count: u64,
    #[cfg_attr(feature = "codegen", ts(type = "number"))]
    pub output_underflow_count: u64,
    #[cfg_attr(feature = "codegen", ts(type = "number"))]
    pub output_overflow_count: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AudioObservationSnapshot {
    pub generation: ConfigGeneration,
    pub render_frame: u64,
    pub enabled: bool,
    pub inputs: Vec<ChannelObservation>,
    pub outputs: Vec<ChannelObservation>,
    pub input_global_max_rms: f32,
    pub output_global_max_rms: f32,
    pub global_max_rms: f32,
    pub playback: PlaybackObservation,
    pub runtime: RenderRuntimeObservation,
    pub dropped_event_count: u64,
    pub queue_pressure_count: u64,
    pub device: AudioDeviceInspectorState,
    pub analysis: AnalysisObservationSnapshot,
}

impl Default for AudioObservationSnapshot {
    fn default() -> Self {
        Self {
            generation: ConfigGeneration::INITIAL,
            render_frame: 0,
            enabled: true,
            inputs: Vec::new(),
            outputs: Vec::new(),
            input_global_max_rms: 0.0,
            output_global_max_rms: 0.0,
            global_max_rms: 0.0,
            playback: PlaybackObservation::default(),
            runtime: RenderRuntimeObservation::default(),
            dropped_event_count: 0,
            queue_pressure_count: 0,
            device: AudioDeviceInspectorState::default(),
            analysis: AnalysisObservationSnapshot::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct AudioObservationReader {
    pub(super) shared: Arc<RwLock<AudioObservationSnapshot>>,
}

impl AudioObservationReader {
    #[must_use]
    pub fn latest(&self) -> AudioObservationSnapshot {
        self.shared.read().map(|snapshot| snapshot.clone()).unwrap_or_default()
    }
}
