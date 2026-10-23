use super::super::*;
use super::engine::*;
use crate::app::module_command::{
    CapturedInvocation, ModuleCommandDeliveryPolicy, ModuleCommandExecuteEvent, ModuleCommandParamOverride,
    MODULE_COMMAND_EXECUTE_TOPIC,
};
use crate::app::{AppEngine, JoyConVibrateCommand};
use golden_core::{
    edit::Edit,
    events::CustomEvent,
    node::{Node, NodeId, NodeMetaPatch},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::{ExecutionPhase, ProcessCtx},
    script::ScriptMethodCall,
};
use std::{sync::mpsc, thread};

fn tick(engine: &mut AppEngine) {
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine.apply_edits().unwrap();
    engine.run_tick(Duration::from_millis(1)).unwrap();
    engine.apply_edits().unwrap();
}
fn quiet(engine: &mut AppEngine, id: NodeId) {
    let mut silent = 0;
    until(|| {
        tick(engine);
        let module = module(engine, id);
        if module.service.outgoing.is_empty()
            && module.service.pending.is_empty()
            && engine.tick_stats().wake_callbacks_fired == 0
        {
            silent += 1;
        } else {
            silent = 0;
        }
        silent >= 20
    });
}
fn invoke(engine: &mut AppEngine, id: NodeId, method: &str, args: Vec<ParamValue>) {
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: id,
        call: ScriptMethodCall::prepare(method, args).unwrap(),
    });
    engine.apply_edits().unwrap();
}
fn set(engine: &mut AppEngine, id: NodeId, value: ParamValue) {
    engine.edits.push(Edit::SetParam {
        node: id,
        value,
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().unwrap();
}
fn command(engine: &mut AppEngine, id: NodeId) -> NodeId {
    let tester = path(engine, id, "command_tester");
    engine.add_user_item(JoyConVibrateCommand::create().into(), Some(tester));
    for _ in 0..8 {
        engine.apply_edits().unwrap();
    }
    engine.nodes.get(tester).unwrap().node_data().first_child.unwrap()
}
fn execution(engine: &AppEngine, command: NodeId, target: &str) -> ModuleCommandExecuteEvent {
    ModuleCommandExecuteEvent {
        command_id: command,
        invocation_id: None,
        delivery_policy: ModuleCommandDeliveryPolicy::Standard,
        param_overrides: vec![
            ModuleCommandParamOverride {
                param_id: path(engine, command, "target"),
                value: ParamValue::Enum("discarded".into()),
            },
            ModuleCommandParamOverride {
                param_id: path(engine, command, "target"),
                value: ParamValue::Enum(target.into()),
            },
        ],
    }
}
fn emit(engine: &mut AppEngine, event: ModuleCommandExecuteEvent) {
    engine.edits.push(Edit::EmitCustomEvent {
        event: CustomEvent::new(
            MODULE_COMMAND_EXECUTE_TOPIC,
            Some(event.command_id),
            serde_json::to_value(event).unwrap(),
        ),
    });
    engine.apply_edits().unwrap();
}
fn output(control: &Control) -> Vec<(String, f32)> {
    control
        .actions
        .lock()
        .unwrap()
        .iter()
        .filter_map(|command| match command {
            JoyConWorkerCommand::Vibrate(request) => Some((request.target.clone(), request.amplitude)),
            _ => None,
        })
        .collect()
}

fn mixed_order(manual_dispatch: bool) {
    let control = Control::default();
    let (mut engine, id, slots, _) = create_controlled(control.clone());
    let cmd = command(&mut engine, id);
    quiet(&mut engine, id);
    let amplitude = path(&engine, cmd, "amplitude");
    set(&mut engine, amplitude, ParamValue::Float(0.3));
    let mut event = execution(&engine, cmd, "left");
    event.param_overrides.push(ModuleCommandParamOverride {
        param_id: amplitude,
        value: ParamValue::Float(0.3),
    });
    emit(&mut engine, event);
    invoke(
        &mut engine,
        id,
        "vibrate",
        vec![
            ParamValue::Float(300.0),
            ParamValue::Float(0.6),
            ParamValue::Int(60),
            ParamValue::Str("right".into()),
        ],
    );
    set(&mut engine, amplitude, ParamValue::Float(0.8));
    for _ in 0..20 {
        if manual_dispatch {
            tick(&mut engine);
        } else {
            engine.run_tick(Duration::from_millis(1)).unwrap();
            engine.apply_edits().unwrap();
        }
        thread::sleep(Duration::from_millis(1));
    }
    quiet(&mut engine, id);
    assert_eq!(output(&control), [("left".into(), 0.3), ("right".into(), 0.6)]);
    assert_eq!(value(&engine, amplitude), ParamValue::Float(0.8));
    finish(&mut engine, id, &slots);
}

#[test]
fn ordinary_ticks_keep_command_before_script_and_last_override() {
    mixed_order(false);
}
#[test]
fn partial_inbox_dispatch_still_keeps_older_command_before_script() {
    mixed_order(true);
}

#[test]
fn sixty_four_sources_cover_inbox_native_receipts_and_queued_disposal() {
    let control = Control::default();
    let (release, gate) = mpsc::channel();
    *control.gate.lock().unwrap() = Some(gate);
    let (mut engine, id, slots, _) = create_controlled(control.clone());
    for _ in 0..64 {
        invoke(&mut engine, id, "vibrate", vec![]);
    }
    assert_eq!(module(&mut engine, id).service.admission.metrics().active, 64);
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: id,
        call: ScriptMethodCall::prepare("vibrate", vec![]).unwrap(),
    });
    assert!(engine.apply_edits().is_err());
    assert!(control.actions.lock().unwrap().is_empty());
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine.apply_edits().unwrap();
    assert_eq!(module(&mut engine, id).service.outgoing.len(), 64);
    for _ in 0..8 {
        service(&mut engine, id);
    }
    until(|| control.actions.lock().unwrap().len() == 1);
    assert_eq!(module(&mut engine, id).service.pending.len(), 64);
    release.send(()).unwrap();
    until(|| module(&mut engine, id).service.pending.iter().all(Receipt::complete));
    let mut missing = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    module(&mut engine, id).service_io(&mut missing);
    assert!(missing.edits.pending.is_empty());
    assert_eq!(module(&mut engine, id).service.pending.len(), 64);
    let mut ctx = context(&engine);
    module(&mut engine, id).service_io(&mut ctx);
    assert_eq!(module(&mut engine, id).service.pending.len(), 48);
    assert_eq!(module(&mut engine, id).service.admission.metrics().active, 64);
    flush(&mut engine, &mut ctx);
    quiet(&mut engine, id);
    until(|| module(&mut engine, id).service.admission.metrics().active == 0);
    assert_eq!(control.actions.lock().unwrap().len(), 64);
    finish(&mut engine, id, &slots);
}

#[test]
fn removed_command_source_is_not_executed_after_a_blocked_action() {
    let control = Control::default();
    let (release, gate) = mpsc::channel();
    *control.gate.lock().unwrap() = Some(gate);
    let (mut engine, id, slots, _) = create_controlled(control.clone());
    let cmd = command(&mut engine, id);
    invoke(&mut engine, id, "vibrate", vec![]);
    tick(&mut engine);
    until(|| control.actions.lock().unwrap().len() == 1);
    let event = execution(&engine, cmd, "left");
    emit(&mut engine, event);
    for _ in 0..8 {
        tick(&mut engine);
    }
    engine.edits.push(Edit::RemoveNode { node: cmd });
    engine.apply_edits().unwrap();
    let dead_zone = path(&engine, id, "parameters/stick_processing/stick_dead_zone");
    let before = Instant::now();
    for _ in 0..30 {
        set(&mut engine, dead_zone, ParamValue::Float(0.2));
    }
    assert!(before.elapsed() < Duration::from_millis(500));
    release.send(()).unwrap();
    quiet(&mut engine, id);
    assert_eq!(control.actions.lock().unwrap().len(), 1);
    finish(&mut engine, id, &slots);
}

#[test]
fn disable_finishes_started_and_cancelled_receipts_through_bounded_disabled_wakes() {
    let control = Control::default();
    let (release, gate) = mpsc::channel();
    *control.gate.lock().unwrap() = Some(gate);
    let (mut engine, id, slots, _) = create_controlled(control.clone());
    invoke(&mut engine, id, "vibrate", vec![]);
    tick(&mut engine);
    until(|| control.actions.lock().unwrap().len() == 1);
    for _ in 0..63 {
        invoke(&mut engine, id, "vibrate", vec![]);
    }
    for _ in 0..8 {
        tick(&mut engine);
    }
    assert_eq!(module(&mut engine, id).service.pending.len(), 64);
    engine.edits.push(Edit::PatchMeta {
        node: id,
        patch: NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    release.send(()).unwrap();
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
    quiet(&mut engine, id);
    assert!(module(&mut engine, id).service.pending.is_empty());
    assert_eq!(control.actions.lock().unwrap().len(), 1);
}

#[test]
fn original_session_source_is_rejected_after_disconnect_and_reconnect() {
    let control = Control::default();
    let (mut engine, id, slots, input) = create_controlled(control.clone());
    let cmd = command(&mut engine, id);
    quiet(&mut engine, id);
    let captured = CapturedInvocation::capture(&engine.process_tree_snapshot(), cmd, None, &["joycon_module"]).unwrap();
    let producer = engine.node_waker(cmd).unwrap();
    let time = engine.time;
    let permit = module(&mut engine, id).service.admission.try_reserve().unwrap();
    let ingress = module(&mut engine, id).service.ingress.try_reserve().unwrap();
    let source = module(&mut engine, id)
        .runtime
        .as_ref()
        .unwrap()
        .capture(Input::Command(captured), producer, time, permit, ingress)
        .unwrap();
    let mut connected = JoyConRuntimeState::disconnected();
    connected.left.connected = true;
    input.lock().unwrap().extend([
        JoyConWorkerEvent::State(Box::new(connected.clone())),
        JoyConWorkerEvent::State(Box::new(JoyConRuntimeState::disconnected())),
        JoyConWorkerEvent::State(Box::new(connected)),
    ]);
    until(|| !source.live());
    source.arrive();
    let receipt = module(&mut engine, id)
        .runtime
        .as_ref()
        .unwrap()
        .submit(source)
        .unwrap_or_else(|_| panic!("fixture admission"));
    until(|| receipt.complete());
    assert_eq!(receipt.take(), Some(Outcome::NotSent));
    assert!(control.actions.lock().unwrap().is_empty());
    quiet(&mut engine, id);
    invoke(&mut engine, id, "vibrate", vec![]);
    quiet(&mut engine, id);
    assert_eq!(control.actions.lock().unwrap().len(), 1);
    finish(&mut engine, id, &slots);
}

#[test]
fn worker_handles_both_script_methods_and_refuses_oversized_arguments() {
    let control = Control::default();
    let (mut engine, id, slots, _) = create_controlled(control.clone());
    invoke(&mut engine, id, "vibrate", vec![]);
    invoke(
        &mut engine,
        id,
        "setPlayerLights",
        vec![ParamValue::Str("on".into()), ParamValue::Str("flash".into())],
    );
    quiet(&mut engine, id);
    assert_eq!(control.actions.lock().unwrap().len(), 2);
    let actions = control.actions.lock().unwrap();
    let JoyConWorkerCommand::SetPlayerLights(request) = &actions[1] else {
        panic!("lights");
    };
    assert_eq!(
        (&request.led_1, &request.led_2, &request.led_3, &request.target),
        (
            &"on".to_owned(),
            &"flash".to_owned(),
            &"off".to_owned(),
            &"both".to_owned()
        )
    );
    drop(actions);
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: id,
        call: ScriptMethodCall::prepare("vibrate", vec![ParamValue::Str("9".repeat(100_000))]).unwrap(),
    });
    assert!(engine.apply_edits().is_err());
    quiet(&mut engine, id);
    assert_eq!(control.actions.lock().unwrap().len(), 2);
    finish(&mut engine, id, &slots);
}

#[test]
fn large_numeric_execute_override_refuses_before_parsing_and_provider_remains_usable() {
    let control = Control::default();
    let (mut engine, id, slots, _) = create_controlled(control.clone());
    let cmd = command(&mut engine, id);
    let mut event = execution(&engine, cmd, "left");
    event.param_overrides.push(ModuleCommandParamOverride {
        param_id: path(&engine, cmd, "frequency_hz"),
        value: ParamValue::Str("9".repeat(100_000)),
    });
    emit(&mut engine, event);
    quiet(&mut engine, id);
    assert!(control.actions.lock().unwrap().is_empty());
    invoke(&mut engine, id, "vibrate", vec![]);
    quiet(&mut engine, id);
    assert_eq!(control.actions.lock().unwrap().len(), 1);
    finish(&mut engine, id, &slots);
}

#[test]
fn full_mailbox_keeps_the_same_source_behind_an_absolute_retry_deadline() {
    let control = Control::default();
    let (release, gate) = mpsc::channel();
    *control.gate.lock().unwrap() = Some(gate);
    let (mut engine, id, slots, _) = create_controlled(control.clone());
    invoke(&mut engine, id, "vibrate", vec![]);
    tick(&mut engine);
    until(|| control.actions.lock().unwrap().len() == 1);
    let mut receipts = Vec::new();
    for _ in 0..64 {
        let request = JoyConWorkerCommand::Vibrate(JoyConVibrateRequest {
            target: "both".into(),
            frequency_hz: 300.0,
            amplitude: 0.9,
            duration_ms: 60,
            description: "fixture".into(),
        });
        receipts.push(module(&mut engine, id).runtime.as_ref().unwrap().send(request).unwrap());
    }
    invoke(&mut engine, id, "vibrate", vec![]);
    for _ in 0..2 {
        engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
        engine.apply_edits().unwrap();
    }
    service(&mut engine, id);
    assert_eq!(module(&mut engine, id).service.outgoing.len(), 1);
    let deadline = module(&mut engine, id).service.output_retry_at.unwrap();
    for _ in 0..20 {
        module(&mut engine, id).request_service();
        tick(&mut engine);
    }
    assert_eq!(module(&mut engine, id).service.output_retry_at, Some(deadline));
    for _ in 0..20 {
        engine.run_tick(Duration::from_millis(1)).unwrap();
        assert_eq!(engine.tick_stats().wake_callbacks_fired, 0);
    }
    release.send(()).unwrap();
    until(|| {
        tick(&mut engine);
        control.actions.lock().unwrap().len() == 66
    });
    assert!(receipts.iter().all(Receipt::complete));
    quiet(&mut engine, id);
    finish(&mut engine, id, &slots);
}

#[test]
fn captured_override_storage_fills_mailbox_bytes_before_item_capacity() {
    let control = Control::default();
    let (release, gate) = mpsc::channel();
    *control.gate.lock().unwrap() = Some(gate);
    let (mut engine, id, slots, _) = create_controlled(control.clone());
    let cmd = command(&mut engine, id);
    let mut event = execution(&engine, cmd, "left");
    event.param_overrides.push(ModuleCommandParamOverride {
        param_id: NodeId(u64::MAX),
        value: ParamValue::Str("x".repeat(100_000)),
    });
    let invocation = CapturedInvocation::capture(
        &engine.process_tree_snapshot(),
        cmd,
        Some(CustomEvent::new(
            MODULE_COMMAND_EXECUTE_TOPIC,
            Some(cmd),
            serde_json::to_value(event).unwrap(),
        )),
        &["joycon_module"],
    )
    .unwrap();
    invoke(&mut engine, id, "vibrate", vec![]);
    tick(&mut engine);
    until(|| control.actions.lock().unwrap().len() == 1);
    let mut receipts = Vec::new();
    loop {
        let producer = engine.node_waker(cmd).unwrap();
        let time = engine.time;
        let permit = module(&mut engine, id).service.admission.try_reserve().unwrap();
        let ingress = module(&mut engine, id).service.ingress.try_reserve().unwrap();
        let runtime = module(&mut engine, id).runtime.as_ref().unwrap();
        let source = runtime
            .capture(Input::Command(invocation.clone()), producer, time, permit, ingress)
            .unwrap();
        source.arrive();
        match runtime.submit(source) {
            Ok(receipt) => receipts.push(receipt),
            Err(_) => break,
        }
        assert!(receipts.len() < 64);
    }
    assert!(receipts.len() > 1);
    module(&mut engine, id).stop_runtime();
    release.send(()).unwrap();
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
    assert!(receipts.iter().all(Receipt::complete));
    assert!(receipts.iter().all(|receipt| receipt.take() == Some(Outcome::NotSent)));
    assert_eq!(control.actions.lock().unwrap().len(), 1);
}

fn capture(engine: &mut AppEngine, id: NodeId, invocation: CapturedInvocation) -> RetainedPayload<Source> {
    let producer = engine.node_waker(invocation.command).unwrap();
    let time = engine.time;
    let permit = module(engine, id).service.admission.try_reserve().unwrap();
    let ingress = module(engine, id).service.ingress.try_reserve().unwrap();
    module(engine, id)
        .runtime
        .as_ref()
        .unwrap()
        .capture(Input::Command(invocation), producer, time, permit, ingress)
        .unwrap()
}

#[test]
fn captured_snapshot_keeps_earlier_command_values_after_later_authored_edits() {
    let control = Control::default();
    let (mut engine, id, slots, _) = create_controlled(control.clone());
    let cmd = command(&mut engine, id);
    let amplitude = path(&engine, cmd, "amplitude");
    set(&mut engine, amplitude, ParamValue::Float(0.3));
    let invocation =
        CapturedInvocation::capture(&engine.process_tree_snapshot(), cmd, None, &["joycon_module"]).unwrap();
    let source = capture(&mut engine, id, invocation);
    set(&mut engine, amplitude, ParamValue::Float(0.8));
    source.arrive();
    let receipt = module(&mut engine, id)
        .runtime
        .as_ref()
        .unwrap()
        .submit(source)
        .unwrap_or_else(|_| panic!("fixture admission"));
    until(|| receipt.complete());
    assert_eq!(output(&control), [("both".into(), 0.3)]);
    assert_eq!(value(&engine, amplitude), ParamValue::Float(0.8));
    finish(&mut engine, id, &slots);
}

#[test]
fn malformed_execute_payload_keeps_its_worker_diagnostic_within_1024_bytes() {
    let control = Control::default();
    let (mut engine, id, slots, _) = create_controlled(control.clone());
    let cmd = command(&mut engine, id);
    let mut payload = serde_json::to_value(execution(&engine, cmd, "left")).unwrap();
    payload["delivery_policy"] = serde_json::Value::String("é".repeat(50_000));
    let invocation = CapturedInvocation::capture(
        &engine.process_tree_snapshot(),
        cmd,
        Some(CustomEvent::new(MODULE_COMMAND_EXECUTE_TOPIC, Some(cmd), payload)),
        &["joycon_module"],
    )
    .unwrap();
    let source = capture(&mut engine, id, invocation);
    assert!(source.prepare().err().unwrap().len() > 1024);
    source.arrive();
    let receipt = module(&mut engine, id)
        .runtime
        .as_ref()
        .unwrap()
        .submit(source)
        .unwrap_or_else(|_| panic!("fixture admission"));
    until(|| receipt.complete());
    let Some(Outcome::Completed(Err(error))) = receipt.take() else {
        panic!("expected a bounded rejection");
    };
    assert!(error.len() <= 1024);
    assert!(!receipt.attempted());
    assert!(control.actions.lock().unwrap().is_empty());
    finish(&mut engine, id, &slots);
}

#[test]
fn disposing_undelivered_ingress_releases_the_dispatch_gate_and_wakes_service() {
    let control = Control::default();
    let (mut engine, id, slots, _) = create_controlled(control.clone());
    let cmd = command(&mut engine, id);
    quiet(&mut engine, id);
    let invocation =
        CapturedInvocation::capture(&engine.process_tree_snapshot(), cmd, None, &["joycon_module"]).unwrap();
    let source = capture(&mut engine, id, invocation);
    assert_eq!(module(&mut engine, id).service.ingress.metrics().active, 1);
    let source = match module(&mut engine, id).runtime.as_ref().unwrap().submit(source) {
        Err(source) => source,
        Ok(_) => panic!("unsettled ingress must not execute"),
    };
    assert!(control.actions.lock().unwrap().is_empty());
    drop(source);
    until(|| {
        engine.run_tick(Duration::from_millis(1)).unwrap();
        engine.apply_edits().unwrap();
        module(&mut engine, id).service.ingress.metrics().active == 0 && engine.tick_stats().wake_callbacks_fired != 0
    });
    assert!(control.actions.lock().unwrap().is_empty());
    finish(&mut engine, id, &slots);
}
