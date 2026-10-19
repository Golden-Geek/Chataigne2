use super::*;
use golden_core::{
    edit::NodeTree,
    parameter::{Parameter, ParameterChangeCheck},
    process_ctx::ProcessCtx,
};
use std::{net::UdpSocket, thread, time::Instant};

fn configured() -> (crate::app::AppEngine, NodeId, UdpSocket) {
    let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
    receiver.set_nonblocking(true).unwrap();
    let (mut engine, module) = create_udp_module();
    let connection = child_by_decl(&engine, module, "connection");
    let outputs = child_by_decl(&engine, connection, "outputs");
    let output = engine.nodes.get(outputs).unwrap().node_data().first_child.unwrap();
    let port = child_by_decl(&engine, output, "remote_port");
    set_param(
        &mut engine,
        port,
        ParamValue::Int(receiver.local_addr().unwrap().port().into()),
    );
    settle_transport_state(&mut engine);
    (engine, module, receiver)
}
fn add_command(engine: &mut crate::app::AppEngine, module: NodeId, command: crate::app::AppNode) -> NodeId {
    let tester = child_by_decl(engine, module, "command_tester");
    engine.add_node(command, Some(tester));
    engine.apply_edits().unwrap();
    for _ in 0..4 {
        engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
        engine.apply_edits().unwrap();
    }
    engine.nodes.get(tester).unwrap().node_data().last_child.unwrap()
}
fn execute(
    engine: &mut crate::app::AppEngine,
    command: NodeId,
    overrides: Vec<crate::app::module_command::ModuleCommandParamOverride>,
) {
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    crate::app::module_command::emit_command_execute_with_invocation(
        &mut ctx,
        command,
        overrides,
        None,
        Default::default(),
    )
    .unwrap();
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine.apply_edits().unwrap();
}
fn receive(engine: &mut crate::app::AppEngine, socket: &UdpSocket, count: usize) -> Vec<Vec<u8>> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut received = Vec::new();
    let mut bytes = [0; 65536];
    while received.len() < count {
        engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
        engine.apply_edits().unwrap();
        engine.run_tick(Duration::from_millis(10)).unwrap();
        engine.apply_edits().unwrap();
        assert_eq!(engine.tick_stats().callbacks_fired, 0);
        loop {
            match socket.recv(&mut bytes) {
                Ok(length) => received.push(bytes[..length].to_vec()),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) => panic!("{error}"),
            }
        }
        assert!(Instant::now() < deadline, "native command dispatch did not complete");
        thread::sleep(Duration::from_millis(1));
    }
    received
}
fn add_values(engine: &mut crate::app::AppEngine, command: NodeId, nested: bool) {
    let values = child_by_decl(engine, command, "values");
    let first = NodeTree::new(Parameter::new(
        "answer",
        ParamValue::Int(42),
        ParameterChangeCheck::None,
    ));
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    if nested {
        ctx.add_child_tree(values, NodeTree::new(Folder::new("scope")).with_child(first), None);
    } else {
        ctx.add_child_tree(values, first, None);
    }
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine.apply_edits().unwrap();
}

#[test]
fn every_stream_command_materializes_natively_and_dispatches_without_json_byte_events() {
    use crate::app::{
        StreamingSendBytesCommand, StreamingSendHexStringCommand, StreamingSendStringCommand,
        StreamingSendValuesAsJsonCommand, StreamingSendValuesCommand,
    };
    let (mut engine, module, socket) = configured();
    for (command, field, text, expected) in [
        (
            StreamingSendStringCommand::create().into(),
            "text",
            r"hello\n",
            b"hello\n".to_vec(),
        ),
        (
            StreamingSendBytesCommand::create().into(),
            "bytes",
            "1 0x02 255",
            vec![1, 2, 255],
        ),
        (
            StreamingSendHexStringCommand::create().into(),
            "hex",
            "48 69",
            b"Hi".to_vec(),
        ),
    ] {
        let command = add_command(&mut engine, module, command);
        let param = child_by_decl(&engine, command, field);
        set_param(&mut engine, param, ParamValue::Str(text.into()));
        engine.apply_edits().unwrap();
        execute(&mut engine, command, vec![]);
        assert_eq!(receive(&mut engine, &socket, 1), [expected]);
    }
    let command = add_command(&mut engine, module, StreamingSendValuesCommand::create().into());
    add_values(&mut engine, command, false);
    execute(&mut engine, command, vec![]);
    assert_eq!(receive(&mut engine, &socket, 1), [b"42".to_vec()]);
    let command = add_command(&mut engine, module, StreamingSendValuesAsJsonCommand::create().into());
    add_values(&mut engine, command, true);
    execute(&mut engine, command, vec![]);
    assert_eq!(
        receive(&mut engine, &socket, 1),
        [br#"{"scope":{"answer":42}}"#.to_vec()]
    );
    use crate::app::module::common::streaming::commands::preparation::PREPARED_COMMAND_TOPIC;
    assert!(!engine.ui_event_log().iter().any(|event| {
        matches!(&event.kind, golden_core::events::EventKind::Custom(event)
            if event.topic == PREPARED_COMMAND_TOPIC)
    }));
}

#[test]
fn invocation_overrides_remain_fifo_and_do_not_mutate_authored_parameters() {
    use crate::app::module_command::ModuleCommandParamOverride;
    let (mut engine, module, socket) = configured();
    let command = add_command(
        &mut engine,
        module,
        crate::app::StreamingSendStringCommand::create().into(),
    );
    let text = child_by_decl(&engine, command, "text");
    set_param(&mut engine, text, ParamValue::Str("authored".into()));
    engine.apply_edits().unwrap();
    for value in ["one", "two", "three"] {
        execute(
            &mut engine,
            command,
            vec![ModuleCommandParamOverride {
                param_id: text,
                value: ParamValue::Str(value.into()),
            }],
        );
    }
    assert_eq!(
        receive(&mut engine, &socket, 3),
        [b"one".to_vec(), b"two".to_vec(), b"three".to_vec()]
    );
    assert_eq!(
        engine.process_tree_snapshot().node(text).unwrap().param_value,
        Some(ParamValue::Str("authored".into()))
    );
}

#[test]
fn manual_trigger_captures_the_snapshot_before_later_authored_edits() {
    let (mut engine, module, socket) = configured();
    let command = add_command(
        &mut engine,
        module,
        crate::app::StreamingSendStringCommand::create().into(),
    );
    let text = child_by_decl(&engine, command, "text");
    let trigger = child_by_decl(&engine, command, "trigger");
    set_param(&mut engine, text, ParamValue::Str("captured".into()));
    set_param(&mut engine, trigger, ParamValue::Trigger());
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine.apply_edits().unwrap();
    set_param(&mut engine, text, ParamValue::Str("later".into()));
    engine.apply_edits().unwrap();
    assert_eq!(receive(&mut engine, &socket, 1), [b"captured".to_vec()]);
}

#[test]
fn native_script_conversion_failure_does_not_poison_the_shared_destination_ticket() {
    let (mut engine, module, socket) = configured();
    for (method, text) in [("sendHex", "zz"), ("sendText", "valid")] {
        engine.edits.push(Edit::CallNodeScriptMethod {
            node: module,
            call: golden_core::script::ScriptMethodCall::prepare(method, vec![ParamValue::Str(text.into())]).unwrap(),
        });
    }
    engine.apply_edits().unwrap();
    assert_eq!(receive(&mut engine, &socket, 1), [b"valid".to_vec()]);
}
