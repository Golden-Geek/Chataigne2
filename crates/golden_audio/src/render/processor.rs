#[cfg(feature = "analysis")]
use crate::AnalysisRenderer;
use crate::{AudioChannelId, AudioError, AudioErrorCategory, AudioRouteId, GainDb};

use super::{
    GainSmoother, PlanarBuffer, RenderPlan,
    route_kernel::{RouteMatrixState, RouteMix, mix_routes},
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RenderProcessorMetrics {
    pub rendered_frames: u64,
    pub rendered_blocks: u64,
}

#[derive(Debug)]
pub struct RenderProcessor {
    plan: Box<RenderPlan>,
    virtual_inputs: PlanarBuffer,
    virtual_outputs: PlanarBuffer,
    input_patch_state: RouteMatrixState,
    monitoring_state: RouteMatrixState,
    playback_patch_state: RouteMatrixState,
    output_patch_state: RouteMatrixState,
    output_gains: Vec<GainSmoother>,
    master_gain: GainSmoother,
    metrics: RenderProcessorMetrics,
    #[cfg(feature = "analysis")]
    analysis: Option<AnalysisRenderer>,
}

impl RenderProcessor {
    pub fn new(plan: RenderPlan) -> Result<Self, AudioError> {
        let block_frames = plan.internal_block_frames.get() as usize;
        Ok(Self {
            virtual_inputs: PlanarBuffer::new(plan.virtual_inputs.len(), block_frames)?,
            virtual_outputs: PlanarBuffer::new(plan.virtual_outputs.len(), block_frames)?,
            input_patch_state: RouteMatrixState::new(&plan.input_patch),
            monitoring_state: RouteMatrixState::new(&plan.monitoring),
            playback_patch_state: RouteMatrixState::new(&plan.playback_patch),
            output_patch_state: RouteMatrixState::new(&plan.output_patch),
            output_gains: plan.output_gains.iter().copied().map(GainSmoother::settled).collect(),
            master_gain: GainSmoother::settled(plan.master_gain),
            plan: Box::new(plan),
            metrics: RenderProcessorMetrics::default(),
            #[cfg(feature = "analysis")]
            analysis: None,
        })
    }

    #[must_use]
    pub fn plan(&self) -> &RenderPlan {
        &self.plan
    }

    #[must_use]
    pub const fn metrics(&self) -> RenderProcessorMetrics {
        self.metrics
    }

    #[cfg(feature = "analysis")]
    pub fn attach_analysis(&mut self, renderer: AnalysisRenderer) -> Result<(), AudioError> {
        if self.analysis.is_some() {
            return Err(AudioError::invalid_configuration(
                "render processor already has an analysis renderer",
            ));
        }
        if !renderer.matches_plan(&self.plan) {
            return Err(AudioError::invalid_configuration(
                "analysis renderer topology does not match the render plan",
            ));
        }
        self.analysis = Some(renderer);
        Ok(())
    }

    #[cfg(feature = "analysis")]
    pub fn take_analysis(&mut self) -> Option<AnalysisRenderer> {
        self.analysis.take()
    }

    pub fn set_route_gain(&mut self, route: AudioRouteId, gain: GainDb) -> Result<(), AudioError> {
        let target = gain.to_linear();
        let ramp_frames = self.plan.gain_ramp_frames;
        let found = self
            .input_patch_state
            .set_target(&self.plan.input_patch, route, target, ramp_frames)
            || self
                .monitoring_state
                .set_target(&self.plan.monitoring, route, target, ramp_frames)
            || self
                .playback_patch_state
                .set_target(&self.plan.playback_patch, route, target, ramp_frames)
            || self
                .output_patch_state
                .set_target(&self.plan.output_patch, route, target, ramp_frames);
        if !found {
            return Err(AudioError::invalid_configuration(format!(
                "render plan does not contain route {route}"
            )));
        }
        Ok(())
    }

    pub fn set_output_gain(&mut self, channel: AudioChannelId, gain: GainDb) -> Result<(), AudioError> {
        let Some(index) = self
            .plan
            .virtual_outputs
            .iter()
            .position(|candidate| *candidate == channel)
        else {
            return Err(AudioError::invalid_configuration(format!(
                "render plan does not contain output channel {channel}"
            )));
        };
        self.output_gains[index].set_target(gain.to_linear(), self.plan.gain_ramp_frames);
        Ok(())
    }

    pub fn set_master_gain(&mut self, gain: GainDb) {
        self.master_gain
            .set_target(gain.to_linear(), self.plan.gain_ramp_frames);
    }

    pub fn render(
        &mut self,
        physical_inputs: &PlanarBuffer,
        playback_inputs: &PlanarBuffer,
        physical_outputs: &mut PlanarBuffer,
        frames: usize,
    ) -> Result<RenderProcessorMetrics, AudioError> {
        self.validate_io(physical_inputs, playback_inputs, physical_outputs, frames)?;
        physical_outputs.zero(frames)?;
        let block_frames = self.plan.internal_block_frames.get() as usize;
        let mut frame_offset = 0;
        while frame_offset < frames {
            let chunk_frames = (frames - frame_offset).min(block_frames);
            self.render_chunk(
                physical_inputs,
                playback_inputs,
                physical_outputs,
                frame_offset,
                chunk_frames,
            )?;
            frame_offset += chunk_frames;
            self.metrics.rendered_blocks = self.metrics.rendered_blocks.saturating_add(1);
        }
        self.metrics.rendered_frames = self.metrics.rendered_frames.saturating_add(frames as u64);
        Ok(self.metrics)
    }

    fn render_chunk(
        &mut self,
        physical_inputs: &PlanarBuffer,
        playback_inputs: &PlanarBuffer,
        physical_outputs: &mut PlanarBuffer,
        frame_offset: usize,
        frames: usize,
    ) -> Result<(), AudioError> {
        mix_routes(
            &self.plan.input_patch,
            &mut self.input_patch_state,
            RouteMix {
                source: physical_inputs,
                source_frame_offset: frame_offset,
                destination: &mut self.virtual_inputs,
                destination_frame_offset: 0,
                frames,
                clear_destination: true,
            },
        )?;
        #[cfg(feature = "analysis")]
        if let Some(analysis) = &mut self.analysis {
            analysis.capture_inputs(
                &self.virtual_inputs,
                frames,
                self.metrics.rendered_frames.saturating_add(frame_offset as u64),
            )?;
        }
        mix_routes(
            &self.plan.monitoring,
            &mut self.monitoring_state,
            RouteMix {
                source: &self.virtual_inputs,
                source_frame_offset: 0,
                destination: &mut self.virtual_outputs,
                destination_frame_offset: 0,
                frames,
                clear_destination: true,
            },
        )?;
        mix_routes(
            &self.plan.playback_patch,
            &mut self.playback_patch_state,
            RouteMix {
                source: playback_inputs,
                source_frame_offset: frame_offset,
                destination: &mut self.virtual_outputs,
                destination_frame_offset: 0,
                frames,
                clear_destination: false,
            },
        )?;
        self.apply_output_and_master_gains(frames);
        #[cfg(feature = "analysis")]
        if let Some(analysis) = &mut self.analysis {
            analysis.capture_outputs(
                &self.virtual_outputs,
                frames,
                self.metrics.rendered_frames.saturating_add(frame_offset as u64),
            )?;
        }
        mix_routes(
            &self.plan.output_patch,
            &mut self.output_patch_state,
            RouteMix {
                source: &self.virtual_outputs,
                source_frame_offset: 0,
                destination: physical_outputs,
                destination_frame_offset: frame_offset,
                frames,
                clear_destination: false,
            },
        )
    }

    fn apply_output_and_master_gains(&mut self, frames: usize) {
        for frame in 0..frames {
            let master = self.master_gain.next_gain();
            for channel in 0..self.virtual_outputs.channels() {
                let gain = self.output_gains[channel].next_gain() * master;
                let sample = self.virtual_outputs.sample(channel, frame) * gain;
                self.virtual_outputs.set_sample(channel, frame, sample);
            }
        }
    }

    fn validate_io(
        &self,
        physical_inputs: &PlanarBuffer,
        playback_inputs: &PlanarBuffer,
        physical_outputs: &PlanarBuffer,
        frames: usize,
    ) -> Result<(), AudioError> {
        if frames == 0 {
            return Err(AudioError::invalid_configuration(
                "render frame count must be greater than zero",
            ));
        }
        if physical_inputs.channels() < self.plan.physical_inputs.len()
            || playback_inputs.channels() < self.plan.playback_source_channels
            || physical_outputs.channels() < self.plan.physical_outputs.len()
        {
            return Err(AudioError::new(
                AudioErrorCategory::CapacityExceeded,
                "render I/O channel capacity is smaller than the compiled plan",
            ));
        }
        physical_inputs.validate_range(0, frames)?;
        playback_inputs.validate_range(0, frames)?;
        physical_outputs.validate_range(0, frames)?;
        Ok(())
    }
}
