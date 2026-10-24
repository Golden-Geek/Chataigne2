use super::super::tests::service::{base, fixture, receive, receiver, set, settle, turn};
use super::*;
use crate::app::{
    AppEngine, DmxSendFrameCommand,
    module_command::{ModuleCommandDeliveryPolicy, ModuleCommandExecuteEvent, ModuleCommandParamOverride},
};
use golden_core::{edit::Edit, events::CustomEvent, parameter::ParameterEventBehaviour, script::ScriptMethodCall};

fn script(engine: &mut AppEngine, id: NodeId, method: &str, args: Vec<ParamValue>) {
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: id,
        call: ScriptMethodCall::prepare(method, args).unwrap(),
    });
}
fn child(engine: &AppEngine, parent: NodeId, name: &str) -> NodeId {
    engine
        .process_tree_snapshot()
        .find_child_by_decl_id(parent, name)
        .unwrap()
}
fn command(engine: &mut AppEngine, module: NodeId) -> NodeId {
    let tester = child(engine, module, "command_tester");
    engine.add_user_item(DmxSendFrameCommand::create().into(), Some(tester));
    for _ in 0..5 {
        turn(engine);
    }
    engine.nodes.get(tester).unwrap().node_data().first_child.unwrap()
}

#[test]
fn ordered_script_deltas_preserve_every_prior_channel_and_reflect_the_last_frame() {
    let receiver = receiver();
    let (mut engine, id) = fixture(&receiver);
    for channel in 1..=16 {
        script(
            &mut engine,
            id,
            "setChannel",
            vec![ParamValue::Int(channel), ParamValue::Int(channel * 2)],
        );
    }
    engine.apply_edits().unwrap();
    assert_eq!(base(&mut engine, id).outgoing.pending.len(), 16);
    settle(&mut engine, id);
    for channel in 1..=16 {
        let frame = receive(&receiver);
        for previous in 1..=channel {
            assert_eq!(frame[previous - 1], (previous * 2) as u8);
        }
        assert!(frame[channel..].iter().all(|value| *value == 0));
    }
    let output = base(&mut engine, id).output_frame.id();
    let value = engine.nodes.get(output).unwrap().engine_param_snapshot().unwrap().value;
    assert_eq!(
        parse_slots_json(&value.as_str().unwrap()).unwrap(),
        base(&mut engine, id).output_slots
    );
    assert!(base(&mut engine, id).outgoing.pending.is_empty());
    base(&mut engine, id).stop_transport();
}

#[test]
fn authored_frame_occurrences_and_script_calls_share_engine_order() {
    let receiver = receiver();
    let (mut engine, id) = fixture(&receiver);
    let output = base(&mut engine, id).output_frame.id();
    engine.edits.push(Edit::SetParam {
        node: output,
        value: ParamValue::Str("[5]".into()),
        behaviour: ParameterEventBehaviour::Append,
    });
    script(
        &mut engine,
        id,
        "setChannel",
        vec![ParamValue::Int(2), ParamValue::Int(8)],
    );
    engine.edits.push(Edit::SetParam {
        node: output,
        value: ParamValue::Str("[9]".into()),
        behaviour: ParameterEventBehaviour::Append,
    });
    script(
        &mut engine,
        id,
        "setChannel",
        vec![ParamValue::Int(3), ParamValue::Int(11)],
    );
    engine.apply_edits().unwrap();
    settle(&mut engine, id);
    assert_eq!(receive(&receiver)[0], 5);
    assert_eq!(&receive(&receiver)[..3], &[5, 8, 0]);
    assert_eq!(receive(&receiver)[0], 9);
    assert_eq!(&receive(&receiver)[..3], &[9, 0, 11]);
    let text = engine
        .nodes
        .get(output)
        .unwrap()
        .engine_param_snapshot()
        .unwrap()
        .value
        .as_str()
        .unwrap();
    assert_eq!(&parse_slots_json(&text).unwrap()[..3], &[9, 0, 11]);
    base(&mut engine, id).stop_transport();
}

#[test]
fn captured_command_last_override_replaces_large_authored_frame_without_an_overlay() {
    let receiver = receiver();
    let (mut engine, id) = fixture(&receiver);
    let command = command(&mut engine, id);
    let slots = child(&engine, command, "slots");
    set(&mut engine, slots, ParamValue::Str("x".repeat(1024 * 1024)));
    let execution = ModuleCommandExecuteEvent {
        command_id: command,
        invocation_id: None,
        delivery_policy: ModuleCommandDeliveryPolicy::Standard,
        param_overrides: vec![
            ModuleCommandParamOverride {
                param_id: slots,
                value: ParamValue::Str("[1]".into()),
            },
            ModuleCommandParamOverride {
                param_id: slots,
                value: ParamValue::Str(slots_json(&vec![5; 512])),
            },
        ],
    };
    engine.edits.push(Edit::EmitCustomEvent {
        event: CustomEvent::new(
            crate::app::module_command::MODULE_COMMAND_EXECUTE_TOPIC,
            Some(command),
            serde_json::to_value(execution).unwrap(),
        ),
    });
    engine.apply_edits().unwrap();
    settle(&mut engine, id);
    assert_eq!(receive(&receiver), vec![5; 512]);
    assert_eq!(
        engine
            .nodes
            .get(slots)
            .unwrap()
            .engine_param_snapshot()
            .unwrap()
            .value
            .as_str()
            .unwrap()
            .len(),
        1024 * 1024
    );
    base(&mut engine, id).stop_transport();
}

#[test]
fn script_capture_keeps_canonical_arguments_and_a_newer_authored_frame_wins_projection() {
    let receiver = receiver();
    let (mut engine, id) = fixture(&receiver);
    let call = ScriptMethodCall::prepare("sendFrame", vec![ParamValue::Str("[7]".into())]).unwrap();
    let pointer = call.args().as_ptr();
    engine.edits.push(Edit::CallNodeScriptMethod { node: id, call });
    engine.apply_edits().unwrap();
    let Input::Script { args, .. } = &base(&mut engine, id).outgoing.pending.front().unwrap().source.input else {
        panic!()
    };
    assert_eq!(pointer, args.as_ptr());
    let output = base(&mut engine, id).output_frame.id();
    set(&mut engine, output, ParamValue::Str("[9]".into()));
    settle(&mut engine, id);
    assert_eq!(receive(&receiver)[0], 7);
    assert_eq!(receive(&receiver)[0], 9);
    assert_eq!(
        engine.nodes.get(output).unwrap().engine_param_snapshot().unwrap().value,
        ParamValue::Str("[9]".into())
    );
    base(&mut engine, id).stop_transport();
}

#[test]
fn reconfiguration_rejects_the_captured_session_without_replaying_it() {
    let receiver = receiver();
    let (mut engine, id) = fixture(&receiver);
    let universe = base(&mut engine, id).universe.id();
    for next in 2..=9 {
        script(&mut engine, id, "sendFrame", vec![ParamValue::Str("[7]".into())]);
        engine.apply_edits().unwrap();
        let source = base(&mut engine, id).outgoing.pending.front().unwrap().source.clone();
        set(&mut engine, universe, ParamValue::Int(next));
        settle(&mut engine, id);
        assert!(!source.is_live());
        assert!(base(&mut engine, id).outgoing.pending.is_empty());
        assert!(base(&mut engine, id).output_slots.is_empty());
    }
    receiver.set_read_timeout(Some(Duration::from_millis(20))).unwrap();
    assert!(receiver.recv(&mut [0; 1024]).is_err());
    base(&mut engine, id).stop_transport();
}

#[test]
fn command_and_script_occurrences_are_ordered_without_an_explicit_inbox_flush() {
    let receiver = receiver();
    let (mut engine, id) = fixture(&receiver);
    let command = command(&mut engine, id);
    let slots = child(&engine, command, "slots");
    set(&mut engine, slots, ParamValue::Str("[3]".into()));
    let execution = ModuleCommandExecuteEvent {
        command_id: command,
        param_overrides: vec![],
        invocation_id: None,
        delivery_policy: ModuleCommandDeliveryPolicy::Standard,
    };
    engine.edits.push(Edit::EmitCustomEvent {
        event: CustomEvent::new(
            crate::app::module_command::MODULE_COMMAND_EXECUTE_TOPIC,
            Some(command),
            serde_json::to_value(execution).unwrap(),
        ),
    });
    script(
        &mut engine,
        id,
        "setChannel",
        vec![ParamValue::Int(2), ParamValue::Int(8)],
    );
    engine.apply_edits().unwrap();
    // Production run_tick preprocesses internal state before worker wakes, and
    // dispatches application inbox callbacks during later stabilization.
    for _ in 0..20 {
        engine.run_tick(Duration::from_millis(1)).unwrap();
        engine.apply_edits().unwrap();
        std::thread::sleep(Duration::from_millis(1));
    }
    settle(&mut engine, id);
    assert_eq!(receive(&receiver)[0], 3);
    assert_eq!(&receive(&receiver)[..2], &[3, 8]);
    base(&mut engine, id).stop_transport();
}

#[test]
fn pending_invocation_count_is_bounded_before_any_preparation_runs() {
    let receiver = receiver();
    let (mut engine, id) = fixture(&receiver);
    for _ in 0..64 {
        script(&mut engine, id, "blackout", vec![]);
    }
    engine.apply_edits().unwrap();
    assert_eq!(base(&mut engine, id).outgoing.pending.len(), 64);
    script(&mut engine, id, "blackout", vec![]);
    assert!(engine.apply_edits().is_err());
    assert_eq!(base(&mut engine, id).outgoing.pending.len(), 64);
    base(&mut engine, id).clear_output();
    base(&mut engine, id).stop_transport();
}
