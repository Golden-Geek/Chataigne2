use crate::{
    AudioErrorCategory, CommandSequence, GainDb, GainMailboxTarget, RealtimeBarrier, RealtimeBarrierKind,
    RealtimeControlUpdate, acknowledged_plan_exchange, ordered_realtime_controls,
};

fn sequence(value: u64) -> CommandSequence {
    CommandSequence::new(value).unwrap()
}

#[test]
fn gain_updates_coalesce_to_the_latest_sequence() {
    let target = GainMailboxTarget::Master;
    let (mut writer, mut reader) = ordered_realtime_controls([target], 4).unwrap();
    writer
        .set_gain(target, GainDb::new(-3.0).unwrap(), sequence(1))
        .unwrap();
    writer
        .set_gain(target, GainDb::new(-6.0).unwrap(), sequence(2))
        .unwrap();
    writer
        .set_gain(target, GainDb::new(-9.0).unwrap(), sequence(3))
        .unwrap();

    let mut updates = Vec::new();
    reader.begin_block(|update| updates.push(update));

    assert_eq!(updates.len(), 1);
    assert!(matches!(
        updates[0],
        RealtimeControlUpdate::Gain { sequence: found, .. } if found == sequence(3)
    ));
}

#[test]
fn gain_updates_cannot_cross_play_stop_sequence_barriers() {
    let target = GainMailboxTarget::Master;
    let (mut writer, mut reader) = ordered_realtime_controls([target], 8).unwrap();
    writer
        .set_gain(target, GainDb::new(-3.0).unwrap(), sequence(1))
        .unwrap();
    writer
        .push_barrier(RealtimeBarrier {
            sequence: sequence(2),
            token: 44,
            kind: RealtimeBarrierKind::Play,
        })
        .unwrap();
    writer
        .set_gain(target, GainDb::new(-9.0).unwrap(), sequence(3))
        .unwrap();

    let mut updates = Vec::new();
    reader.begin_block(|update| updates.push(update));

    assert_eq!(updates.len(), 3);
    assert!(matches!(
        updates[0],
        RealtimeControlUpdate::Gain { sequence: found, .. } if found == sequence(1)
    ));
    assert!(matches!(
        updates[1],
        RealtimeControlUpdate::Barrier(RealtimeBarrier { sequence: found, .. }) if found == sequence(2)
    ));
    assert!(matches!(
        updates[2],
        RealtimeControlUpdate::Gain { sequence: found, .. } if found == sequence(3)
    ));
}

#[test]
fn barrier_queue_pressure_is_explicit_and_does_not_partially_publish() {
    let target = GainMailboxTarget::Master;
    let (mut writer, mut reader) = ordered_realtime_controls([target], 1).unwrap();
    writer.set_gain(target, GainDb::UNITY, sequence(1)).unwrap();
    let error = writer
        .push_barrier(RealtimeBarrier {
            sequence: sequence(2),
            token: 0,
            kind: RealtimeBarrierKind::StopAll,
        })
        .unwrap_err();
    assert_eq!(error.category, AudioErrorCategory::QueueFull);
    assert_eq!(writer.pressure().realtime_control_full, 1);

    let mut updates = Vec::new();
    reader.begin_block(|update| updates.push(update));
    assert_eq!(updates.len(), 1);
    assert!(matches!(updates[0], RealtimeControlUpdate::Gain { .. }));
}

#[test]
fn million_gain_controls_coalesce_without_queue_growth() {
    const UPDATES: u64 = 1_000_000;
    let target = GainMailboxTarget::Master;
    let (mut writer, mut reader) = ordered_realtime_controls([target], 2).unwrap();
    for value in 1..=UPDATES {
        writer.set_gain(target, GainDb::UNITY, sequence(value)).unwrap();
    }

    let mut emitted = None;
    reader.begin_block(|update| emitted = Some(update));
    assert!(matches!(
        emitted,
        Some(RealtimeControlUpdate::Gain { sequence: found, .. }) if found == sequence(UPDATES)
    ));
}

#[test]
fn plan_swap_barrier_preserves_commands_on_both_sides() {
    let target = GainMailboxTarget::Master;
    let (mut writer, mut reader) = ordered_realtime_controls([target], 8).unwrap();
    let (mut plan_writer, mut plan_reader) = acknowledged_plan_exchange(Box::new("old"));
    writer
        .set_gain(target, GainDb::new(-3.0).unwrap(), sequence(1))
        .unwrap();
    plan_writer.publish(Box::new("new")).unwrap();
    writer
        .push_barrier(RealtimeBarrier {
            sequence: sequence(2),
            token: 0,
            kind: RealtimeBarrierKind::PlanSwap,
        })
        .unwrap();
    writer
        .set_gain(target, GainDb::new(-9.0).unwrap(), sequence(3))
        .unwrap();

    let mut order = Vec::new();
    reader.begin_block(|update| match update {
        RealtimeControlUpdate::Gain { sequence, .. } => order.push(sequence.get()),
        RealtimeControlUpdate::Barrier(barrier) => {
            assert_eq!(barrier.kind, RealtimeBarrierKind::PlanSwap);
            plan_reader.begin_block();
            order.push(barrier.sequence.get());
        }
    });

    assert_eq!(order, [1, 2, 3]);
    assert_eq!(plan_reader.active(), &"new");
    plan_writer.reclaim_acknowledged();
    plan_reader.retire().reclaim();
}
