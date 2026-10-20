use super::*;

fn receiver() -> UdpSocket {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    socket.set_nonblocking(true).unwrap();
    socket
}

fn packets(engine: &mut crate::app::AppEngine, socket: &UdpSocket, expected: usize) -> Vec<rosc::OscMessage> {
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    let mut messages = Vec::new();
    let mut bytes = [0u8; 4096];
    while messages.len() < expected {
        service_osc_module_turn(engine);
        match socket.recv(&mut bytes) {
            Ok(length) => match decoder::decode_udp(&bytes[..length]).unwrap().1 {
                OscPacket::Message(message) => messages.push(message),
                other => panic!("unexpected OSC packet: {other:?}"),
            },
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(
                    std::time::Instant::now() < deadline,
                    "expected {expected} OSC messages, received {messages:?}"
                );
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(error) => panic!("OSC loopback failed: {error}"),
        }
    }
    messages
}

#[test]
fn every_script_send_alias_preserves_canonical_arguments_through_native_output() {
    let socket = receiver();
    let (mut engine, module) = create_osc_module_with_output(socket.local_addr().unwrap().port());
    for method in ["sendMessage", "sendOSC", "sendOsc"] {
        engine.edits.push(Edit::CallNodeScriptMethod {
            node: module,
            call: golden_core::script::ScriptMethodCall::prepare(
                method,
                vec![
                    ParamValue::Str(format!("/{method}")),
                    ParamValue::Int(7),
                    ParamValue::Vec2(1.5, 2.5),
                    ParamValue::Str("preserved".into()),
                ],
            )
            .unwrap(),
        });
    }
    engine.apply_edits().unwrap();
    let messages = packets(&mut engine, &socket, 3);
    for (message, method) in messages.iter().zip(["sendMessage", "sendOSC", "sendOsc"]) {
        assert_eq!(message.addr, format!("/{method}"));
        assert_eq!(
            message.args,
            [
                OscType::Int(7),
                OscType::Float(1.5),
                OscType::Float(2.5),
                OscType::String("preserved".into())
            ]
        );
    }
}

#[test]
fn tuple_feedback_coalesces_changed_children_into_one_latest_packet() {
    let socket = receiver();
    let (mut engine, module) = create_osc_module_with_output(socket.local_addr().unwrap().port());
    let values = find_path(&engine, module, "values").unwrap();
    // A detached tree gives the fixture the same single structural admission as
    // received multi-value materialization.
    let tree = golden_core::edit::NodeTree::new(Folder::new("tuple"))
        .with_child(golden_core::edit::NodeTree::new(Parameter::new(
            "value 1",
            ParamValue::Int(0),
            ParameterChangeCheck::ValueChange,
        )))
        .with_child(golden_core::edit::NodeTree::new(Parameter::new(
            "value 2",
            ParamValue::Int(0),
            ParameterChangeCheck::ValueChange,
        )));
    let mut ctx = golden_core::process_ctx::ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.add_child_tree(values, tree, None);
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    service_osc_module_turn(&mut engine);
    let tuple = find_path(&engine, values, "tuple").unwrap();
    let first = find_path(&engine, tuple, "value 1").unwrap();
    let second = find_path(&engine, tuple, "value 2").unwrap();
    let auto_feedback = find_path(&engine, module, "parameters/processing/auto_feedback").unwrap();
    set_param(&mut engine, auto_feedback, ParamValue::Bool(true));
    engine.apply_edits().unwrap();
    service_osc_module_turn(&mut engine);
    set_param(&mut engine, first, ParamValue::Int(11));
    set_param(&mut engine, second, ParamValue::Int(22));
    engine.apply_edits().unwrap();
    let messages = packets(&mut engine, &socket, 1);
    assert_eq!(messages[0].addr, "/tuple");
    assert_eq!(messages[0].args, [OscType::Int(11), OscType::Int(22)]);
    let deadline = std::time::Instant::now() + Duration::from_millis(100);
    while std::time::Instant::now() < deadline {
        service_osc_module_turn(&mut engine);
        let mut bytes = [0u8; 4096];
        assert!(
            matches!(socket.recv(&mut bytes), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock),
            "one tuple state change must not produce duplicate feedback"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}
