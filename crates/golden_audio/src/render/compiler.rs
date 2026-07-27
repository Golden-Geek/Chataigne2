use std::collections::HashMap;

use crate::{AudioConfiguration, AudioEngineConfig, AudioError, AudioRouteId, EngineLimits, PhysicalChannelKey};

use super::{
    CompiledAnalysisTap, CompiledRoute, CompiledRouteMatrix, RenderPlan, RenderWarning, RenderWarningCode, RouteSpan,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RenderCompileContext {
    pub playback_source_channels: usize,
}

impl RenderCompileContext {
    #[must_use]
    pub fn derive_from_configuration(config: &AudioConfiguration) -> Self {
        let playback_source_channels = config
            .playback_patch
            .iter()
            .map(|route| usize::from(route.source_channel) + 1)
            .max()
            .unwrap_or(0);
        Self {
            playback_source_channels,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderPlanCompilation {
    pub plan: RenderPlan,
    pub warnings: Vec<RenderWarning>,
}

#[derive(Clone, Debug)]
pub struct RenderPlanCompiler {
    engine: AudioEngineConfig,
    limits: EngineLimits,
}

impl RenderPlanCompiler {
    #[must_use]
    pub fn new(engine: AudioEngineConfig, limits: EngineLimits) -> Self {
        Self { engine, limits }
    }

    pub fn compile(
        &self,
        config: &AudioConfiguration,
        context: &RenderCompileContext,
    ) -> Result<RenderPlanCompilation, AudioError> {
        self.engine.validate()?;
        config.validate(&self.limits)?;
        for tap in &config.analysis_taps {
            tap.processor.validate(self.engine.sample_rate, &self.limits)?;
        }
        let mut virtual_inputs = config
            .virtual_inputs
            .iter()
            .map(|channel| channel.id)
            .collect::<Vec<_>>();
        virtual_inputs.sort_unstable();
        let mut virtual_outputs = config
            .virtual_outputs
            .iter()
            .map(|channel| channel.id)
            .collect::<Vec<_>>();
        virtual_outputs.sort_unstable();

        let input_indices = virtual_inputs
            .iter()
            .enumerate()
            .map(|(index, id)| (*id, index))
            .collect::<HashMap<_, _>>();
        let output_indices = virtual_outputs
            .iter()
            .enumerate()
            .map(|(index, id)| (*id, index))
            .collect::<HashMap<_, _>>();
        let physical_input_indices = index_keys(config.physical_inputs.as_slice());
        let physical_output_indices = index_keys(config.physical_outputs.as_slice());
        let mut warnings = Vec::new();

        let input_routes = config
            .input_patch
            .iter()
            .map(|route| CompiledRoute {
                id: route.id,
                source: physical_input_indices[&route.source],
                destination: input_indices[&route.destination],
                gain: route.gain.to_linear(),
            })
            .collect::<Vec<_>>();
        let monitor_routes = config.monitoring.iter().map(|route| CompiledRoute {
            id: route.id,
            source: input_indices[&route.source],
            destination: output_indices[&route.destination],
            gain: route.gain.to_linear(),
        });
        let playback_routes = config
            .playback_patch
            .iter()
            .filter_map(|route| {
                let source = usize::from(route.source_channel);
                if source >= context.playback_source_channels {
                    warnings.push(unresolved_warning(
                        RenderWarningCode::UnresolvedPlaybackChannel,
                        route.id,
                        format!("playback source channel {source} is unavailable"),
                    ));
                    return None;
                }
                Some(CompiledRoute {
                    id: route.id,
                    source,
                    destination: output_indices[&route.destination],
                    gain: route.gain.to_linear(),
                })
            })
            .collect::<Vec<_>>();
        let output_routes = config
            .output_patch
            .iter()
            .map(|route| CompiledRoute {
                id: route.id,
                source: output_indices[&route.source],
                destination: physical_output_indices[&route.destination],
                gain: route.gain.to_linear(),
            })
            .collect::<Vec<_>>();

        let output_gain_by_id = config
            .virtual_outputs
            .iter()
            .map(|channel| (channel.id, channel.gain.to_linear()))
            .collect::<HashMap<_, _>>();
        let output_gains = virtual_outputs.iter().map(|id| output_gain_by_id[id]).collect();
        let gain_ramp_frames = (self.engine.sample_rate.get() as f32 * self.engine.gain_ramp_ms / 1_000.0)
            .round()
            .max(1.0) as u32;
        let rms_window_frames = (self.engine.sample_rate.get() as f32 * self.engine.rms_window_ms / 1_000.0)
            .round()
            .max(1.0) as u32;
        let observation_interval_frames =
            (self.engine.sample_rate.get() / u32::from(self.engine.observation_hz)).max(1);
        let mut analysis_taps = config
            .analysis_taps
            .iter()
            .map(|tap| CompiledAnalysisTap {
                id: tap.id,
                source: tap.source,
                source_index: input_indices[&tap.source],
                enabled: tap.enabled,
                processor: tap.processor,
            })
            .collect::<Vec<_>>();
        analysis_taps.sort_by_key(|tap| tap.id);

        Ok(RenderPlanCompilation {
            plan: RenderPlan {
                sample_rate: self.engine.sample_rate,
                internal_block_frames: self.engine.internal_block_frames,
                observation_hz: self.engine.observation_hz,
                observation_interval_frames,
                rms_window_frames,
                gain_ramp_frames,
                physical_inputs: config.physical_inputs.clone(),
                physical_outputs: config.physical_outputs.clone(),
                virtual_inputs,
                virtual_outputs,
                playback_source_channels: context.playback_source_channels,
                input_patch: compile_matrix(physical_input_indices.len(), input_indices.len(), input_routes),
                monitoring: compile_matrix(input_indices.len(), output_indices.len(), monitor_routes),
                playback_patch: compile_matrix(context.playback_source_channels, output_indices.len(), playback_routes),
                output_patch: compile_matrix(output_indices.len(), physical_output_indices.len(), output_routes),
                output_gains,
                master_gain: config.master_gain.to_linear(),
                analysis_taps,
            },
            warnings,
        })
    }
}

fn index_keys(keys: &[PhysicalChannelKey]) -> HashMap<PhysicalChannelKey, usize> {
    keys.iter()
        .cloned()
        .enumerate()
        .map(|(index, key)| (key, index))
        .collect()
}

fn compile_matrix(
    source_channels: usize,
    destination_channels: usize,
    routes: impl IntoIterator<Item = CompiledRoute>,
) -> CompiledRouteMatrix {
    let mut routes = routes.into_iter().collect::<Vec<_>>();
    routes.sort_by_key(|route| (route.destination, route.source, route.id));
    let mut destination_spans = Vec::with_capacity(destination_channels);
    let mut cursor = 0;
    for destination in 0..destination_channels {
        let start = cursor;
        while cursor < routes.len() && routes[cursor].destination == destination {
            cursor += 1;
        }
        destination_spans.push(RouteSpan { start, end: cursor });
    }
    CompiledRouteMatrix {
        source_channels,
        destination_channels,
        routes,
        destination_spans,
    }
}

fn unresolved_warning(code: RenderWarningCode, route: AudioRouteId, message: String) -> RenderWarning {
    RenderWarning { code, route, message }
}
