use super::super::*;
use super::engine::*;
use crate::app::{module::script_api::NativeCallbackPreparer, AppEngine};
use golden_core::{
    edit::Edit,
    engine::EngineEditError,
    events::EventKind,
    node::{NodeId, NodeMetaPatch},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::ExecutionPhase,
    script::ScriptMethodCall,
};
use serde_json::Value;
use std::{sync::mpsc, thread};

fn pool(items: usize) -> PayloadPool {
    PayloadPool::new("joycon-qualified-input", items, items * EVENT_WEIGHT).unwrap()
}
fn available_items(pool: &PayloadPool) -> usize {
    let mut reservations = Vec::new();
    while let Ok(reservation) = pool.try_reserve(0) {
        reservations.push(reservation);
        assert!(reservations.len() <= 128);
    }
    reservations.len()
}
fn callbacks(engine: &AppEngine, name: &str) -> Vec<Value> {
    engine
        .inbox
        .events
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::Custom(event) if event.payload["callback"] == name => Some(event.payload["args"].clone()),
            _ => None,
        })
        .collect()
}
fn set(engine: &mut AppEngine, node: NodeId, value: ParamValue) {
    engine.edits.push(Edit::SetParam {
        node,
        value,
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().unwrap();
}
fn invoke(engine: &mut AppEngine, id: NodeId) {
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: id,
        call: ScriptMethodCall::prepare("vibrate", vec![]).unwrap(),
    });
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine.apply_edits().unwrap();
}
fn rejected(engine: &mut AppEngine, ctx: &mut golden_core::process_ctx::ProcessCtx) {
    engine.absorb_edits(ctx).unwrap();
    assert!(matches!(
        engine.apply_edits(),
        Err(EngineEditError::EditPreconditionFailed { .. })
    ));
}
fn config(engine: &mut AppEngine, id: NodeId) {
    sync_handles(engine, id);
    let mut ctx = context(engine);
    module(engine, id).configure_projection(&mut ctx);
    flush(engine, &mut ctx);
}

#[test]
fn callback_pressure_pauses_reads_atomically_while_commands_and_maintenance_continue_then_recovers() {
    let callbacks = PayloadPool::new("joycon-qualified-callbacks", 128, 8 * 1024 * 1024).unwrap();
    let held = callbacks.try_reserve(8 * 1024 * 1024).unwrap();
    let control = Control::default();
    let (mut engine, id, slots, input) = create_prepared(
        control.clone(),
        Some(NativeCallbackPreparer::using(callbacks.clone())),
        pool(64),
    );
    let shared = module(&mut engine, id).runtime.as_ref().unwrap().shared.clone();
    input.lock().unwrap().push_back(state(true, 0.5));
    until(|| shared.pressure.load(Ordering::Acquire));
    let polls = control.polls.load(Ordering::Acquire);
    let maintenance = control.maintenance.load(Ordering::Acquire);
    service(&mut engine, id);
    invoke(&mut engine, id);
    service(&mut engine, id);
    until(|| control.actions.lock().unwrap().len() == 1);
    until(|| control.maintenance.load(Ordering::Acquire) > maintenance + 2);
    assert_eq!(control.polls.load(Ordering::Acquire), polls);
    assert_eq!(
        available_items(&callbacks),
        127,
        "no partial candidate registrations while blocked"
    );
    assert!(buttons(&engine).is_empty());
    drop(held);
    until(|| queued(&mut engine, id) == 1);
    service(&mut engine, id);
    assert_eq!(buttons(&engine), [true]);
    until(|| !shared.pressure.load(Ordering::Acquire));
    service(&mut engine, id);
    assert!(Arc::ptr_eq(
        &shared,
        &module(&mut engine, id).runtime.as_ref().unwrap().shared
    ));
    assert_eq!(control.actions.lock().unwrap().len(), 1);
    finish(&mut engine, id, &slots);
}

#[test]
fn cancellation_during_callback_pressure_releases_native_slot_without_replaying_or_waiting_for_capacity() {
    let callbacks = PayloadPool::new("joycon-blocked-callbacks", 128, 8 * 1024 * 1024).unwrap();
    let held = callbacks.try_reserve(8 * 1024 * 1024).unwrap();
    let control = Control::default();
    let events = pool(1);
    let (mut engine, id, slots, input) = create_prepared(
        control.clone(),
        Some(NativeCallbackPreparer::using(callbacks.clone())),
        events.clone(),
    );
    input.lock().unwrap().push_back(state(true, 0.0));
    until(|| module(&mut engine, id).runtime.as_ref().unwrap().pressure());
    let start = Instant::now();
    module(&mut engine, id).stop_runtime();
    assert!(start.elapsed() < Duration::from_millis(100));
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
    assert!(events.wait_for_idle(Duration::from_secs(1)));
    assert!(control.actions.lock().unwrap().is_empty());
    assert_eq!(available_items(&callbacks), 127);
    drop(held);
    assert!(callbacks.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn both_sides_full_vocabulary_preserves_callback_arguments_and_raw_copies_keep_canonical_quota() {
    let callbacks_pool = PayloadPool::new("joycon-full-vocabulary", 128, 8 * 1024 * 1024).unwrap();
    let (mut engine, id, slots, input) = create_prepared(
        Control::default(),
        Some(NativeCallbackPreparer::using(callbacks_pool.clone())),
        pool(4),
    );
    let mode = path(&engine, id, "connection/motion_data_mode");
    set(&mut engine, mode, ParamValue::Enum("all".into()));
    config(&mut engine, id);
    let mut raw = JoyConRuntimeState::disconnected();
    for side in JoyConSide::ALL {
        let row = raw.side_mut(side);
        row.connected = true;
        row.stick_x = 0.5;
        row.stick_y = -0.5;
        row.orientation_pitch = 10.0;
        row.orientation_roll = 20.0;
        row.accelerometer = (1.0, 2.0, 3.0);
        row.gyroscope = (4.0, 5.0, 6.0);
    }
    raw.left.left_buttons = vec![
        Buttons::Down,
        Buttons::Up,
        Buttons::Right,
        Buttons::Left,
        Buttons::SL,
        Buttons::SR,
        Buttons::L,
        Buttons::ZL,
    ];
    raw.left.shared_buttons = vec![Buttons::Minus, Buttons::LStick, Buttons::Capture];
    raw.right.right_buttons = vec![
        Buttons::Y,
        Buttons::X,
        Buttons::B,
        Buttons::A,
        Buttons::SR,
        Buttons::SL,
        Buttons::R,
        Buttons::ZR,
    ];
    raw.right.shared_buttons = vec![Buttons::Plus, Buttons::RStick, Buttons::Home];
    input.lock().unwrap().push_back(JoyConWorkerEvent::State(Box::new(raw)));
    until(|| queued(&mut engine, id) == 1);
    service(&mut engine, id);
    assert_eq!(callbacks(&engine, "joyConConnected").len(), 2);
    let changed = callbacks(&engine, "joyConButtonChanged");
    assert_eq!(changed.len(), 22);
    assert!(changed
        .iter()
        .all(|args| args[2] == true && args[3]["buttons"][args[1].as_str().unwrap()] == true));
    assert_eq!(callbacks(&engine, "joyConButtonPressed").len(), 22);
    let sticks = callbacks(&engine, "joyConStickChanged");
    assert_eq!(sticks.len(), 2);
    assert!(sticks.iter().all(
        |args| args[1] == serde_json::json!({"x":(0.5-0.1)/0.9,"y":-(0.5-0.1)/0.9}) && args[2]["stick"] == args[1]
    ));
    let motion = callbacks(&engine, "joyConMotionChanged");
    assert_eq!(motion.len(), 2);
    assert!(motion.iter().all(|args| args[1] == args[2]["motion"]));
    let copies: Vec<_> = engine
        .inbox
        .events
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::Custom(event)
                if event.payload["callback"]
                    .as_str()
                    .is_some_and(|name| name.starts_with("joyCon")) =>
            {
                Some(event.payload.clone())
            }
            _ => None,
        })
        .collect();
    assert_eq!(copies.len(), 50);
    finish(&mut engine, id, &slots);
    drop(engine);
    assert_eq!(available_items(&callbacks_pool), 78);
    drop(copies);
    assert!(callbacks_pool.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn queued_leaf_and_ancestor_rename_reject_before_any_value_or_callback() {
    for target in [
        "values/left_controller/left_buttons/left_dpad_down",
        "values/left_controller/left_buttons",
        "values/left_controller",
        "values",
    ] {
        let (mut engine, id, slots, input) = create_controlled(Control::default());
        let connected = path(&engine, id, "values/info/left_controller_connected");
        let target = path(&engine, id, target);
        input.lock().unwrap().push_back(state(true, 0.5));
        until(|| queued(&mut engine, id) == 1);
        let mut ctx = context(&engine);
        module(&mut engine, id).service_io(&mut ctx);
        engine.edits.push(Edit::PatchMeta {
            node: target,
            patch: NodeMetaPatch {
                short_name: Some("renamed_target".into()),
                ..Default::default()
            },
        });
        engine.apply_edits().unwrap();
        rejected(&mut engine, &mut ctx);
        assert_eq!(value(&engine, connected), ParamValue::Bool(false));
        assert!(buttons(&engine).is_empty());
        finish(&mut engine, id, &slots);
    }
}

#[test]
fn queued_ancestor_reparent_rejects_before_values_or_callbacks() {
    let (mut engine, id, slots, input) = create_controlled(Control::default());
    let connected = path(&engine, id, "values/info/left_controller_connected");
    let target = path(&engine, id, "values/left_controller");
    let destination = path(&engine, id, "parameters");
    input.lock().unwrap().push_back(state(true, 0.0));
    until(|| queued(&mut engine, id) == 1);
    let mut ctx = context(&engine);
    module(&mut engine, id).service_io(&mut ctx);
    engine.edits.push(Edit::MoveNode {
        node: target,
        new_parent: destination,
        new_prev_sibling: None,
    });
    engine.apply_edits().unwrap();
    rejected(&mut engine, &mut ctx);
    assert_eq!(value(&engine, connected), ParamValue::Bool(false));
    assert!(buttons(&engine).is_empty());
    finish(&mut engine, id, &slots);
}

#[test]
fn queued_projection_keeps_original_input_quota_until_rejected_edits_are_disposed() {
    let events = pool(1);
    let (mut engine, id, slots, input) = create_prepared(Control::default(), None, events.clone());
    input.lock().unwrap().push_back(state(true, 0.0));
    until(|| queued(&mut engine, id) == 1);
    let mut ctx = context(&engine);
    module(&mut engine, id).service_io(&mut ctx);
    engine.absorb_edits(&mut ctx).unwrap();
    assert!(events.try_reserve(EVENT_WEIGHT).is_err());
    finish(&mut engine, id, &slots);
    assert!(events.try_reserve(EVENT_WEIGHT).is_err());
    assert!(matches!(
        engine.apply_edits(),
        Err(EngineEditError::EditPreconditionFailed { .. })
    ));
    assert!(buttons(&engine).is_empty());
    assert!(events.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn authored_settings_changed_after_projection_capture_reject_stale_callbacks_without_native_reconfiguration() {
    for (parameter, next) in [
        ("parameters/stick_processing/stick_dead_zone", ParamValue::Float(0.8)),
        ("connection/motion_data_mode", ParamValue::Enum("all".into())),
    ] {
        let (mut engine, id, slots, input) = create_controlled(Control::default());
        let connected = path(&engine, id, "values/info/left_controller_connected");
        input.lock().unwrap().push_back(state(true, 0.5));
        until(|| queued(&mut engine, id) == 1);
        let mut ctx = context(&engine);
        module(&mut engine, id).service_io(&mut ctx);
        let settings = path(&engine, id, parameter);
        set(&mut engine, settings, next);
        rejected(&mut engine, &mut ctx);
        assert_eq!(value(&engine, connected), ParamValue::Bool(false));
        assert!(buttons(&engine).is_empty());
        finish(&mut engine, id, &slots);
    }
}

#[test]
fn analog_coalescing_selects_against_last_applied_state_without_losing_other_side_changes() {
    let (mut engine, id, slots, input) = create_controlled(Control::default());
    let dead_zone = path(&engine, id, "parameters/stick_processing/stick_dead_zone");
    set(&mut engine, dead_zone, ParamValue::Float(0.0));
    config(&mut engine, id);
    let cap = path(&engine, id, "connection/processing_fps_cap");
    set(&mut engine, cap, ParamValue::Int(1));
    input.lock().unwrap().push_back(state(false, 0.0));
    until(|| queued(&mut engine, id) == 1);
    service(&mut engine, id);
    input.lock().unwrap().extend([state(false, 0.5), state(false, 0.0)]);
    until(|| queued(&mut engine, id) == 2);
    service(&mut engine, id);
    assert!(module(&mut engine, id).pending_state.is_some());
    module(&mut engine, id).processing_elapsed = Duration::from_secs(1);
    service(&mut engine, id);
    assert!(callbacks(&engine, "joyConStickChanged").is_empty());
    let mut raw = JoyConRuntimeState::disconnected();
    raw.left.connected = true;
    raw.left.stick_x = 0.5;
    raw.right.connected = true;
    input
        .lock()
        .unwrap()
        .push_back(JoyConWorkerEvent::State(Box::new(raw.clone())));
    until(|| queued(&mut engine, id) == 1);
    service(&mut engine, id);
    raw.left.stick_x = 0.8;
    input
        .lock()
        .unwrap()
        .push_back(JoyConWorkerEvent::State(Box::new(raw.clone())));
    raw.right.stick_y = 0.5;
    input.lock().unwrap().push_back(JoyConWorkerEvent::State(Box::new(raw)));
    until(|| queued(&mut engine, id) == 2);
    service(&mut engine, id);
    module(&mut engine, id).processing_elapsed = Duration::from_secs(1);
    service(&mut engine, id);
    let sticks = callbacks(&engine, "joyConStickChanged");
    assert_eq!(
        sticks
            .iter()
            .map(|args| (args[0].as_str().unwrap(), args[1].clone()))
            .collect::<Vec<_>>(),
        [
            ("left", serde_json::json!({"x":0.5,"y":0.0})),
            ("left", serde_json::json!({"x":0.8,"y":0.0})),
            ("right", serde_json::json!({"x":0.0,"y":0.5}))
        ]
    );
    finish(&mut engine, id, &slots);
}

#[test]
fn eight_projection_reconfigurations_cancel_old_publications_and_reuse_one_native_lifetime() {
    let control = Control::default();
    let (mut engine, id, slots, input) = create_controlled(control.clone());
    let shared = module(&mut engine, id).runtime.as_ref().unwrap().shared.clone();
    let dead_zone = path(&engine, id, "parameters/stick_processing/stick_dead_zone");
    let mode = path(&engine, id, "connection/motion_data_mode");
    let stick = path(&engine, id, "values/left_controller/left_stick");
    input.lock().unwrap().push_back(state(true, 0.5));
    until(|| queued(&mut engine, id) == 1);
    service(&mut engine, id);
    let mut old = Vec::new();
    for round in 0..8 {
        input.lock().unwrap().push_back(state(round % 2 == 0, 0.5));
        until(|| queued(&mut engine, id) >= 1);
        let publication = module(&mut engine, id).runtime.as_ref().unwrap().try_recv().unwrap();
        old.extend(publication.guards.clone());
        set(
            &mut engine,
            dead_zone,
            ParamValue::Float(if round % 2 == 0 { 0.8 } else { 0.1 }),
        );
        set(
            &mut engine,
            mode,
            ParamValue::Enum(if round % 2 == 0 { "all" } else { "none" }.into()),
        );
        config(&mut engine, id);
        assert!(publication.guards.iter().any(|guard| !guard.is_current()));
        drop(publication);
        until(|| queued(&mut engine, id) >= 1);
        // A previous configuration can still occupy the mailbox; service cancels it explicitly.
        until(|| {
            module(&mut engine, id).processing_elapsed = Duration::from_secs(1);
            service(&mut engine, id);
            value(&engine, stick) == ParamValue::Vec2(if round % 2 == 0 { 0.0 } else { (0.5 - 0.1) / 0.9 }, 0.0)
                && queued(&mut engine, id) == 0
        });
        invoke(&mut engine, id);
        service(&mut engine, id);
        until(|| control.actions.lock().unwrap().len() == round + 1);
        assert!(Arc::ptr_eq(
            &shared,
            &module(&mut engine, id).runtime.as_ref().unwrap().shared
        ));
        assert_eq!(slots.metrics().active, 1);
    }
    assert!(old.iter().any(|guard| !guard.is_current()));
    assert_eq!(slots.metrics().peak, 1);
    assert_eq!(control.actions.lock().unwrap().len(), 8);
    finish(&mut engine, id, &slots);
}

#[test]
fn native_panic_schedules_recovery_before_blocked_cleanup_and_fresh_provider_recovers() {
    let control = Control::default();
    let (release, cleanup) = mpsc::channel();
    *control.cleanup.lock().unwrap() = Some(cleanup);
    let (mut engine, id, slots, _) = create_controlled(control.clone());
    control.panic_poll.store(true, Ordering::Release);
    until(|| {
        service(&mut engine, id);
        module(&mut engine, id).service.retry_at.is_some()
    });
    assert!(module(&mut engine, id).runtime.is_none());
    assert_eq!(slots.metrics().active, 1);
    assert_eq!(module(&mut engine, id).service.owner.metrics().active, 1);
    let retry = module(&mut engine, id).service.retry_at;
    let dead_zone = path(&engine, id, "parameters/stick_processing/stick_dead_zone");
    let start = Instant::now();
    for _ in 0..30 {
        set(&mut engine, dead_zone, ParamValue::Float(0.2));
        service(&mut engine, id);
    }
    assert!(start.elapsed() < Duration::from_millis(500));
    assert_eq!(module(&mut engine, id).service.retry_at, retry);
    release.send(()).unwrap();
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
    let fresh = Control::default();
    let input = fresh.input.clone();
    install(&mut engine, id, fresh.clone(), &slots, None, pool(64));
    config(&mut engine, id);
    input.lock().unwrap().push_back(state(true, 0.5));
    until(|| queued(&mut engine, id) == 1);
    service(&mut engine, id);
    let connected = path(&engine, id, "values/info/left_controller_connected");
    assert_eq!(value(&engine, connected), ParamValue::Bool(true));
    invoke(&mut engine, id);
    service(&mut engine, id);
    until(|| fresh.actions.lock().unwrap().len() == 1);
    finish(&mut engine, id, &slots);
}

#[test]
fn continuous_512_native_edges_keep_order_and_two_record_turns_while_output_and_unrelated_edits_continue() {
    let control = Control::default();
    let (mut engine, id, slots, input) = create_controlled(control.clone());
    input.lock().unwrap().push_back(state(false, 0.0));
    until(|| queued(&mut engine, id) == 1);
    service(&mut engine, id);
    invoke(&mut engine, id);
    input
        .lock()
        .unwrap()
        .extend((0..512).map(|index| state(index % 2 == 0, 0.0)));
    let cap = path(&engine, id, "connection/processing_fps_cap");
    let mut rounds = 0;
    until(|| {
        let before = buttons(&engine).len();
        service(&mut engine, id);
        let count = buttons(&engine).len();
        assert!(count - before <= 2);
        if rounds % 16 == 0 {
            set(&mut engine, cap, ParamValue::Int(60));
        }
        rounds += 1;
        if count != 512 {
            thread::sleep(Duration::from_millis(1));
        }
        count == 512
    });
    assert_eq!(
        buttons(&engine),
        (0..512).map(|index| index % 2 == 0).collect::<Vec<_>>()
    );
    until(|| control.actions.lock().unwrap().len() == 1);
    assert!(rounds >= 256);
    finish(&mut engine, id, &slots);
}
