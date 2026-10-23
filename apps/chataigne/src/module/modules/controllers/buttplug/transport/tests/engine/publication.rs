use super::*;
use golden_core::{engine::EngineEditError, node::NodeMetaPatch};

fn device(index: u32) -> ButtplugDeviceInfo {
    ButtplugDeviceInfo {
        index,
        variant_id: format!("device:{index}"),
        name: format!("Fixture {index}"),
        display_name: None,
        outputs: vec!["vibrate".into()],
    }
}
fn take(engine: &mut AppEngine, id: NodeId) -> RetainedPayload<Publication> {
    let mut result = None;
    until(|| {
        let mut events = Vec::new();
        module(engine, id).transport.as_ref().unwrap().drain_events(&mut events);
        result = events.pop();
        result.is_some()
    });
    result.unwrap()
}
fn callbacks(engine: &AppEngine) -> Vec<u64> {
    engine
        .inbox
        .events
        .iter()
        .filter_map(|event| match &event.kind {
            golden_core::events::EventKind::Custom(event)
                if event.payload.get("callback").and_then(serde_json::Value::as_str) == Some("buttplugDeviceAdded") =>
            {
                event.payload.get("args")?.as_array()?.first()?.get("index")?.as_u64()
            }
            _ => None,
        })
        .collect()
}

#[test]
fn queued_publication_retains_original_quota_and_retirement_rejects_all_values_and_callbacks() {
    let (mut engine, id) = create();
    let slots = slots();
    let input = PayloadPool::new("buttplug-one-publication", 1, 4 * EVENT_WEIGHT).unwrap();
    let control = Control::default();
    install(
        &mut engine,
        id,
        Fake {
            control: control.clone(),
            gate: None,
            cleanup: None,
        },
        &slots,
        input.clone(),
    );
    quiet(&mut engine, id);
    assert!(input.wait_for_idle(Duration::from_secs(1)));
    let last = path(&engine, id, "values/info/last_event");
    let before = value(&engine, last);
    control
        .input
        .lock()
        .unwrap()
        .push_back(ButtplugWorkerEvent::DeviceAdded(device(7)));
    let event = take(&mut engine, id);
    let mut ctx = context(&engine);
    module(&mut engine, id).project_event(&mut ctx, event);
    engine.absorb_edits(&mut ctx).unwrap();
    assert!(input.try_reserve(EVENT_WEIGHT).is_err());
    module(&mut engine, id).stop_transport();
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
    assert!(input.try_reserve(EVENT_WEIGHT).is_err());
    assert!(matches!(
        engine.apply_edits(),
        Err(EngineEditError::EditPreconditionFailed { .. })
    ));
    assert_eq!(value(&engine, last), before);
    assert!(callbacks(&engine).is_empty());
    assert!(input.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn snapshot_free_service_keeps_prepared_native_input_for_a_later_notified_turn() {
    let (mut engine, id) = create();
    let slots = slots();
    let control = Control::default();
    attach(&mut engine, id, control.clone(), &slots, None);
    quiet(&mut engine, id);
    control
        .input
        .lock()
        .unwrap()
        .push_back(ButtplugWorkerEvent::DeviceAdded(device(11)));
    until(|| module(&mut engine, id).transport.as_ref().unwrap().queued_events() == 1);
    let mut missing = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    module(&mut engine, id).service_io(&mut missing);
    assert!(missing.edits.pending.is_empty());
    assert_eq!(module(&mut engine, id).transport.as_ref().unwrap().queued_events(), 1);
    let mut ctx = context(&engine);
    module(&mut engine, id).service_io(&mut ctx);
    flush(&mut engine, &mut ctx);
    assert_eq!(callbacks(&engine), vec![11]);
    finish(&mut engine, id, &slots);
}

#[test]
fn queued_publication_checks_target_structure_before_any_write_or_callback() {
    let (mut engine, id) = create();
    let slots = slots();
    let control = Control::default();
    attach(&mut engine, id, control.clone(), &slots, None);
    quiet(&mut engine, id);
    control
        .input
        .lock()
        .unwrap()
        .push_back(ButtplugWorkerEvent::DeviceAdded(device(3)));
    let event = take(&mut engine, id);
    let mut ctx = context(&engine);
    module(&mut engine, id).project_event(&mut ctx, event);
    let last = path(&engine, id, "values/info/last_event");
    engine.edits.push(Edit::PatchMeta {
        node: last,
        patch: NodeMetaPatch {
            short_name: Some("moved_last_event".into()),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    engine.absorb_edits(&mut ctx).unwrap();
    assert!(matches!(
        engine.apply_edits(),
        Err(EngineEditError::EditPreconditionFailed { .. })
    ));
    assert!(callbacks(&engine).is_empty());
    finish(&mut engine, id, &slots);
}

#[test]
fn repeated_callback_occurrences_are_prepared_on_worker_and_queued_without_coalescing() {
    let (mut engine, id) = create();
    let slots = slots();
    let control = Control::default();
    attach(&mut engine, id, control.clone(), &slots, None);
    quiet(&mut engine, id);
    for _ in 0..2 {
        control
            .input
            .lock()
            .unwrap()
            .push_back(ButtplugWorkerEvent::DeviceAdded(device(9)));
    }
    let mut ctx = context(&engine);
    for _ in 0..2 {
        let event = take(&mut engine, id);
        assert!(event.callback.is_some());
        module(&mut engine, id).project_event(&mut ctx, event);
    }
    flush(&mut engine, &mut ctx);
    assert_eq!(callbacks(&engine), vec![9, 9]);
    finish(&mut engine, id, &slots);
}

#[test]
fn continuous_native_input_keeps_all_512_occurrences_and_bounded_actor_turns() {
    let (mut engine, id) = create();
    let slots = slots();
    let control = Control::default();
    attach(&mut engine, id, control.clone(), &slots, None);
    quiet(&mut engine, id);
    invoke(&mut engine, id, "stopAll", vec![]);
    for _ in 0..2 {
        engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
        engine.apply_edits().unwrap();
    }
    assert_eq!(
        module(&mut engine, id).service.outgoing.len(),
        1,
        "the command must reach the captured-output inbox before native input starts"
    );
    control
        .input
        .lock()
        .unwrap()
        .extend((0..512).map(|index| ButtplugWorkerEvent::DeviceAdded(device(index))));
    let max = path(&engine, id, "parameters/max_output");
    let mut rounds = 0;
    until(|| {
        let before = callbacks(&engine).len();
        let mut ctx = context(&engine);
        module(&mut engine, id).service_io(&mut ctx);
        flush(&mut engine, &mut ctx);
        let count = callbacks(&engine).len();
        assert!(count - before <= 16);
        if rounds % 16 == 0 {
            set(&mut engine, max, ParamValue::Float(0.5));
        }
        rounds += 1;
        count == 512
    });
    assert_eq!(callbacks(&engine), (0..512).collect::<Vec<_>>());
    assert_eq!(control.actions.lock().unwrap().len(), 1);
    finish(&mut engine, id, &slots);
}

#[test]
fn input_lifetime_pressure_pauses_native_consumption_without_reconnecting_and_recovers() {
    let (mut engine, id) = create();
    let slots = slots();
    let input = PayloadPool::new("buttplug-two-publications", 2, 4 * EVENT_WEIGHT).unwrap();
    let control = Control::default();
    install(
        &mut engine,
        id,
        Fake {
            control: control.clone(),
            gate: None,
            cleanup: None,
        },
        &slots,
        input,
    );
    quiet(&mut engine, id);
    control
        .input
        .lock()
        .unwrap()
        .extend((0..3).map(|index| ButtplugWorkerEvent::DeviceAdded(device(index))));
    until(|| module(&mut engine, id).transport.as_ref().unwrap().pressure());
    let polls = control.polls.load(Ordering::Acquire);
    let shared = module(&mut engine, id).transport.as_ref().unwrap().shared.clone();
    let max = path(&engine, id, "parameters/max_output");
    for _ in 0..30 {
        set(&mut engine, max, ParamValue::Float(0.5));
    }
    thread::sleep(Duration::from_millis(20));
    assert_eq!(control.polls.load(Ordering::Acquire), polls);
    assert_eq!(slots.metrics().active, 1);
    until(|| {
        let mut ctx = context(&engine);
        module(&mut engine, id).service_io(&mut ctx);
        flush(&mut engine, &mut ctx);
        callbacks(&engine).len() == 3
    });
    assert!(Arc::ptr_eq(
        &shared,
        &module(&mut engine, id).transport.as_ref().unwrap().shared
    ));
    assert_eq!(callbacks(&engine), vec![0, 1, 2]);
    finish(&mut engine, id, &slots);
}

#[test]
fn frozen_catalogs_keep_parameter_identity_missing_selection_and_later_authored_values() {
    let (mut engine, id) = create();
    let slots = slots();
    let control = Control::default();
    attach(&mut engine, id, control.clone(), &slots, None);
    quiet(&mut engine, id);
    let selected = path(&engine, id, "connection/device");
    let uuid = engine.process_tree_snapshot().node(selected).unwrap().uuid;
    control
        .input
        .lock()
        .unwrap()
        .push_back(ButtplugWorkerEvent::Devices(vec![device(7), device(8)]));
    let event = take(&mut engine, id);
    let domain = event.catalog.as_ref().unwrap().options.clone();
    let mut ctx = context(&engine);
    module(&mut engine, id).project_event(&mut ctx, event);
    flush(&mut engine, &mut ctx);
    assert!(engine
        .nodes
        .get(selected)
        .unwrap()
        .engine_param_snapshot()
        .unwrap()
        .constraints
        .enum_options
        .same_storage(&domain));
    set(&mut engine, selected, ParamValue::Enum("device:7".into()));
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine.apply_edits().unwrap();
    quiet(&mut engine, id);
    control
        .input
        .lock()
        .unwrap()
        .push_back(ButtplugWorkerEvent::Devices(Vec::new()));
    quiet(&mut engine, id);
    assert_eq!(value(&engine, selected), ParamValue::Enum("device:7".into()));
    let options = engine
        .nodes
        .get(selected)
        .unwrap()
        .engine_param_snapshot()
        .unwrap()
        .constraints
        .enum_options;
    assert!(options
        .iter()
        .any(|option| option.variant_id == "device:7" && option.tags.iter().any(|tag| tag == "missing")));
    control
        .input
        .lock()
        .unwrap()
        .push_back(ButtplugWorkerEvent::Devices(vec![device(8)]));
    let event = take(&mut engine, id);
    let mut ctx = context(&engine);
    module(&mut engine, id).project_event(&mut ctx, event);
    set(&mut engine, selected, ParamValue::Enum("none".into()));
    flush(&mut engine, &mut ctx);
    assert_eq!(value(&engine, selected), ParamValue::Enum("none".into()));
    assert_eq!(engine.process_tree_snapshot().node(selected).unwrap().uuid, uuid);
    finish(&mut engine, id, &slots);
}
