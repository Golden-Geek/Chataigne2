use golden_audio::{
    AudioChannelId, AudioConfiguration, AudioEngineConfig, AudioRouteId, EngineLimits, GainDb, InputPatchRoute,
    MonitorRoute, OutputPatchRoute, PhysicalChannelKey, PlanarBuffer, RenderCompileContext, RenderPlanCompiler,
    RenderProcessor, VirtualInputChannel, VirtualOutputChannel,
};
use uuid::Uuid;

pub struct RenderFixture {
    pub processor: RenderProcessor,
    pub physical_inputs: PlanarBuffer,
    pub playback_inputs: PlanarBuffer,
    pub physical_outputs: PlanarBuffer,
    pub frames: usize,
}

pub fn fixture(channels: usize, total_routes: usize, frames: usize) -> RenderFixture {
    let inputs = (0..channels).map(|index| channel_id(1, index)).collect::<Vec<_>>();
    let outputs = (0..channels).map(|index| channel_id(2, index)).collect::<Vec<_>>();
    let physical_inputs = (0..channels).map(|index| physical("input", index)).collect::<Vec<_>>();
    let physical_outputs = (0..channels).map(|index| physical("output", index)).collect::<Vec<_>>();
    let mut config = AudioConfiguration::empty();
    config.virtual_inputs = inputs
        .iter()
        .enumerate()
        .map(|(index, id)| VirtualInputChannel {
            id: *id,
            label: format!("Input {index}"),
        })
        .collect();
    config.virtual_outputs = outputs
        .iter()
        .enumerate()
        .map(|(index, id)| VirtualOutputChannel {
            id: *id,
            label: format!("Output {index}"),
            gain: GainDb::UNITY,
        })
        .collect();
    config.input_patch = (0..channels)
        .map(|index| InputPatchRoute {
            id: route_id(1, index),
            source: physical_inputs[index].clone(),
            destination: inputs[index],
            gain: GainDb::UNITY,
        })
        .collect();
    config.output_patch = (0..channels)
        .map(|index| OutputPatchRoute {
            id: route_id(2, index),
            source: outputs[index],
            destination: physical_outputs[index].clone(),
            gain: GainDb::UNITY,
        })
        .collect();
    let monitoring_routes = total_routes.saturating_sub(channels * 2);
    config.monitoring = (0..monitoring_routes)
        .map(|index| MonitorRoute {
            id: route_id(3, index),
            source: inputs[index % channels],
            destination: outputs[(index / channels) % channels],
            gain: GainDb::UNITY,
        })
        .collect();
    let context = RenderCompileContext {
        physical_inputs,
        physical_outputs,
        playback_source_channels: 0,
    };
    let limits = EngineLimits {
        max_routes: total_routes.max(1) as u32,
        ..EngineLimits::default()
    };
    let plan = RenderPlanCompiler::new(AudioEngineConfig::default(), limits)
        .compile(&config, &context)
        .unwrap()
        .plan;
    let mut input = PlanarBuffer::new(channels, frames).unwrap();
    for channel in 0..channels {
        input.channel_mut(channel).fill(0.125);
    }
    RenderFixture {
        processor: RenderProcessor::new(plan).unwrap(),
        physical_inputs: input,
        playback_inputs: PlanarBuffer::new(0, frames).unwrap(),
        physical_outputs: PlanarBuffer::new(channels, frames).unwrap(),
        frames,
    }
}

fn channel_id(group: u8, index: usize) -> AudioChannelId {
    AudioChannelId::from_uuid(Uuid::from_u128((u128::from(group) << 120) | (index as u128 + 1)))
}

fn route_id(group: u8, index: usize) -> AudioRouteId {
    AudioRouteId::from_uuid(Uuid::from_u128((u128::from(group) << 120) | (index as u128 + 1)))
}

fn physical(prefix: &str, index: usize) -> PhysicalChannelKey {
    PhysicalChannelKey::new(format!("{prefix}:{index}")).unwrap()
}
