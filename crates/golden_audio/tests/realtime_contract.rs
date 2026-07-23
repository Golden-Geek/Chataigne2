use golden_audio::{
    CommandSequence, GainDb, GainMailboxTarget, PlanSwapResult, RealtimeControlUpdate, acknowledged_plan_exchange,
    ordered_realtime_controls,
};

#[test]
fn external_consumer_can_drive_the_callback_safe_contract() {
    let (mut publisher, mut realtime) = acknowledged_plan_exchange(Box::new(1_u32));
    publisher.publish(Box::new(2)).unwrap();
    assert_eq!(realtime.begin_block(), PlanSwapResult::Swapped);
    assert_eq!(*realtime.active(), 2);
    assert_eq!(publisher.reclaim_acknowledged(), 1);

    let (mut controls, mut callback) = ordered_realtime_controls([GainMailboxTarget::Master], 2).unwrap();
    let sequence = CommandSequence::new(1).unwrap();
    controls
        .set_gain(GainMailboxTarget::Master, GainDb::UNITY, sequence)
        .unwrap();
    let mut update = None;
    callback.begin_block(|value| update = Some(value));
    assert!(matches!(
        update,
        Some(RealtimeControlUpdate::Gain {
            sequence: found,
            ..
        }) if found == sequence
    ));

    realtime.retire().reclaim();
}
