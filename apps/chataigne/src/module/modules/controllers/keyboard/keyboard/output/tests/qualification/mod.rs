use super::*;
use golden_core::{
    edit::Edit,
    engine::EngineEditError,
    events::EventKind,
    parameter::{ParamValue, ParameterEventBehaviour},
};
mod fixture;
use fixture::*;

#[test]
fn completed_output_report_survives_unrelated_queued_input_retirement() {
    let mut fixture = Fixture::new();
    fixture.completed_output();
    let mut ctx = fixture.queued_combined_turn();
    fixture.target().stop_input_backend();
    fixture.engine.absorb_edits(&mut ctx).unwrap();
    assert!(matches!(
        fixture.engine.apply_edits(),
        Err(EngineEditError::EditPreconditionFailed { .. })
    ));
    let event = engine::path(&fixture.engine, fixture.id, "values/info/last_event");
    assert!(engine::value(&fixture.engine, event)
        .as_str()
        .unwrap()
        .contains("a key"));
    assert_eq!(fixture.output.calls.lock().unwrap().len(), 1);
    fixture.finish();
}

#[test]
fn output_retirement_rejects_only_final_readiness_after_input_values_and_callbacks_apply() {
    let mut fixture = Fixture::new();
    fixture.completed_output();
    let output_active = engine::path(&fixture.engine, fixture.id, "values/info/output_active");
    fixture.engine.edits.push(Edit::SetParam {
        node: output_active,
        value: ParamValue::Bool(false),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    fixture.engine.apply_edits().unwrap();
    let mut ctx = fixture.queued_combined_turn();
    fixture.target().stop_output_backend();
    fixture.engine.absorb_edits(&mut ctx).unwrap();
    assert!(matches!(
        fixture.engine.apply_edits(),
        Err(EngineEditError::EditPreconditionFailed { .. })
    ));
    let last_key = engine::path(&fixture.engine, fixture.id, "values/info/last_key");
    assert_eq!(engine::value(&fixture.engine, last_key), ParamValue::Str("b".into()));
    assert_eq!(engine::value(&fixture.engine, output_active), ParamValue::Bool(false));
    assert!(fixture.engine.inbox.events.iter().any(
        |event| matches!(&event.kind, EventKind::Custom(event) if event.payload["callback"] == "keyboardKeyChanged")
    ));
    fixture.finish();
}

#[test]
fn combined_healthy_input_output_is_idle_without_actor_polling_or_snapshots() {
    let mut fixture = Fixture::new();
    for _ in 0..100 {
        fixture.tick();
        let stats = fixture.engine.tick_stats();
        assert_eq!(stats.nodes_due, 0);
        assert_eq!(stats.callbacks_fired, 0);
        assert_eq!(stats.wake_callbacks_fired, 0);
        assert_eq!(stats.snapshot_builds, 0);
    }
    assert_eq!(fixture.input.opens.load(Ordering::Acquire), 1);
    assert_eq!(fixture.output_opens.load(Ordering::Acquire), 1);
    fixture.finish();
}

#[test]
fn combined_native_failures_recover_both_services_without_replay_or_periodic_updates() {
    let mut fixture = Fixture::new();
    fixture.input.panic.store(true, Ordering::Release);
    fixture.output.panic.store(true, Ordering::Release);
    engine::invoke(
        &mut fixture.engine,
        fixture.id,
        "tapKey",
        vec![ParamValue::Str("a".into())],
    );
    until(|| {
        fixture.tick();
        assert_eq!(fixture.engine.tick_stats().callbacks_fired, 0);
        fixture.input.opens.load(Ordering::Acquire) == 2
            && fixture.output_opens.load(Ordering::Acquire) == 2
            && fixture.target().input.catalog.is_some()
            && fixture.target().output.is_ready()
    });
    assert_eq!(fixture.output.calls.lock().unwrap().len(), 1);
    fixture.quiet();
    engine::invoke(
        &mut fixture.engine,
        fixture.id,
        "tapKey",
        vec![ParamValue::Str("b".into())],
    );
    until(|| {
        fixture.tick();
        fixture.output.calls.lock().unwrap().len() == 2 && fixture.target().output.receipts.is_empty()
    });
    assert_eq!(fixture.output.calls.lock().unwrap().len(), 2);
    fixture.finish();
}

#[test]
fn continuous_input_progresses_while_sixty_four_output_sources_wait_for_a_blocked_effect() {
    let mut fixture = Fixture::new();
    let (release, gate) = mpsc::channel();
    *fixture.output.gate.lock().unwrap() = Some(gate);
    for _ in 0..64 {
        engine::invoke(
            &mut fixture.engine,
            fixture.id,
            "tapKey",
            vec![ParamValue::Str("a".into())],
        );
    }
    until(|| {
        fixture.tick();
        fixture.output.calls.lock().unwrap().len() == 1
    });
    assert_eq!(fixture.target().output.admission.metrics().active, 64);
    fixture
        .input
        .records
        .lock()
        .unwrap()
        .extend((0..256).map(|index| key(index % 2 == 0)));
    let mut seen = std::collections::HashSet::new();
    let mut input = Vec::new();
    until(|| {
        fixture.tick();
        for event in fixture.engine.ui_events_since(None) {
            if let EventKind::Custom(event) = event.kind {
                if event.payload["callback"] == "keyboardKeyChanged"
                    && seen.insert(Arc::as_ptr(&event.payload) as usize)
                {
                    input.push(event.payload["args"][1].as_bool().unwrap());
                }
            }
        }
        assert_eq!(fixture.engine.tick_stats().callbacks_fired, 0);
        input.len() == 256
    });
    assert_eq!(input, (0..256).map(|index| index % 2 == 0).collect::<Vec<_>>());
    assert_eq!(fixture.output.calls.lock().unwrap().len(), 1);
    assert_eq!(fixture.target().output.admission.metrics().active, 64);
    release.send(()).unwrap();
    until(|| {
        fixture.tick();
        fixture.output.calls.lock().unwrap().len() == 64 && fixture.target().output.receipts.is_empty()
    });
    fixture.quiet();
    until(|| fixture.target().output.admission.metrics().active == 0);
    fixture.finish();
}
