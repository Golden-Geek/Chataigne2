use std::{thread, time::Duration};

use allocation_counter::measure;
use uuid::Uuid;

use crate::{
    AnalysisProcessorConfiguration, AnalysisResult, AnalysisTapConfiguration, AnalysisTapId, AudioChannelId,
    AudioConfiguration, AudioEngineConfig, AudioRouteId, ConfigGeneration, EngineLimits, FrameCount, GainDb,
    InputPatchRoute, MonitorRoute, OutputPatchRoute, PhysicalChannelKey, PitchAnalysisConfiguration, PlanarBuffer,
    RenderCompileContext, RenderPlan, RenderPlanCompiler, RenderProcessor, VirtualInputChannel, VirtualOutputChannel,
    analysis_pipeline,
};

fn id(value: u128) -> AudioChannelId {
    AudioChannelId::from_uuid(Uuid::from_u128(value))
}

fn route(value: u128) -> AudioRouteId {
    AudioRouteId::from_uuid(Uuid::from_u128(value))
}

fn analysis_plan() -> RenderPlan {
    let input = id(1);
    let output = id(2);
    let physical_input = PhysicalChannelKey::new("input:0").unwrap();
    let physical_output = PhysicalChannelKey::new("output:0").unwrap();
    let mut configuration = AudioConfiguration::empty();
    configuration.virtual_inputs.push(VirtualInputChannel {
        id: input,
        label: "Input".to_owned(),
    });
    configuration.virtual_outputs.push(VirtualOutputChannel {
        id: output,
        label: "Output".to_owned(),
        gain: GainDb::UNITY,
    });
    configuration.input_patch.push(InputPatchRoute {
        id: route(1),
        source: physical_input.clone(),
        destination: input,
        gain: GainDb::UNITY,
    });
    configuration.monitoring.push(MonitorRoute {
        id: route(2),
        source: input,
        destination: output,
        gain: GainDb::UNITY,
    });
    configuration.output_patch.push(OutputPatchRoute {
        id: route(3),
        source: output,
        destination: physical_output.clone(),
        gain: GainDb::UNITY,
    });
    configuration.analysis_taps.push(AnalysisTapConfiguration {
        id: AnalysisTapId::from_uuid(Uuid::from_u128(10)),
        source: input,
        enabled: true,
        processor: AnalysisProcessorConfiguration::Pitch(PitchAnalysisConfiguration {
            confidence_threshold: 0.75,
            ..PitchAnalysisConfiguration::default()
        }),
    });
    let engine = AudioEngineConfig {
        internal_block_frames: FrameCount::new(128).unwrap(),
        rms_window_ms: 5.0,
        ..AudioEngineConfig::default()
    };
    RenderPlanCompiler::new(engine, EngineLimits::default())
        .compile(
            &configuration,
            &RenderCompileContext {
                physical_inputs: vec![physical_input],
                physical_outputs: vec![physical_output],
                playback_source_channels: 0,
            },
        )
        .unwrap()
        .plan
}

fn render_tone(processor: &mut RenderProcessor, blocks: usize) {
    let mut input = PlanarBuffer::new(1, 128).unwrap();
    let playback = PlanarBuffer::new(0, 128).unwrap();
    let mut output = PlanarBuffer::new(1, 128).unwrap();
    let start_frame = processor.metrics().rendered_frames;
    for block in 0..blocks {
        for frame in 0..128 {
            let timeline = start_frame + (block * 128 + frame) as u64;
            input.set_sample(
                0,
                frame,
                0.8 * (std::f32::consts::TAU * 440.0 * timeline as f32 / 48_000.0).sin(),
            );
        }
        processor.render(&input, &playback, &mut output, 128).unwrap();
    }
}

#[test]
fn render_pipeline_publishes_generation_safe_meters_pitch_and_diagnostics() {
    let plan = analysis_plan();
    let limits = EngineLimits::default();
    let (mut controller, renderer) = analysis_pipeline(ConfigGeneration::new(7), &plan, &limits).unwrap();
    let observations = controller.observations();
    let mut processor = RenderProcessor::new(plan).unwrap();
    processor.attach_analysis(renderer).unwrap();
    render_tone(&mut processor, 32);

    let mut snapshot = observations.latest();
    for _ in 0..40 {
        snapshot = observations.latest();
        if snapshot.taps[0].result.is_some() {
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(snapshot.generation, ConfigGeneration::new(7));
    assert_eq!(snapshot.inputs.len(), 1);
    assert_eq!(snapshot.outputs.len(), 1);
    assert!((snapshot.inputs[0].rms_linear - 0.8 * std::f32::consts::FRAC_1_SQRT_2).abs() < 0.02);
    assert!((snapshot.outputs[0].rms_linear - snapshot.inputs[0].rms_linear).abs() < 1.0e-6);
    let Some(AnalysisResult::Pitch(pitch)) = &snapshot.taps[0].result else {
        panic!("pitch result was not published: {snapshot:?}");
    };
    assert!(pitch.valid, "{pitch:?}");
    assert!((pitch.frequency_hz - 440.0).abs() < 2.0, "{pitch:?}");
    assert!(snapshot.diagnostics.captured_frames > 0);
    assert!(snapshot.diagnostics.processed_frames > 0);

    controller
        .set_tap_enabled(AnalysisTapId::from_uuid(Uuid::from_u128(10)), false)
        .unwrap();
    let mut settled = observations.latest().diagnostics;
    for _ in 0..40 {
        settled = observations.latest().diagnostics;
        if settled.processed_frames + settled.stale_frames == settled.captured_frames {
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        settled.processed_frames + settled.stale_frames,
        settled.captured_frames,
        "captured frames must reach a terminal state after disabling the tap"
    );
    let processed_before = settled.processed_frames;
    let captured_before = settled.captured_frames;
    render_tone(&mut processor, 24);
    thread::sleep(Duration::from_millis(20));
    let disabled = observations.latest();
    assert!(!disabled.taps[0].enabled);
    assert!(disabled.taps[0].result.is_none());
    assert_eq!(disabled.diagnostics.processed_frames, processed_before);
    assert_eq!(disabled.diagnostics.captured_frames, captured_before);

    let renderer = processor.take_analysis().unwrap();
    controller.shutdown().unwrap();
    renderer.into_retirement().reclaim();
}

#[test]
fn worker_overload_drops_analysis_frames_without_rejecting_audio_capture() {
    let plan = analysis_plan();
    let (mut controller, mut renderer) =
        analysis_pipeline(ConfigGeneration::new(11), &plan, &EngineLimits::default()).unwrap();
    let observations = controller.observations();
    let frames = 2_048 * 40;
    let mut inputs = PlanarBuffer::new(1, frames).unwrap();
    for frame in 0..frames {
        inputs.set_sample(
            0,
            frame,
            (std::f32::consts::TAU * 220.0 * frame as f32 / 48_000.0).sin(),
        );
    }
    renderer.capture_inputs(&inputs, frames, 0).unwrap();
    assert!(observations.latest().diagnostics.dropped_frames > 0);
    controller.shutdown().unwrap();
    renderer.into_retirement().reclaim();
}

#[test]
fn worker_processes_a_bounded_burst_without_discarding_intermediate_frames() {
    let plan = analysis_plan();
    let (mut controller, mut renderer) =
        analysis_pipeline(ConfigGeneration::new(12), &plan, &EngineLimits::default()).unwrap();
    let observations = controller.observations();
    let frames = 2_048 + 3 * 1_024;
    let mut inputs = PlanarBuffer::new(1, frames).unwrap();
    for frame in 0..frames {
        inputs.set_sample(
            0,
            frame,
            (std::f32::consts::TAU * 220.0 * frame as f32 / 48_000.0).sin(),
        );
    }
    renderer.capture_inputs(&inputs, frames, 0).unwrap();

    let mut diagnostics = observations.latest().diagnostics;
    for _ in 0..100 {
        diagnostics = observations.latest().diagnostics;
        if diagnostics.processed_frames == 4 {
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }

    assert_eq!(diagnostics.captured_frames, 4);
    assert_eq!(diagnostics.processed_frames, 4);
    assert_eq!(diagnostics.dropped_frames, 0);
    controller.shutdown().unwrap();
    renderer.into_retirement().reclaim();
}

#[test]
fn warmed_render_with_meter_publication_and_analysis_capture_does_not_allocate() {
    let plan = analysis_plan();
    let (mut controller, renderer) =
        analysis_pipeline(ConfigGeneration::new(3), &plan, &EngineLimits::default()).unwrap();
    let mut processor = RenderProcessor::new(plan).unwrap();
    processor.attach_analysis(renderer).unwrap();
    let input = PlanarBuffer::new(1, 256).unwrap();
    let playback = PlanarBuffer::new(0, 256).unwrap();
    let mut output = PlanarBuffer::new(1, 256).unwrap();
    for _ in 0..6 {
        processor.render(&input, &playback, &mut output, 256).unwrap();
    }
    let allocations = measure(|| {
        processor.render(&input, &playback, &mut output, 256).unwrap();
    });
    assert_eq!(allocations.count_total, 0, "{allocations:?}");
    assert_eq!(allocations.count_current, 0, "{allocations:?}");
    assert_eq!(allocations.bytes_total, 0, "{allocations:?}");
    assert_eq!(allocations.bytes_current, 0, "{allocations:?}");

    let renderer = processor.take_analysis().unwrap();
    controller.shutdown().unwrap();
    renderer.into_retirement().reclaim();
}
