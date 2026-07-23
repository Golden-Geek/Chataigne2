use crate::{AudioError, AudioRouteId};

use super::{CompiledRouteMatrix, GainSmoother, PlanarBuffer};

#[derive(Clone, Debug)]
pub(crate) struct RouteMatrixState {
    gains: Vec<GainSmoother>,
}

impl RouteMatrixState {
    pub(crate) fn new(matrix: &CompiledRouteMatrix) -> Self {
        Self {
            gains: matrix
                .routes
                .iter()
                .map(|route| GainSmoother::settled(route.gain))
                .collect(),
        }
    }

    pub(crate) fn set_target(
        &mut self,
        matrix: &CompiledRouteMatrix,
        route_id: AudioRouteId,
        target: f32,
        ramp_frames: u32,
    ) -> bool {
        let Some(index) = matrix.routes.iter().position(|route| route.id == route_id) else {
            return false;
        };
        self.gains[index].set_target(target, ramp_frames);
        true
    }
}

pub(crate) struct RouteMix<'a> {
    pub(crate) source: &'a PlanarBuffer,
    pub(crate) source_frame_offset: usize,
    pub(crate) destination: &'a mut PlanarBuffer,
    pub(crate) destination_frame_offset: usize,
    pub(crate) frames: usize,
    pub(crate) clear_destination: bool,
}

pub(crate) fn mix_routes(
    matrix: &CompiledRouteMatrix,
    state: &mut RouteMatrixState,
    mix: RouteMix<'_>,
) -> Result<(), AudioError> {
    validate_mix_shape(matrix, &mix)?;
    if mix.clear_destination {
        mix.destination.zero_range(mix.destination_frame_offset, mix.frames)?;
    }
    for (destination, span) in matrix.destination_spans.iter().enumerate() {
        let destination_samples = &mut mix.destination.channel_mut(destination)
            [mix.destination_frame_offset..mix.destination_frame_offset + mix.frames];
        for route_index in span.start..span.end {
            let route = &matrix.routes[route_index];
            let source_samples =
                &mix.source.channel(route.source)[mix.source_frame_offset..mix.source_frame_offset + mix.frames];
            let gain = &mut state.gains[route_index];
            for (destination_sample, source_sample) in destination_samples.iter_mut().zip(source_samples) {
                *destination_sample += source_sample * gain.next_gain();
            }
        }
    }
    Ok(())
}

fn validate_mix_shape(matrix: &CompiledRouteMatrix, mix: &RouteMix<'_>) -> Result<(), AudioError> {
    if mix.source.channels() < matrix.source_channels {
        return Err(AudioError::invalid_configuration(format!(
            "route source has {} channels but plan requires {}",
            mix.source.channels(),
            matrix.source_channels
        )));
    }
    if mix.destination.channels() < matrix.destination_channels {
        return Err(AudioError::invalid_configuration(format!(
            "route destination has {} channels but plan requires {}",
            mix.destination.channels(),
            matrix.destination_channels
        )));
    }
    mix.source.validate_range(mix.source_frame_offset, mix.frames)?;
    mix.destination
        .validate_range(mix.destination_frame_offset, mix.frames)?;
    Ok(())
}
