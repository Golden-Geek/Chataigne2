use golden_audio::{
    AudioChannelId, AudioConfiguration, AudioEngineConfig, AudioRouteId, EngineLimits, GainDb, MonitorRoute,
    OfflineRenderer, OutputPatchRoute, PhysicalChannelKey, PlanarBuffer, PlaybackRoute, RenderCompileContext,
    RenderPlanCompiler, VirtualInputChannel, VirtualOutputChannel,
};
use uuid::Uuid;

#[test]
fn offline_renderer_executes_backend_neutral_signal_graph() {
    let input = AudioChannelId::from_uuid(Uuid::from_u128(1));
    let output = AudioChannelId::from_uuid(Uuid::from_u128(2));
    let physical_output = PhysicalChannelKey::new("offline:out:0").unwrap();
    let mut config = AudioConfiguration::empty();
    config.virtual_inputs.push(VirtualInputChannel {
        id: input,
        label: "Input".to_owned(),
    });
    config.virtual_outputs.push(VirtualOutputChannel {
        id: output,
        label: "Output".to_owned(),
        gain: GainDb::UNITY,
    });
    config.monitoring.push(MonitorRoute {
        id: AudioRouteId::from_uuid(Uuid::from_u128(3)),
        source: input,
        destination: output,
        gain: GainDb::UNITY,
    });
    config.playback_patch.push(PlaybackRoute {
        id: AudioRouteId::from_uuid(Uuid::from_u128(5)),
        source_channel: 0,
        destination: output,
        gain: GainDb::UNITY,
    });
    config.output_patch.push(OutputPatchRoute {
        id: AudioRouteId::from_uuid(Uuid::from_u128(4)),
        source: output,
        destination: physical_output.clone(),
        gain: GainDb::UNITY,
    });
    let context = RenderCompileContext {
        physical_inputs: Vec::new(),
        physical_outputs: vec![physical_output],
        playback_source_channels: 1,
    };
    let plan = RenderPlanCompiler::new(AudioEngineConfig::default(), EngineLimits::default())
        .compile(&config, &context)
        .unwrap()
        .plan;
    let mut renderer = OfflineRenderer::new(plan).unwrap();
    let physical_inputs = PlanarBuffer::new(0, 32).unwrap();
    let mut playback_inputs = PlanarBuffer::new(1, 32).unwrap();
    playback_inputs.channel_mut(0).fill(0.25);
    let output = renderer.render(&physical_inputs, &playback_inputs, 32).unwrap();
    assert!(output.channel(0).iter().all(|sample| *sample == 0.25));
}
