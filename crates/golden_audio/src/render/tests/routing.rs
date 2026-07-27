use crate::{
    AudioEngineConfig, EngineLimits, GainDb, MonitorRoute, PlanarBuffer, RenderPlanCompiler, RenderProcessor,
    render_scalar_reference,
};

use super::support::{compile, one_to_one_fixture, physical, route_id};

#[test]
fn empty_graph_renders_silence() {
    let fixture = one_to_one_fixture(0, false);
    let plan = compile(&fixture);
    let mut processor = RenderProcessor::new(plan).unwrap();
    let input = PlanarBuffer::new(0, 17).unwrap();
    let playback = PlanarBuffer::new(0, 17).unwrap();
    let mut output = PlanarBuffer::new(0, 17).unwrap();
    processor.render(&input, &playback, &mut output, 17).unwrap();
    assert_eq!(output.channels(), 0);
}

#[test]
fn monitoring_and_playback_mix_before_fader_master_and_output_patch() {
    let mut fixture = one_to_one_fixture(1, true);
    fixture.config.input_patch[0].gain = GainDb::new(-6.0206).unwrap();
    fixture.config.monitoring[0].gain = GainDb::new(-6.0206).unwrap();
    fixture.config.playback_patch[0].gain = GainDb::new(-6.0206).unwrap();
    fixture.config.virtual_outputs[0].gain = GainDb::new(-6.0206).unwrap();
    fixture.config.master_gain = GainDb::new(-6.0206).unwrap();
    fixture.config.output_patch[0].gain = GainDb::new(-6.0206).unwrap();
    let plan = compile(&fixture);
    let mut processor = RenderProcessor::new(plan).unwrap();
    let mut input = PlanarBuffer::new(1, 1).unwrap();
    input.set_sample(0, 0, 1.0);
    let mut playback = PlanarBuffer::new(1, 1).unwrap();
    playback.set_sample(0, 0, 1.0);
    let mut output = PlanarBuffer::new(1, 1).unwrap();
    processor.render(&input, &playback, &mut output, 1).unwrap();

    let half = GainDb::new(-6.0206).unwrap().to_linear();
    let expected = (half * half + half) * half * half * half;
    assert!((output.sample(0, 0) - expected).abs() < 0.000_01);
}

#[test]
fn many_to_one_and_fanout_routes_sum_independently() {
    let mut fixture = one_to_one_fixture(2, false);
    fixture.config.monitoring.push(MonitorRoute {
        id: route_id(8, 0),
        source: fixture.inputs[1],
        destination: fixture.outputs[0],
        gain: GainDb::UNITY,
    });
    fixture.config.monitoring.push(MonitorRoute {
        id: route_id(8, 1),
        source: fixture.inputs[0],
        destination: fixture.outputs[1],
        gain: GainDb::UNITY,
    });
    let plan = compile(&fixture);
    let mut processor = RenderProcessor::new(plan).unwrap();
    let mut input = PlanarBuffer::new(2, 1).unwrap();
    input.set_sample(0, 0, 0.25);
    input.set_sample(1, 0, 0.5);
    let playback = PlanarBuffer::new(0, 1).unwrap();
    let mut output = PlanarBuffer::new(2, 1).unwrap();
    processor.render(&input, &playback, &mut output, 1).unwrap();
    assert!((output.sample(0, 0) - 0.75).abs() < f32::EPSILON);
    assert!((output.sample(1, 0) - 0.75).abs() < f32::EPSILON);
}

#[test]
fn arbitrary_callback_sizes_are_chunked_without_signal_change() {
    for frames in [1, 17, 64, 127, 128, 511, 1_025] {
        let fixture = one_to_one_fixture(1, false);
        let plan = compile(&fixture);
        let mut processor = RenderProcessor::new(plan).unwrap();
        let mut input = PlanarBuffer::new(1, frames).unwrap();
        for frame in 0..frames {
            input.set_sample(0, frame, frame as f32 / frames as f32);
        }
        let playback = PlanarBuffer::new(0, frames).unwrap();
        let mut output = PlanarBuffer::new(1, frames).unwrap();
        processor.render(&input, &playback, &mut output, frames).unwrap();
        assert_eq!(input.channel(0), output.channel(0));
    }
}

#[test]
fn master_gain_changes_ramp_sample_accurately_across_internal_chunks() {
    let fixture = one_to_one_fixture(1, false);
    let plan = compile(&fixture);
    let ramp_frames = plan.gain_ramp_frames as usize;
    let mut processor = RenderProcessor::new(plan).unwrap();
    processor.set_master_gain(GainDb::SILENCE);
    let mut input = PlanarBuffer::new(1, ramp_frames).unwrap();
    input.channel_mut(0).fill(1.0);
    let playback = PlanarBuffer::new(0, ramp_frames).unwrap();
    let mut output = PlanarBuffer::new(1, ramp_frames).unwrap();
    processor.render(&input, &playback, &mut output, ramp_frames).unwrap();
    assert!(output.sample(0, 0) < 1.0);
    assert!(output.sample(0, 0) > 0.0);
    assert_eq!(output.sample(0, ramp_frames - 1), 0.0);
}

#[test]
fn compiler_is_deterministic_and_rejects_missing_physical_channels() {
    let mut fixture = one_to_one_fixture(2, true);
    let compiler = RenderPlanCompiler::new(AudioEngineConfig::default(), EngineLimits::default());
    let expected = compiler.compile(&fixture.config, &fixture.context).unwrap().plan;
    fixture.config.virtual_inputs.reverse();
    fixture.config.virtual_outputs.reverse();
    fixture.config.input_patch.reverse();
    fixture.config.monitoring.reverse();
    fixture.config.playback_patch.reverse();
    fixture.config.output_patch.reverse();
    let reordered = compiler.compile(&fixture.config, &fixture.context).unwrap().plan;
    assert_eq!(expected, reordered);

    fixture.config.physical_inputs.clear();
    assert!(compiler.compile(&fixture.config, &fixture.context).is_err());
}

#[test]
fn full_physical_inventory_preserves_non_contiguous_route_indices() {
    let mut fixture = one_to_one_fixture(1, false);
    fixture.config.physical_inputs = (0..6).map(|index| physical("input", index)).collect();
    fixture.config.physical_outputs = (0..6).map(|index| physical("output", index)).collect();
    fixture.config.input_patch[0].source = physical("input", 4);
    fixture.config.output_patch[0].destination = physical("output", 5);

    let plan = compile(&fixture);
    assert_eq!(plan.physical_inputs, fixture.config.physical_inputs);
    assert_eq!(plan.physical_outputs, fixture.config.physical_outputs);
    assert_eq!(plan.input_patch.source_channels, 6);
    assert_eq!(plan.input_patch.routes[0].source, 4);
    assert_eq!(plan.output_patch.destination_channels, 6);
    assert_eq!(plan.output_patch.routes[0].destination, 5);

    let mut processor = RenderProcessor::new(plan).unwrap();
    let mut input = PlanarBuffer::new(6, 1).unwrap();
    input.set_sample(4, 0, 0.375);
    let playback = PlanarBuffer::new(0, 1).unwrap();
    let mut output = PlanarBuffer::new(6, 1).unwrap();
    processor.render(&input, &playback, &mut output, 1).unwrap();

    assert_eq!(output.sample(5, 0), 0.375);
    for channel in 0..5 {
        assert_eq!(output.sample(channel, 0), 0.0);
    }
}

#[test]
fn optimized_kernel_matches_scalar_reference_for_randomized_sparse_routes() {
    let mut random = Lcg::new(0x5eed_f00d);
    for case in 0..64 {
        let channels = 1 + random.usize(8);
        let frames = 1 + random.usize(257);
        let mut fixture = one_to_one_fixture(channels, true);
        for route_index in 0..random.usize(channels * channels + 1) {
            let source = random.usize(channels);
            let destination = random.usize(channels);
            let gain = -60.0 + random.f32() * 72.0;
            fixture.config.monitoring.push(MonitorRoute {
                id: route_id(16 + case as u8, route_index + channels),
                source: fixture.inputs[source],
                destination: fixture.outputs[destination],
                gain: GainDb::new(gain).unwrap(),
            });
        }
        let plan = RenderPlanCompiler::new(AudioEngineConfig::default(), EngineLimits::default())
            .compile(&fixture.config, &fixture.context)
            .unwrap()
            .plan;
        let mut physical = PlanarBuffer::new(channels, frames).unwrap();
        let mut playback = PlanarBuffer::new(channels, frames).unwrap();
        for channel in 0..channels {
            for frame in 0..frames {
                physical.set_sample(channel, frame, random.f32() * 2.0 - 1.0);
                playback.set_sample(channel, frame, random.f32() * 2.0 - 1.0);
            }
        }
        let expected = render_scalar_reference(&plan, &physical, &playback, frames).unwrap();
        let mut actual = PlanarBuffer::new(channels, frames).unwrap();
        RenderProcessor::new(plan)
            .unwrap()
            .render(&physical, &playback, &mut actual, frames)
            .unwrap();
        for channel in 0..channels {
            for frame in 0..frames {
                assert!(
                    (expected.sample(channel, frame) - actual.sample(channel, frame)).abs() < 0.000_01,
                    "case={case} channel={channel} frame={frame}"
                );
            }
        }
    }
}

#[derive(Clone, Copy)]
struct Lcg(u64);

impl Lcg {
    const fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }

    fn usize(&mut self, maximum: usize) -> usize {
        if maximum == 0 {
            0
        } else {
            (self.next() as usize) % maximum
        }
    }

    fn f32(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1_u32 << 24) as f32
    }
}
