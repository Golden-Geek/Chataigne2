use serde::{Deserialize, Serialize};

use crate::{AudioChannelId, AudioRouteId, FrameCount, PhysicalChannelKey, SampleRate};

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledRoute {
    pub id: AudioRouteId,
    pub source: usize,
    pub destination: usize,
    pub gain: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouteSpan {
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledRouteMatrix {
    pub source_channels: usize,
    pub destination_channels: usize,
    pub routes: Vec<CompiledRoute>,
    pub destination_spans: Vec<RouteSpan>,
}

impl CompiledRouteMatrix {
    #[must_use]
    pub fn empty(source_channels: usize, destination_channels: usize) -> Self {
        Self {
            source_channels,
            destination_channels,
            routes: Vec::new(),
            destination_spans: vec![RouteSpan { start: 0, end: 0 }; destination_channels],
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderPlan {
    pub sample_rate: SampleRate,
    pub internal_block_frames: FrameCount,
    pub gain_ramp_frames: u32,
    pub physical_inputs: Vec<PhysicalChannelKey>,
    pub physical_outputs: Vec<PhysicalChannelKey>,
    pub virtual_inputs: Vec<AudioChannelId>,
    pub virtual_outputs: Vec<AudioChannelId>,
    pub playback_source_channels: usize,
    pub input_patch: CompiledRouteMatrix,
    pub monitoring: CompiledRouteMatrix,
    pub playback_patch: CompiledRouteMatrix,
    pub output_patch: CompiledRouteMatrix,
    pub output_gains: Vec<f32>,
    pub master_gain: f32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderWarningCode {
    UnresolvedPhysicalInput,
    UnresolvedPhysicalOutput,
    UnresolvedPlaybackChannel,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RenderWarning {
    pub code: RenderWarningCode,
    pub route: AudioRouteId,
    pub message: String,
}
