use uuid::Uuid;

use crate::{
    AudioChannelId, AudioConfiguration, AudioEngineConfig, AudioRouteId, EngineLimits, GainDb, InputPatchRoute,
    MonitorRoute, OutputPatchRoute, PhysicalChannelKey, PlaybackRoute, RenderCompileContext, RenderPlan,
    RenderPlanCompiler, VirtualInputChannel, VirtualOutputChannel,
};

pub(super) struct Fixture {
    pub(super) config: AudioConfiguration,
    pub(super) context: RenderCompileContext,
    pub(super) inputs: Vec<AudioChannelId>,
    pub(super) outputs: Vec<AudioChannelId>,
}

pub(super) fn channel_id(group: u8, index: usize) -> AudioChannelId {
    let value = (u128::from(group) << 120) | (index as u128 + 1);
    AudioChannelId::from_uuid(Uuid::from_u128(value))
}

pub(super) fn route_id(group: u8, index: usize) -> AudioRouteId {
    let value = (u128::from(group) << 120) | (index as u128 + 1);
    AudioRouteId::from_uuid(Uuid::from_u128(value))
}

pub(super) fn physical(prefix: &str, index: usize) -> PhysicalChannelKey {
    PhysicalChannelKey::new(format!("{prefix}:{index}")).unwrap()
}

pub(super) fn one_to_one_fixture(channels: usize, playback: bool) -> Fixture {
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
            label: format!("Input {}", index + 1),
        })
        .collect();
    config.virtual_outputs = outputs
        .iter()
        .enumerate()
        .map(|(index, id)| VirtualOutputChannel {
            id: *id,
            label: format!("Output {}", index + 1),
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
    config.monitoring = (0..channels)
        .map(|index| MonitorRoute {
            id: route_id(2, index),
            source: inputs[index],
            destination: outputs[index],
            gain: GainDb::UNITY,
        })
        .collect();
    config.output_patch = (0..channels)
        .map(|index| OutputPatchRoute {
            id: route_id(4, index),
            source: outputs[index],
            destination: physical_outputs[index].clone(),
            gain: GainDb::UNITY,
        })
        .collect();
    if playback {
        config.playback_patch = (0..channels)
            .map(|index| PlaybackRoute {
                id: route_id(3, index),
                source_channel: index as u16,
                destination: outputs[index],
                gain: GainDb::UNITY,
            })
            .collect();
    }
    Fixture {
        config,
        context: RenderCompileContext {
            physical_inputs,
            physical_outputs,
            playback_source_channels: if playback { channels } else { 0 },
        },
        inputs,
        outputs,
    }
}

pub(super) fn compile(fixture: &Fixture) -> RenderPlan {
    RenderPlanCompiler::new(AudioEngineConfig::default(), EngineLimits::default())
        .compile(&fixture.config, &fixture.context)
        .unwrap()
        .plan
}
