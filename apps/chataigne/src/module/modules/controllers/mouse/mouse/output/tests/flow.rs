use super::engine::*;
use super::*;
use crate::app::module_command::{
    CapturedInvocation, ModuleCommandDeliveryPolicy, ModuleCommandExecuteEvent, ModuleCommandParamOverride,
    MODULE_COMMAND_EXECUTE_TOPIC,
};
use golden_core::{edit::Edit, events::CustomEvent, parameter::ParamValue, process_ctx::ExecutionPhase};

#[test]
fn actor_admission_covers_sixty_four_local_native_completed_and_queued_sources() {
    let control = Control::default();
    let (release, gate) = mpsc::channel();
    *control.gate.lock().unwrap() = Some(gate);
    let (mut engine, id, slots) = create(control.clone());
    for _ in 0..64 {
        invoke(&mut engine, id, "click", vec![ParamValue::Str("left".into())]);
    }
    assert_eq!(module(&mut engine, id).output.admission.metrics().active, 64);
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: id,
        call: golden_core::script::ScriptMethodCall::prepare("click", vec![ParamValue::Str("middle".into())]).unwrap(),
    });
    assert!(engine.apply_edits().is_err());
    assert!(control.calls.lock().unwrap().is_empty());
    for _ in 0..2 {
        engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
        engine.apply_edits().unwrap();
    }
    assert_eq!(module(&mut engine, id).output.outgoing.len(), 64);
    for _ in 0..8 {
        service(&mut engine, id);
    }
    assert_eq!(module(&mut engine, id).output.receipts.len(), 64);
    until(|| control.calls.lock().unwrap().len() == 1);
    release.send(()).unwrap();
    until(|| module(&mut engine, id).output.receipts.iter().all(Receipt::complete));
    let mut ctx = context(&engine);
    module(&mut engine, id).service_output(&mut ctx);
    assert_eq!(module(&mut engine, id).output.receipts.len(), 48);
    assert_eq!(module(&mut engine, id).output.admission.metrics().active, 64);
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    quiet(&mut engine, id);
    until(|| module(&mut engine, id).output.admission.metrics().active == 0);
    assert_eq!(control.calls.lock().unwrap().len(), 64);
    finish(&mut engine, id, &slots);
}
#[test]
fn execute_override_bytes_fill_native_mailbox_before_items_and_retry_keeps_the_same_source() {
    let control = Control::default();
    let (release, gate) = mpsc::channel();
    *control.gate.lock().unwrap() = Some(gate);
    let (mut engine, id, slots) = create(control.clone());
    invoke(&mut engine, id, "click", vec![ParamValue::Str("left".into())]);
    until(|| {
        tick(&mut engine);
        control.calls.lock().unwrap().len() == 1
    });
    let cmd = command(&mut engine, id);
    let event = ModuleCommandExecuteEvent {
        command_id: cmd,
        invocation_id: None,
        delivery_policy: ModuleCommandDeliveryPolicy::Standard,
        param_overrides: vec![ModuleCommandParamOverride {
            param_id: path(&engine, cmd, "button"),
            value: ParamValue::Enum("x".repeat(20_000)),
        }],
    };
    let captured = CapturedInvocation::capture(
        &engine.process_tree_snapshot(),
        cmd,
        Some(CustomEvent::new(
            MODULE_COMMAND_EXECUTE_TOPIC,
            Some(cmd),
            serde_json::to_value(event).unwrap(),
        )),
        &["mouse_module"],
    )
    .unwrap();
    let mut receipts = Vec::new();
    let refused = loop {
        let mouse = module(&mut engine, id);
        let runtime = mouse.output.runtime.as_ref().unwrap();
        let source = runtime
            .capture_fixture(
                Input::Command(captured.clone()),
                golden_core::engine::EngineTime {
                    tick: 1,
                    micro: 0,
                    seq: 0,
                },
                mouse.output.admission.try_reserve().unwrap(),
                mouse.output.ingress.try_reserve().unwrap(),
            )
            .unwrap();
        source.arrive();
        match runtime.submit(source) {
            Ok(receipt) => receipts.push(receipt),
            Err(source) => break source,
        }
        assert!(receipts.len() < 63);
    };
    assert!(receipts.len() > 1 && receipts.len() < 63);
    let original = refused.clone();
    let refused = match module(&mut engine, id).output.runtime.as_ref().unwrap().submit(refused) {
        Err(source) => source,
        Ok(_) => panic!("full mailbox"),
    };
    assert!(std::ptr::eq(&*original, &*refused));
    drop(original);
    release.send(()).unwrap();
    until(|| receipts.iter().all(Receipt::complete));
    let receipt = module(&mut engine, id)
        .output
        .runtime
        .as_ref()
        .unwrap()
        .submit(refused)
        .unwrap_or_else(|_| panic!("retry capacity"));
    until(|| receipt.complete());
    assert!(matches!(receipt.take(), Some(Outcome::Completed(Err(_)))));
    assert_eq!(control.calls.lock().unwrap().len(), 1);
    quiet(&mut engine, id);
    finish(&mut engine, id, &slots);
}
#[test]
fn eight_send_reconfigurations_retire_old_sources_and_reuse_one_native_lifetime() {
    let control = Control::default();
    let (mut engine, id, slots) = create(control.clone());
    let send = path(&engine, id, "connection/send_enabled");
    let mut old = Vec::new();
    for round in 0..8 {
        let time = engine.time;
        let mouse = module(&mut engine, id);
        let source = mouse
            .output
            .runtime
            .as_ref()
            .unwrap()
            .capture_fixture(
                Input::Prepared(request(MouseButtonKind::Middle)),
                time,
                mouse.output.admission.try_reserve().unwrap(),
                mouse.output.ingress.try_reserve().unwrap(),
            )
            .unwrap();
        source.arrive();
        old.push(source);
        set(&mut engine, send, ParamValue::Bool(false));
        tick(&mut engine);
        assert!(slots.wait_for_idle(Duration::from_secs(1)));
        assert!(old.iter().all(|source| !source.live()));
        set(&mut engine, send, ParamValue::Bool(true));
        tick(&mut engine);
        install(&mut engine, id, control.clone(), &slots);
        quiet(&mut engine, id);
        invoke(&mut engine, id, "click", vec![ParamValue::Str("left".into())]);
        quiet(&mut engine, id);
        assert_eq!(control.calls.lock().unwrap().len(), round + 1);
        assert_eq!(slots.metrics().active, 1);
    }
    assert_eq!(slots.metrics().peak, 1);
    finish(&mut engine, id, &slots);
}
