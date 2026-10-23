use super::engine::*;
use super::*;
use crate::app::module_command::{
    CapturedInvocation, ModuleCommandDeliveryPolicy, ModuleCommandExecuteEvent, ModuleCommandParamOverride,
    MODULE_COMMAND_EXECUTE_TOPIC,
};
use golden_core::{edit::Edit, events::CustomEvent, node::NodeMetaPatch, parameter::ParamValue};

fn execute(engine: &mut crate::app::AppEngine, cmd: golden_core::node::NodeId, key: &str) {
    let key_id = path(engine, cmd, "button");
    let event = ModuleCommandExecuteEvent {
        command_id: cmd,
        invocation_id: None,
        delivery_policy: ModuleCommandDeliveryPolicy::Standard,
        param_overrides: vec![
            ModuleCommandParamOverride {
                param_id: key_id,
                value: ParamValue::Enum("discarded".into()),
            },
            ModuleCommandParamOverride {
                param_id: key_id,
                value: ParamValue::Enum(key.into()),
            },
        ],
    };
    engine.edits.push(Edit::EmitCustomEvent {
        event: CustomEvent::new(
            MODULE_COMMAND_EXECUTE_TOPIC,
            Some(cmd),
            serde_json::to_value(event).unwrap(),
        ),
    });
    engine.apply_edits().unwrap();
}
fn mixed_order(manual: bool) {
    let control = Control::default();
    let (mut engine, id, slots) = create(control.clone());
    let cmd = command(&mut engine, id);
    quiet(&mut engine, id);
    execute(&mut engine, cmd, "left");
    invoke(&mut engine, id, "pressButton", vec![ParamValue::Str("middle".into())]);
    let key = path(&engine, cmd, "button");
    set(&mut engine, key, ParamValue::Enum("right".into()));
    for _ in 0..20 {
        if manual {
            tick(&mut engine);
        } else {
            engine.run_tick(Duration::from_millis(1)).unwrap();
            engine.apply_edits().unwrap();
        }
        thread::sleep(Duration::from_millis(1));
    }
    quiet(&mut engine, id);
    assert_eq!(
        *control.calls.lock().unwrap(),
        [
            request(MouseButtonKind::Left),
            Request::Button {
                button: MouseButtonKind::Middle,
                action: MouseButtonAction::Press
            }
        ]
    );
    assert_eq!(value(&engine, key), ParamValue::Enum("right".into()));
    finish(&mut engine, id, &slots);
}
#[test]
fn ordinary_ticks_keep_command_before_script_and_last_override() {
    mixed_order(false);
}
#[test]
fn partial_dispatch_keeps_older_command_before_later_script() {
    mixed_order(true);
}
#[test]
fn scripts_keep_all_three_actions_and_worker_validation_does_not_break_later_calls() {
    let control = Control::default();
    let (mut engine, id, slots) = create(control.clone());
    for method in ["click", "pressButton", "releaseButton"] {
        invoke(&mut engine, id, method, vec![ParamValue::Str("left".into())]);
        quiet(&mut engine, id);
    }
    invoke(&mut engine, id, "click", vec![ParamValue::Str("x".repeat(65))]);
    quiet(&mut engine, id);
    assert_eq!(control.calls.lock().unwrap().len(), 3);
    let last = path(&engine, id, "values/info/last_event");
    assert!(value(&engine, last).as_str().unwrap().contains("64 bytes"));
    invoke(&mut engine, id, "click", vec![ParamValue::Str("middle".into())]);
    quiet(&mut engine, id);
    assert_eq!(
        control
            .calls
            .lock()
            .unwrap()
            .iter()
            .map(|request| match request {
                Request::Button { action, .. } => *action,
                _ => panic!("button request"),
            })
            .collect::<Vec<_>>(),
        [
            MouseButtonAction::Click,
            MouseButtonAction::Press,
            MouseButtonAction::Release,
            MouseButtonAction::Click
        ]
    );
    finish(&mut engine, id, &slots);
}
#[test]
fn removed_command_producer_is_not_executed_after_a_blocked_effect() {
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
    execute(&mut engine, cmd, "middle");
    tick(&mut engine);
    tick(&mut engine);
    engine.edits.push(Edit::RemoveNode { node: cmd });
    engine.apply_edits().unwrap();
    release.send(()).unwrap();
    quiet(&mut engine, id);
    assert_eq!(control.calls.lock().unwrap().len(), 1);
    finish(&mut engine, id, &slots);
}
#[test]
fn disabling_finishes_accepted_receipts_through_disabled_worker_wakes() {
    let control = Control::default();
    let (release, gate) = mpsc::channel();
    *control.gate.lock().unwrap() = Some(gate);
    let (mut engine, id, slots) = create(control.clone());
    for _ in 0..8 {
        invoke(&mut engine, id, "click", vec![ParamValue::Str("left".into())]);
    }
    until(|| {
        tick(&mut engine);
        control.calls.lock().unwrap().len() == 1 && module(&mut engine, id).output.receipts.len() == 8
    });
    engine.edits.push(Edit::PatchMeta {
        node: id,
        patch: NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    release.send(()).unwrap();
    quiet(&mut engine, id);
    until(|| module(&mut engine, id).output.admission.metrics().active == 0);
    assert_eq!(control.calls.lock().unwrap().len(), 1);
    assert!(module(&mut engine, id).output.runtime.is_none());
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
}
#[test]
fn canonical_snapshot_survives_later_authored_values_without_an_execute_overlay() {
    let control = Control::default();
    let (mut engine, id, slots) = create(control.clone());
    let cmd = command(&mut engine, id);
    let key = path(&engine, cmd, "button");
    set(&mut engine, key, ParamValue::Enum("left".into()));
    quiet(&mut engine, id);
    let captured = CapturedInvocation::capture(&engine.process_tree_snapshot(), cmd, None, &["mouse_module"]).unwrap();
    let producer = engine.node_waker(cmd).unwrap();
    let time = engine.time;
    let mouse = module(&mut engine, id);
    let source = mouse
        .output
        .runtime
        .as_ref()
        .unwrap()
        .capture(
            Input::Command(captured),
            producer,
            time,
            mouse.output.admission.try_reserve().unwrap(),
            mouse.output.ingress.try_reserve().unwrap(),
        )
        .unwrap();
    set(&mut engine, key, ParamValue::Enum("middle".into()));
    source.arrive();
    let receipt = module(&mut engine, id)
        .output
        .runtime
        .as_ref()
        .unwrap()
        .submit(source)
        .unwrap_or_else(|_| panic!("fixture admission"));
    until(|| receipt.complete());
    assert_eq!(*control.calls.lock().unwrap(), [request(MouseButtonKind::Left)]);
    assert_eq!(value(&engine, key), ParamValue::Enum("middle".into()));
    finish(&mut engine, id, &slots);
}
#[test]
fn script_admission_rejects_extra_arguments_before_native_capture() {
    let control = Control::default();
    let (mut engine, id, slots) = create(control.clone());
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: id,
        call: golden_core::script::ScriptMethodCall::prepare("click", vec![ParamValue::Str("left".into()); 5]).unwrap(),
    });
    assert!(engine.apply_edits().is_err());
    assert!(control.calls.lock().unwrap().is_empty());
    assert_eq!(module(&mut engine, id).output.admission.metrics().active, 0);
    finish(&mut engine, id, &slots);
}
