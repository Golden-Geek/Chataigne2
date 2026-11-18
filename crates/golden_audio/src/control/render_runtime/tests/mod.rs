use allocation_counter::measure;

use super::*;
use crate::{
    AudioConfiguration, AudioRouteId, FrameCount, OutputPatchRoute, PhysicalChannelKey, PlaybackRoute,
    RenderCompileContext, RenderPlanCompiler, VirtualOutputChannel, pcm_playback_bus, playback_voice_pool,
};

#[test]
fn external_pcm_uses_the_managed_playback_patch_and_physical_output_bridge() {
    let frames = 128;
    let config = AudioEngineConfig {
        internal_block_frames: FrameCount::new(frames).unwrap(),
        ..Default::default()
    };
    let frames = frames as usize;
    let limits = EngineLimits {
        max_virtual_outputs: 2,
        ..Default::default()
    };
    let (mut writer, reader) = pcm_playback_bus(2, config.internal_block_frames, 4, &limits).unwrap();
    let generation = ConfigGeneration::new(1);
    let mut routing = AudioConfiguration::empty();
    routing.physical_outputs = ["output:0", "output:1"]
        .map(|key| PhysicalChannelKey::new(key).unwrap())
        .into();
    for lane in 0..2 {
        let destination = AudioChannelId::from_uuid(uuid::Uuid::from_u128(u128::from(lane) + 1));
        routing.virtual_outputs.push(VirtualOutputChannel {
            id: destination,
            label: format!("Lane {lane}"),
            gain: GainDb::UNITY,
        });
        routing.playback_patch.push(PlaybackRoute {
            id: AudioRouteId::from_uuid(uuid::Uuid::from_u128(u128::from(lane) + 3)),
            source_channel: lane,
            destination,
            gain: GainDb::UNITY,
        });
        routing.output_patch.push(OutputPatchRoute {
            id: AudioRouteId::from_uuid(uuid::Uuid::from_u128(u128::from(lane) + 5)),
            source: destination,
            destination: routing.physical_outputs[usize::from(1 - lane)].clone(),
            gain: GainDb::UNITY,
        });
    }
    let compiled = RenderPlanCompiler::new(config, limits.clone())
        .compile(&routing, &RenderCompileContext::derive_from_configuration(&routing))
        .unwrap();
    let (plan, _analysis) = RenderRuntimePlan::new(generation, compiled.plan, &limits).unwrap();
    let mut plan = Some(plan);
    let (_voice_control, voices) = playback_voice_pool(4, 2, frames).unwrap();
    let mut playback = PlaybackBus {
        voices,
        pcm: Some(reader),
        buffer: PlanarBuffer::new(2, frames).unwrap(),
    };
    let capacity = frames * 2;
    let (producer, mut consumer) = RingBuffer::new(capacity);
    let mut output = Some(OutputRuntimeBridge {
        producer,
        channels: 2,
        capacity,
        prefilled: Arc::new(AtomicBool::new(false)),
        consumer_started: Arc::new(AtomicBool::new(true)),
    });
    let mut planes = vec![0.75; frames * 2];
    planes[frames..].fill(-0.25);
    assert!(writer.try_write(generation, 0, &planes).unwrap());
    let observation = SharedRenderObservation::default();
    let allocations = measure(|| {
        let _realtime = RealtimeScope::enter();
        render_block(&mut plan, &mut None, &mut output, &mut playback, frames, &observation);
    });
    assert_eq!(allocations.count_total, 0);
    assert_eq!(allocations.bytes_current, 0);
    assert_eq!(writer.cursor().queued_output_frames, frames);
    assert_eq!(writer.cursor().next_frame, frames as u64);
    for _ in 0..frames {
        assert_eq!(consumer.pop().unwrap(), -0.25);
        assert_eq!(consumer.pop().unwrap(), 0.75);
    }
}
