use super::*;
use crate::app::module_command::{
    CapturedInvocation, ModuleCommandDeliveryPolicy, ModuleCommandExecuteEvent, ModuleCommandParamOverride,
    MODULE_COMMAND_EXECUTE_TOPIC,
};
use golden_core::{events::CustomEvent, node::NodeMetaPatch};

fn execution(engine: &AppEngine, cmd: NodeId, target: &str) -> ModuleCommandExecuteEvent {
    ModuleCommandExecuteEvent {
        command_id: cmd,
        invocation_id: None,
        delivery_policy: ModuleCommandDeliveryPolicy::Standard,
        param_overrides: vec![
            ModuleCommandParamOverride {
                param_id: path(engine, cmd, "target"),
                value: ParamValue::Str("discarded".into()),
            },
            ModuleCommandParamOverride {
                param_id: path(engine, cmd, "target"),
                value: ParamValue::Str(target.into()),
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
fn output(control: &Control) -> Vec<(String, f64)> {
    control
        .actions
        .lock()
        .unwrap()
        .iter()
        .filter_map(|request| match request {
            ButtplugWorkerCommand::SetOutput(value) => Some((value.target.clone(), value.value)),
            _ => None,
        })
        .collect()
}

#[test]
fn ordinary_tick_preserves_command_before_script_and_last_override_without_snapshot_overlay() {
    let (mut engine, id) = create();
    let slots = slots();
    let control = Control::default();
    attach(&mut engine, id, control.clone(), &slots, None);
    quiet(&mut engine, id);
    let cmd = command(&mut engine, id);
    quiet(&mut engine, id);
    let target = path(&engine, cmd, "target");
    set(&mut engine, target, ParamValue::Str("x".repeat(1024 * 1024)));
    let max = path(&engine, id, "parameters/max_output");
    set(&mut engine, max, ParamValue::Float(0.25));
    let event = execution(&engine, cmd, "earlier");
    emit(&mut engine, event);
    invoke(
        &mut engine,
        id,
        "vibrate",
        vec![ParamValue::Float(0.9), ParamValue::Str("later".into())],
    );
    for _ in 0..20 {
        engine.run_tick(Duration::from_millis(1)).unwrap();
        engine.apply_edits().unwrap();
        thread::sleep(Duration::from_millis(1));
    }
    quiet(&mut engine, id);
    assert_eq!(output(&control), vec![("earlier".into(), 0.0), ("later".into(), 0.25)]);
    let ParamValue::Str(authored) = value(&engine, target) else {
        panic!()
    };
    assert_eq!(authored.len(), 1024 * 1024);
    assert!(authored.bytes().all(|byte| byte == b'x'));
    finish(&mut engine, id, &slots);
}

#[test]
fn script_capture_keeps_module_defaults_from_its_canonical_snapshot() {
    let (mut engine, id) = create();
    let slots = slots();
    let control = Control::default();
    attach(&mut engine, id, control.clone(), &slots, None);
    quiet(&mut engine, id);
    let max = path(&engine, id, "parameters/max_output");
    set(&mut engine, max, ParamValue::Float(0.25));
    invoke(&mut engine, id, "vibrate", vec![ParamValue::Float(0.9)]);
    set(&mut engine, max, ParamValue::Float(1.0));
    quiet(&mut engine, id);
    assert_eq!(output(&control), vec![("all".into(), 0.25)]);
    finish(&mut engine, id, &slots);
}

#[test]
fn module_admission_covers_local_inbox_native_work_results_and_queued_disposal() {
    let (mut engine, id) = create();
    let slots = slots();
    let control = Control::default();
    let (release, gate) = mpsc::channel();
    attach(&mut engine, id, control.clone(), &slots, Some(gate));
    quiet(&mut engine, id);
    for _ in 0..64 {
        invoke(&mut engine, id, "stopAll", vec![]);
    }
    assert_eq!(module(&mut engine, id).service.admission.metrics().active, 64);
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: id,
        call: ScriptMethodCall::prepare("stopAll", vec![]).unwrap(),
    });
    assert!(engine.apply_edits().is_err());
    assert!(control.actions.lock().unwrap().is_empty());
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine.apply_edits().unwrap();
    assert_eq!(module(&mut engine, id).service.outgoing.len(), 64);
    for _ in 0..8 {
        let mut ctx = context(&engine);
        module(&mut engine, id).service_io(&mut ctx);
        flush(&mut engine, &mut ctx);
    }
    until(|| control.actions.lock().unwrap().len() == 1);
    assert_eq!(module(&mut engine, id).service.pending.len(), 64);
    release.send(()).unwrap();
    until(|| {
        module(&mut engine, id)
            .service
            .pending
            .iter()
            .all(CommandReceipt::complete)
    });
    let mut missing = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    module(&mut engine, id).service_io(&mut missing);
    assert_eq!(module(&mut engine, id).service.pending.len(), 64);
    assert!(missing.edits.pending.is_empty());
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
fn deleting_a_queued_command_prevents_execution_while_unrelated_edits_continue() {
    let (mut engine, id) = create();
    let cmd = command(&mut engine, id);
    let slots = slots();
    let control = Control::default();
    let (release, gate) = mpsc::channel();
    attach(&mut engine, id, control.clone(), &slots, Some(gate));
    quiet(&mut engine, id);
    invoke(&mut engine, id, "stopAll", vec![]);
    turn(&mut engine);
    until(|| control.actions.lock().unwrap().len() == 1);
    let event = execution(&engine, cmd, "removed");
    emit(&mut engine, event);
    for _ in 0..8 {
        turn(&mut engine);
    }
    engine.edits.push(Edit::RemoveNode { node: cmd });
    engine.apply_edits().unwrap();
    let max = path(&engine, id, "parameters/max_output");
    let start = Instant::now();
    for _ in 0..30 {
        set(&mut engine, max, ParamValue::Float(0.5));
    }
    assert!(start.elapsed() < Duration::from_millis(500));
    release.send(()).unwrap();
    quiet(&mut engine, id);
    assert_eq!(control.actions.lock().unwrap().len(), 1);
    assert!(module(&mut engine, id).transport.as_ref().unwrap().is_running());
    finish(&mut engine, id, &slots);
}

#[test]
fn disable_preserves_the_started_result_and_drains_all_cancelled_receipts_in_bounded_turns() {
    let (mut engine, id) = create();
    let slots = slots();
    let control = Control::default();
    let (release, gate) = mpsc::channel();
    attach(&mut engine, id, control.clone(), &slots, Some(gate));
    quiet(&mut engine, id);
    invoke(&mut engine, id, "stopAll", vec![]);
    turn(&mut engine);
    until(|| control.actions.lock().unwrap().len() == 1);
    for _ in 0..63 {
        invoke(&mut engine, id, "stopAll", vec![]);
    }
    for _ in 0..8 {
        turn(&mut engine);
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
    let mut ctx = context(&engine);
    module(&mut engine, id).service_io(&mut ctx);
    assert_eq!(module(&mut engine, id).service.pending.len(), 48);
    flush(&mut engine, &mut ctx);
    until(|| {
        turn(&mut engine);
        module(&mut engine, id).service.pending.is_empty()
    });
    assert_eq!(control.actions.lock().unwrap().len(), 1);
}

#[test]
fn all_script_methods_share_worker_preparation_and_oversized_inputs_reject_without_native_work() {
    let (mut engine, id) = create();
    let slots = slots();
    let control = Control::default();
    attach(&mut engine, id, control.clone(), &slots, None);
    quiet(&mut engine, id);
    for method in ["startScanning", "stopScanning", "stopAllDevices", "stopAll"] {
        invoke(&mut engine, id, method, vec![]);
    }
    invoke(&mut engine, id, "stopDevice", vec![]);
    invoke(
        &mut engine,
        id,
        "setOutput",
        vec![ParamValue::Str("vibrate".into()), ParamValue::Float(0.1)],
    );
    for method in ["vibrate", "rotate", "oscillate", "position"] {
        invoke(&mut engine, id, method, vec![ParamValue::Float(0.2)]);
    }
    invoke(
        &mut engine,
        id,
        "positionWithDuration",
        vec![ParamValue::Float(0.3), ParamValue::Int(250)],
    );
    quiet(&mut engine, id);
    assert_eq!(control.actions.lock().unwrap().len(), 11);
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: id,
        call: ScriptMethodCall::prepare("stopDevice", vec![ParamValue::Str("x".repeat(100_000))]).unwrap(),
    });
    assert!(engine.apply_edits().is_err());
    quiet(&mut engine, id);
    assert_eq!(control.actions.lock().unwrap().len(), 11);
    finish(&mut engine, id, &slots);
}

#[test]
fn captured_override_bytes_fill_the_native_mailbox_before_its_item_limit() {
    let (mut engine, id) = create();
    let cmd = command(&mut engine, id);
    let mut event = execution(&engine, cmd, "ignored");
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
        &["buttplug_module"],
    )
    .unwrap();
    let slots = slots();
    let control = Control::default();
    let (release, gate) = mpsc::channel();
    attach(&mut engine, id, control.clone(), &slots, Some(gate));
    quiet(&mut engine, id);
    invoke(&mut engine, id, "stopAll", vec![]);
    turn(&mut engine);
    until(|| control.actions.lock().unwrap().len() == 1);
    let mut receipts = Vec::new();
    loop {
        let producer = engine.node_waker(cmd).unwrap();
        let time = engine.time;
        let transport = module(&mut engine, id).transport.as_ref().unwrap();
        let source = transport
            .capture(
                Input::Command(invocation.clone()),
                producer,
                time,
                RetirementPool::new(NonZeroUsize::MIN).try_reserve().unwrap(),
            )
            .unwrap();
        match transport.submit(source) {
            Ok(receipt) => receipts.push(receipt),
            Err(_) => break,
        }
        assert!(receipts.len() < 64);
    }
    assert!(receipts.len() > 1);
    module(&mut engine, id).stop_transport();
    release.send(()).unwrap();
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
    assert!(receipts.iter().all(CommandReceipt::complete));
    assert!(receipts
        .iter()
        .all(|receipt| receipt.take() == Some(CommandStatus::NotSent)));
    assert_eq!(control.actions.lock().unwrap().len(), 1);
}

#[test]
fn a_captured_source_from_a_lost_session_is_not_executed_after_reconnect() {
    let (mut engine, id) = create();
    let cmd = command(&mut engine, id);
    let slots = slots();
    let control = Control::default();
    attach(&mut engine, id, control.clone(), &slots, None);
    quiet(&mut engine, id);
    let invocation =
        CapturedInvocation::capture(&engine.process_tree_snapshot(), cmd, None, &["buttplug_module"]).unwrap();
    let producer = engine.node_waker(cmd).unwrap();
    let time = engine.time;
    let permit = module(&mut engine, id).service.admission.try_reserve().unwrap();
    let transport = module(&mut engine, id).transport.as_ref().unwrap();
    let source = transport
        .capture(Input::Command(invocation), producer, time, permit)
        .unwrap();
    control.fail_poll.store(true, Ordering::Release);
    until(|| !transport.shared.connected.load(Ordering::Acquire));
    until(|| transport.shared.connected.load(Ordering::Acquire));
    assert!(!source.live());
    let receipt = transport.submit(source).ok().unwrap();
    until(|| receipt.complete());
    assert_eq!(receipt.take(), Some(CommandStatus::NotSent));
    assert!(control.actions.lock().unwrap().is_empty());
    invoke(&mut engine, id, "stopAll", vec![]);
    quiet(&mut engine, id);
    assert_eq!(control.actions.lock().unwrap().len(), 1);
    finish(&mut engine, id, &slots);
}

#[test]
fn oversized_consumed_command_field_fails_before_io_and_the_same_provider_remains_usable() {
    let (mut engine, id) = create();
    let cmd = command(&mut engine, id);
    let slots = slots();
    let control = Control::default();
    attach(&mut engine, id, control.clone(), &slots, None);
    quiet(&mut engine, id);
    let target = path(&engine, cmd, "target");
    set(&mut engine, target, ParamValue::Str("x".repeat(1024 * 1024)));
    emit(
        &mut engine,
        ModuleCommandExecuteEvent {
            command_id: cmd,
            invocation_id: None,
            delivery_policy: ModuleCommandDeliveryPolicy::Standard,
            param_overrides: vec![],
        },
    );
    quiet(&mut engine, id);
    assert!(control.actions.lock().unwrap().is_empty());
    assert!(module(&mut engine, id).transport.as_ref().unwrap().is_running());
    invoke(&mut engine, id, "stopAll", vec![]);
    quiet(&mut engine, id);
    assert_eq!(control.actions.lock().unwrap().len(), 1);
    finish(&mut engine, id, &slots);
}
