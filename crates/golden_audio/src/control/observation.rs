use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

use crate::{AudioChannelId, AudioDeviceInspectorState, ConfigGeneration};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ChannelObservation {
    pub channel: AudioChannelId,
    pub rms_linear: f32,
    pub rms_dbfs: f32,
    pub peak_dbfs: f32,
    pub clipped: bool,
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
    pub active_voice_count: u16,
    pub dropped_event_count: u64,
    pub device: AudioDeviceInspectorState,
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
            active_voice_count: 0,
            dropped_event_count: 0,
            device: AudioDeviceInspectorState::default(),
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
