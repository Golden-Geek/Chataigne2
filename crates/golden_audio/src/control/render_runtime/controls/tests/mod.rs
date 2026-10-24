use allocation_counter::measure;
use uuid::Uuid;

use super::super::RenderRuntimePlan;
use super::{ControlKind, ControlRejection, GainTarget, RenderControl, RenderControlReader, render_controls};
use crate::{
    AnalysisController, AudioChannelId, AudioConfiguration, AudioEngineConfig, AudioErrorCategory, AudioRouteId,
    CommandSequence, ConfigGeneration, EngineLimits, GainDb, InputPatchRoute, MonitorRoute, OutputPatchRoute,
    PhysicalChannelKey, PlanarBuffer, RealtimeScope, RenderCompileContext, RenderPlanCompiler, VirtualInputChannel,
    VirtualOutputChannel,
};

#[derive(Default)]
struct Fixture {
    analysis: Vec<AnalysisController>,
}

fn channel(index: u128) -> AudioChannelId {
    AudioChannelId::from_uuid(Uuid::from_u128(index))
}

fn sequence(index: u64) -> CommandSequence {
    CommandSequence::new(index).unwrap()
}

impl Fixture {
    fn plan(&mut self, generation: u64, outputs: &[u128], gain: GainDb) -> RenderRuntimePlan {
        let input = channel(100);
        let physical_input = PhysicalChannelKey::new("input").unwrap();
        let mut configuration = AudioConfiguration::empty();
        configuration.master_gain = gain;
        configuration.physical_inputs = vec![physical_input.clone()];
        configuration.virtual_inputs = vec![VirtualInputChannel {
            id: input,
            label: "Input".into(),
        }];
        configuration.input_patch = vec![InputPatchRoute {
            id: AudioRouteId::from_uuid(Uuid::from_u128(100)),
            source: physical_input,
            destination: input,
            gain: GainDb::UNITY,
        }];
        for id in outputs {
            let output = channel(*id);
            let physical = PhysicalChannelKey::new(format!("output-{id}")).unwrap();
            configuration.physical_outputs.push(physical.clone());
            configuration.virtual_outputs.push(VirtualOutputChannel {
                id: output,
                label: format!("Output {id}"),
                gain: GainDb::UNITY,
            });
            configuration.monitoring.push(MonitorRoute {
                id: AudioRouteId::from_uuid(Uuid::from_u128(*id + 200)),
                source: input,
                destination: output,
                gain: GainDb::UNITY,
            });
            configuration.output_patch.push(OutputPatchRoute {
                id: AudioRouteId::from_uuid(Uuid::from_u128(*id + 300)),
                source: output,
                destination: physical,
                gain: GainDb::UNITY,
            });
        }
        let limits = EngineLimits::default();
        let plan = RenderPlanCompiler::new(AudioEngineConfig::default(), limits.clone())
            .compile(
                &configuration,
                &RenderCompileContext::derive_from_configuration(&configuration),
            )
            .unwrap()
            .plan;
        let (mut runtime, analysis) = RenderRuntimePlan::new(ConfigGeneration::new(generation), plan, &limits).unwrap();
        runtime.physical_inputs.channel_mut(0).fill(1.0);
        self.analysis.push(analysis);
        runtime
    }
}

fn begin(reader: &mut RenderControlReader) {
    let _scope = RealtimeScope::enter();
    reader.begin_block(|rejection| panic!("unexpected render control rejection: {rejection:?}"));
}

fn render(reader: &mut RenderControlReader, playback: &PlanarBuffer) {
    let plan = reader.active_mut().as_mut().unwrap();
    let frames = plan.processor.plan().internal_block_frames.get() as usize;
    for _ in 0..4 {
        plan.processor
            .render(&plan.physical_inputs, playback, &mut plan.physical_outputs, frames)
            .unwrap();
    }
}

fn assert_output(reader: &mut RenderControlReader, index: usize, expected: f32) {
    let plan = reader.active_mut().as_mut().unwrap();
    let frames = plan.processor.plan().internal_block_frames.get() as usize;
    let actual = plan.physical_outputs.sample(index, frames - 1);
    assert!(
        (actual - expected).abs() < 0.00001,
        "output {index}: {actual} != {expected}"
    );
}

fn playback() -> PlanarBuffer {
    PlanarBuffer::new(0, AudioEngineConfig::default().internal_block_frames.get() as usize).unwrap()
}

#[test]
fn older_gains_cannot_overwrite_a_later_configuration() {
    let mut fixture = Fixture::default();
    let (mut writer, mut reader) = render_controls(8);
    let playback = playback();
    writer
        .publish_plan(sequence(1), fixture.plan(1, &[1], GainDb::UNITY))
        .unwrap();
    begin(&mut reader);
    assert_eq!(writer.reclaim_acknowledged(), 1);

    writer.set_master_gain(sequence(2), GainDb::SILENCE).unwrap();
    writer
        .set_channel_gain(sequence(3), channel(1), GainDb::SILENCE)
        .unwrap();
    writer
        .publish_plan(sequence(4), fixture.plan(2, &[1], GainDb::UNITY))
        .unwrap();
    begin(&mut reader);
    render(&mut reader, &playback);
    assert_output(&mut reader, 0, 1.0);

    writer.set_master_gain(sequence(5), GainDb::new(-6.0).unwrap()).unwrap();
    writer
        .set_channel_gain(sequence(6), channel(1), GainDb::new(-12.0).unwrap())
        .unwrap();
    begin(&mut reader);
    render(&mut reader, &playback);
    assert_output(&mut reader, 0, GainDb::new(-18.0).unwrap().to_linear());
    reader.retire().reclaim();
}

#[test]
fn removed_and_readded_identity_never_retargets_an_older_gain() {
    let mut fixture = Fixture::default();
    let (mut writer, mut reader) = render_controls(8);
    let playback = playback();
    writer
        .publish_plan(sequence(1), fixture.plan(1, &[1, 2], GainDb::UNITY))
        .unwrap();
    begin(&mut reader);
    writer.reclaim_acknowledged();
    writer
        .set_channel_gain(sequence(2), channel(1), GainDb::SILENCE)
        .unwrap();
    writer
        .publish_plan(sequence(3), fixture.plan(2, &[2], GainDb::UNITY))
        .unwrap();
    assert_eq!(
        writer
            .set_channel_gain(sequence(4), channel(1), GainDb::SILENCE)
            .unwrap_err()
            .category,
        AudioErrorCategory::InvalidConfiguration
    );
    writer
        .set_channel_gain(sequence(4), channel(2), GainDb::new(-6.0).unwrap())
        .unwrap();
    begin(&mut reader);
    render(&mut reader, &playback);
    assert_output(&mut reader, 0, GainDb::new(-6.0).unwrap().to_linear());
    writer.reclaim_acknowledged();

    writer
        .publish_plan(sequence(5), fixture.plan(3, &[1, 2], GainDb::UNITY))
        .unwrap();
    writer
        .set_channel_gain(sequence(6), channel(1), GainDb::new(-12.0).unwrap())
        .unwrap();
    begin(&mut reader);
    render(&mut reader, &playback);
    assert_output(&mut reader, 0, GainDb::new(-12.0).unwrap().to_linear());
    assert_output(&mut reader, 1, 1.0);
    reader.retire().reclaim();
}

#[test]
fn full_control_queue_cannot_publish_an_unordered_plan() {
    let mut fixture = Fixture::default();
    let (mut writer, mut reader) = render_controls(2);
    let playback = playback();
    writer
        .publish_plan(sequence(1), fixture.plan(1, &[1], GainDb::UNITY))
        .unwrap();
    begin(&mut reader);
    writer.reclaim_acknowledged();
    writer.set_master_gain(sequence(2), GainDb::SILENCE).unwrap();
    writer
        .set_channel_gain(sequence(3), channel(1), GainDb::SILENCE)
        .unwrap();
    assert_eq!(
        writer
            .publish_plan(sequence(4), fixture.plan(2, &[2], GainDb::UNITY))
            .unwrap_err()
            .category,
        AudioErrorCategory::QueueFull
    );
    begin(&mut reader);
    render(&mut reader, &playback);
    assert_eq!(
        reader.active_mut().as_ref().unwrap().generation,
        ConfigGeneration::new(1)
    );
    assert_output(&mut reader, 0, 0.0);
    writer
        .publish_plan(sequence(4), fixture.plan(2, &[2], GainDb::UNITY))
        .unwrap();
    writer
        .set_channel_gain(sequence(5), channel(2), GainDb::new(-6.0).unwrap())
        .unwrap();
    begin(&mut reader);
    render(&mut reader, &playback);
    assert_output(&mut reader, 0, GainDb::new(-6.0).unwrap().to_linear());
    reader.retire().reclaim();
}

#[test]
fn rejected_pending_plan_preserves_the_accepted_target_layout() {
    let mut fixture = Fixture::default();
    let (mut writer, mut reader) = render_controls(8);
    let playback = playback();
    writer
        .publish_plan(sequence(1), fixture.plan(1, &[1], GainDb::UNITY))
        .unwrap();
    assert_eq!(
        writer
            .publish_plan(sequence(2), fixture.plan(2, &[2], GainDb::UNITY))
            .unwrap_err()
            .category,
        AudioErrorCategory::QueueFull
    );
    writer
        .set_channel_gain(sequence(2), channel(1), GainDb::SILENCE)
        .unwrap();
    begin(&mut reader);
    render(&mut reader, &playback);
    assert_output(&mut reader, 0, 0.0);
    assert_eq!(
        reader.active_mut().as_ref().unwrap().generation,
        ConfigGeneration::new(1)
    );
    reader.retire().reclaim();
}

#[test]
fn publication_preserves_acknowledgement_for_control_sidecar_retirement() {
    let mut fixture = Fixture::default();
    let (mut writer, mut reader) = render_controls(8);
    writer
        .publish_plan(sequence(1), fixture.plan(1, &[1], GainDb::UNITY))
        .unwrap();
    begin(&mut reader);
    assert_eq!(
        writer
            .publish_plan(sequence(2), fixture.plan(2, &[2], GainDb::UNITY))
            .unwrap_err()
            .category,
        AudioErrorCategory::QueueFull
    );
    assert_eq!(writer.reclaim_acknowledged(), 1);
    writer
        .publish_plan(sequence(2), fixture.plan(2, &[2], GainDb::UNITY))
        .unwrap();
    begin(&mut reader);
    assert_eq!(writer.reclaim_acknowledged(), 1);
    reader.retire().reclaim();
}

#[test]
fn saturated_retirement_preserves_plan_barrier_and_following_gains() {
    let mut fixture = Fixture::default();
    let (mut writer, mut reader) = render_controls(8);
    let playback = playback();
    writer
        .publish_plan(sequence(1), fixture.plan(1, &[1], GainDb::UNITY))
        .unwrap();
    reader.plans.fill_return_queue_for_test(Box::new(None));
    begin(&mut reader);
    writer.set_master_gain(sequence(2), GainDb::SILENCE).unwrap();
    begin(&mut reader);
    render(&mut reader, &playback);
    assert_output(&mut reader, 0, 1.0);
    assert_eq!(writer.reclaim_acknowledged(), 1);
    assert!(writer.plans.has_pending_plan());
    begin(&mut reader);
    assert_eq!(writer.reclaim_acknowledged(), 1);
    assert!(!writer.plans.has_pending_plan());
    begin(&mut reader);
    render(&mut reader, &playback);
    assert_output(&mut reader, 0, 0.0);

    writer
        .publish_plan(sequence(3), fixture.plan(2, &[2], GainDb::UNITY))
        .unwrap();
    writer
        .set_channel_gain(sequence(4), channel(2), GainDb::new(-6.0).unwrap())
        .unwrap();
    begin(&mut reader);
    render(&mut reader, &playback);
    assert_output(&mut reader, 0, GainDb::new(-6.0).unwrap().to_linear());
    reader.retire().reclaim();
}

fn raw_gain(sequence_number: u64, generation: u64, target: GainTarget) -> RenderControl {
    RenderControl {
        sequence: sequence(sequence_number),
        generation: ConfigGeneration::new(generation),
        kind: ControlKind::Gain { target, linear: 0.0 },
    }
}

#[test]
fn stale_missing_and_invalid_targets_have_explicit_nonallocating_outcomes() {
    let mut fixture = Fixture::default();
    let (mut writer, mut reader) = render_controls(8);
    writer.controls.push(raw_gain(1, 0, GainTarget::Master)).unwrap();
    let mut reasons = Vec::new();
    reader.begin_block(|reason| reasons.push(reason));
    assert_eq!(reasons, [ControlRejection::MissingPlan]);
    writer
        .publish_plan(sequence(2), fixture.plan(2, &[1], GainDb::UNITY))
        .unwrap();
    begin(&mut reader);
    writer.controls.push(raw_gain(3, 1, GainTarget::Output(0))).unwrap();
    writer
        .controls
        .push(raw_gain(4, 2, GainTarget::Output(usize::MAX)))
        .unwrap();
    writer.controls.push(raw_gain(2, 2, GainTarget::Master)).unwrap();
    let mut rejected = [0; 3];
    let mut count = 0;
    let allocations = measure(|| {
        let _scope = RealtimeScope::enter();
        reader.begin_block(|reason| {
            rejected[count] = match reason {
                ControlRejection::GenerationMismatch => 1,
                ControlRejection::InvalidTarget => 2,
                ControlRejection::OutOfOrder => 3,
                ControlRejection::MissingPlan => 4,
            };
            count += 1;
        });
    });
    assert_eq!(rejected, [1, 2, 3]);
    assert_zero_allocations(allocations);
    let playback = playback();
    render(&mut reader, &playback);
    assert_output(&mut reader, 0, 1.0);
    reader.retire().reclaim();
}

#[test]
fn a_control_turn_consumes_only_its_initial_bounded_prefix() {
    let mut fixture = Fixture::default();
    let (mut writer, mut reader) = render_controls(2);
    writer
        .publish_plan(sequence(1), fixture.plan(1, &[1], GainDb::UNITY))
        .unwrap();
    begin(&mut reader);
    writer.controls.push(raw_gain(2, 0, GainTarget::Master)).unwrap();
    writer.controls.push(raw_gain(3, 0, GainTarget::Master)).unwrap();
    let mut next_sequence = 4;
    reader.begin_block(|reason| {
        assert_eq!(reason, ControlRejection::GenerationMismatch);
        writer
            .controls
            .push(raw_gain(next_sequence, 0, GainTarget::Master))
            .unwrap();
        next_sequence += 1;
    });
    assert_eq!(next_sequence, 6);
    assert_eq!(reader.controls.slots(), 2);
    reader.retire().reclaim();
}

fn assert_zero_allocations(allocations: allocation_counter::AllocationInfo) {
    assert_eq!(allocations.count_total, 0, "{allocations:?}");
    assert_eq!(allocations.count_current, 0, "{allocations:?}");
    assert_eq!(allocations.bytes_total, 0, "{allocations:?}");
    assert_eq!(allocations.bytes_current, 0, "{allocations:?}");
}

#[test]
fn production_plan_and_gain_transitions_allocate_and_deallocate_nothing() {
    let mut fixture = Fixture::default();
    let (mut writer, mut reader) = render_controls(8);
    let playback = playback();
    writer
        .publish_plan(sequence(1), fixture.plan(1, &[1], GainDb::UNITY))
        .unwrap();
    assert_zero_allocations(measure(|| {
        let _scope = RealtimeScope::enter();
        begin(&mut reader);
        render(&mut reader, &playback);
    }));
    writer.reclaim_acknowledged();
    writer
        .set_channel_gain(sequence(2), channel(1), GainDb::SILENCE)
        .unwrap();
    writer
        .publish_plan(sequence(3), fixture.plan(2, &[2], GainDb::UNITY))
        .unwrap();
    writer
        .set_channel_gain(sequence(4), channel(2), GainDb::new(-6.0).unwrap())
        .unwrap();
    assert_zero_allocations(measure(|| {
        let _scope = RealtimeScope::enter();
        begin(&mut reader);
        render(&mut reader, &playback);
    }));
    assert_output(&mut reader, 0, GainDb::new(-6.0).unwrap().to_linear());
    writer.reclaim_acknowledged();
    writer
        .publish_plan(sequence(5), fixture.plan(3, &[1, 2], GainDb::UNITY))
        .unwrap();
    reader.plans.fill_return_queue_for_test(Box::new(None));
    writer
        .set_channel_gain(sequence(6), channel(1), GainDb::new(-12.0).unwrap())
        .unwrap();
    assert_zero_allocations(measure(|| {
        let _scope = RealtimeScope::enter();
        begin(&mut reader);
        render(&mut reader, &playback);
        begin(&mut reader);
    }));
    assert_output(&mut reader, 0, GainDb::new(-12.0).unwrap().to_linear());
    assert_output(&mut reader, 1, 1.0);
    writer.reclaim_acknowledged();
    assert_zero_allocations(measure(|| begin(&mut reader)));
    reader.retire().reclaim();
}

#[test]
fn invalid_sequence_generation_or_disconnection_cannot_change_admitted_state() {
    let mut fixture = Fixture::default();
    let (mut writer, mut reader) = render_controls(8);
    assert!(writer.set_master_gain(sequence(1), GainDb::UNITY).is_err());
    writer
        .publish_plan(sequence(1), fixture.plan(1, &[1], GainDb::UNITY))
        .unwrap();
    begin(&mut reader);
    writer.reclaim_acknowledged();
    assert!(writer.set_master_gain(sequence(1), GainDb::SILENCE).is_err());
    assert!(
        writer
            .publish_plan(sequence(2), fixture.plan(1, &[2], GainDb::UNITY))
            .is_err()
    );
    writer.set_channel_gain(sequence(2), channel(1), GainDb::UNITY).unwrap();
    reader.retire().reclaim();
    assert_eq!(
        writer
            .set_master_gain(sequence(3), GainDb::SILENCE)
            .unwrap_err()
            .category,
        AudioErrorCategory::ShuttingDown
    );
}
