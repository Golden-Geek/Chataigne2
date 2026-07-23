use crate::{AudioError, AudioErrorCategory};

use super::{CompiledRouteMatrix, PlanarBuffer, RenderPlan};

pub fn render_scalar_reference(
    plan: &RenderPlan,
    physical_inputs: &PlanarBuffer,
    playback_inputs: &PlanarBuffer,
    frames: usize,
) -> Result<PlanarBuffer, AudioError> {
    validate_io(plan, physical_inputs, playback_inputs, frames)?;
    let mut virtual_inputs = PlanarBuffer::new(plan.virtual_inputs.len(), frames)?;
    let mut virtual_outputs = PlanarBuffer::new(plan.virtual_outputs.len(), frames)?;
    let mut physical_outputs = PlanarBuffer::new(plan.physical_outputs.len(), frames)?;

    mix_static(&plan.input_patch, physical_inputs, &mut virtual_inputs, frames);
    mix_static(&plan.monitoring, &virtual_inputs, &mut virtual_outputs, frames);
    mix_static(&plan.playback_patch, playback_inputs, &mut virtual_outputs, frames);
    for channel in 0..virtual_outputs.channels() {
        let gain = plan.output_gains[channel] * plan.master_gain;
        for frame in 0..frames {
            let sample = virtual_outputs.sample(channel, frame) * gain;
            virtual_outputs.set_sample(channel, frame, sample);
        }
    }
    mix_static(&plan.output_patch, &virtual_outputs, &mut physical_outputs, frames);
    Ok(physical_outputs)
}

fn mix_static(matrix: &CompiledRouteMatrix, source: &PlanarBuffer, destination: &mut PlanarBuffer, frames: usize) {
    for route in &matrix.routes {
        for frame in 0..frames {
            let mixed = destination.sample(route.destination, frame) + source.sample(route.source, frame) * route.gain;
            destination.set_sample(route.destination, frame, mixed);
        }
    }
}

fn validate_io(
    plan: &RenderPlan,
    physical_inputs: &PlanarBuffer,
    playback_inputs: &PlanarBuffer,
    frames: usize,
) -> Result<(), AudioError> {
    if frames == 0 {
        return Err(AudioError::invalid_configuration(
            "reference render frame count must be greater than zero",
        ));
    }
    if physical_inputs.channels() < plan.physical_inputs.len()
        || playback_inputs.channels() < plan.playback_source_channels
    {
        return Err(AudioError::new(
            AudioErrorCategory::CapacityExceeded,
            "reference render I/O channel capacity is smaller than the compiled plan",
        ));
    }
    physical_inputs.validate_range(0, frames)?;
    playback_inputs.validate_range(0, frames)?;
    Ok(())
}
