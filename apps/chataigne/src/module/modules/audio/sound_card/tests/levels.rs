use std::time::Duration;

use golden_core::parameter::{ParamValue, RangeConstraint};

use super::*;
use super::super::{
    LEVELS_UPDATE_RATE_DEFAULT_HZ, LEVELS_UPDATE_RATE_MAX_HZ,
    LEVELS_UPDATE_RATE_MIN_HZ,
};

#[test]
fn processing_exposes_bounded_levels_update_rate() {
    let (engine, module) = create_sound_card();
    let snapshot = engine.process_tree_snapshot();
    let update_rate = find_path(
        &snapshot,
        module,
        "parameters/processing/levels_update_rate_hz",
    )
    .expect("levels update rate");
    let state = snapshot.node(update_rate).expect("levels update rate state");

    assert_eq!(state.label, "Levels Update Rate");
    assert_eq!(
        state.param_value,
        Some(ParamValue::Int(LEVELS_UPDATE_RATE_DEFAULT_HZ))
    );
    assert_eq!(
        state
            .param_constraints
            .as_ref()
            .and_then(|constraints| constraints.range.clone()),
        RangeConstraint::uniform(
            Some(f64::from(LEVELS_UPDATE_RATE_MIN_HZ)),
            Some(f64::from(LEVELS_UPDATE_RATE_MAX_HZ)),
        )
    );
}

#[test]
fn levels_update_interval_clamps_to_authored_range() {
    assert_eq!(runtime::levels_update_interval(5), Duration::from_millis(200));
    assert_eq!(runtime::levels_update_interval(0), Duration::from_secs(1));
    assert_eq!(
        runtime::levels_update_interval(31),
        Duration::from_secs_f64(1.0 / 30.0)
    );
}

#[test]
fn output_activity_requires_enabled_finite_signal_at_threshold() {
    assert!(!runtime::has_output_activity(
        false,
        runtime::OUTPUT_ACTIVITY_RMS_THRESHOLD * 2.0,
    ));
    assert!(!runtime::has_output_activity(true, f32::NAN));
    assert!(!runtime::has_output_activity(
        true,
        runtime::OUTPUT_ACTIVITY_RMS_THRESHOLD / 2.0,
    ));
    assert!(runtime::has_output_activity(
        true,
        runtime::OUTPUT_ACTIVITY_RMS_THRESHOLD,
    ));
}

#[test]
fn missing_runtime_clears_active_output_once() {
    let mut module = SoundCardModule::create();
    module.outgoing_activity_active = true;

    let mut first = test_process_ctx(1);
    module.synchronize_runtime_projection(&mut first);
    assert!(!module.outgoing_activity_active);
    let first_events = first
        .edits
        .drain()
        .into_iter()
        .filter_map(|request| match request.edit {
            Edit::EmitCustomEvent { event } => Some(event),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(first_events.len(), 1);
    assert_eq!(
        first_events[0].topic,
        chataigne_sound_card_protocol::SOUND_CARD_OUTPUT_ACTIVITY_TOPIC
    );
    assert_eq!(
        first_events[0].payload.as_ref(),
        &serde_json::Value::Bool(false)
    );

    let mut unchanged = test_process_ctx(2);
    module.synchronize_runtime_projection(&mut unchanged);
    assert!(unchanged.edits.pending.is_empty());
}
