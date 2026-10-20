use super::*;

fn receiving() -> (crate::app::AppEngine, NodeId, std::net::SocketAddr) {
    let (mut engine, module) = create_osc_module_with_output(9);
    let input = find_path(&engine, module, "connection/input").unwrap();
    let port = find_path(&engine, input, "port").unwrap();
    let receiver_port = available_udp_port();
    set_param(&mut engine, port, ParamValue::Int(i32::from(receiver_port)));
    set_node_enabled(&mut engine, input, true);
    settle_osc_module_state(&mut engine);
    (engine, module, ([127, 0, 0, 1], receiver_port).into())
}

fn send(address: std::net::SocketAddr, packets: Vec<rosc::OscPacket>) {
    let packet = OscPacket::Bundle(rosc::OscBundle {
        timetag: (0, 1).into(),
        content: packets,
    });
    let bytes = rosc::encoder::encode(&packet).unwrap();
    UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .send_to(&bytes, address)
        .unwrap();
}

fn message(address: &str, args: Vec<OscType>) -> OscPacket {
    OscPacket::Message(rosc::OscMessage {
        addr: address.into(),
        args,
    })
}

fn finish_input(engine: &mut crate::app::AppEngine, module: NodeId, path: &str) {
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        service_osc_module_turn(engine);
        let crate::app::AppNode::GenericOscModule(node) = engine.nodes.get(module).unwrap() else {
            unreachable!()
        };
        if find_path(engine, module, path).is_some() && !node.has_pending_incoming_messages_for_test() {
            return;
        }
        assert!(std::time::Instant::now() < deadline, "OSC input did not finish: {path}");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn incoming_message_auto_adds_value_nodes_under_values_folder() {
    let (mut engine, module, address) = receiving();
    send(address, vec![message("/foo", vec![OscType::Int(42)])]);
    finish_input(&mut engine, module, "values/foo");
    let crate::app::AppNode::GenericOscModule(node) = engine.nodes.get(module).unwrap() else {
        unreachable!()
    };
    assert!(node.auto_add_enabled_for_test());
    let value = find_path(&engine, module, "values/foo").unwrap();
    assert_eq!(param_value(&engine, value), ParamValue::Int(42));
    assert_eq!(
        engine.nodes.get(value).unwrap().node_data().meta.user_permissions,
        NodeUserPermissions::all()
    );
}

#[test]
fn incoming_multi_message_auto_adds_missing_path_with_batched_trees() {
    let (mut engine, module, address) = receiving();
    send(
        address,
        vec![message(
            "/rig/arm/pose",
            vec![OscType::Int(1), OscType::Int(2), OscType::Int(3)],
        )],
    );
    finish_input(&mut engine, module, "values/rig/arm/pose/value 3");
    for index in 1..=3 {
        let value = find_path(&engine, module, &format!("values/rig/arm/pose/value {index}")).unwrap();
        assert_eq!(param_value(&engine, value), ParamValue::Int(index));
    }
}

#[test]
fn incoming_float_vector_cast_stays_one_parameter() {
    let (mut engine, module, address) = receiving();
    send(
        address,
        vec![message(
            "/pose",
            vec![OscType::Float(1.0), OscType::Float(2.0), OscType::Float(3.0)],
        )],
    );
    finish_input(&mut engine, module, "values/pose");
    let pose = find_path(&engine, module, "values/pose").unwrap();
    assert_eq!(param_value(&engine, pose), ParamValue::Vec3(1.0, 2.0, 3.0));
    assert!(find_path(&engine, pose, "value 1").is_none());
}

#[test]
fn incoming_messages_preserve_shared_missing_paths_across_bounded_turns() {
    let (mut engine, module, address) = receiving();
    send(
        address,
        vec![
            message("/rig/arm/x", vec![OscType::Float(1.25)]),
            message("/rig/arm/y", vec![OscType::Float(2.5)]),
        ],
    );
    finish_input(&mut engine, module, "values/rig/arm/y");
    let x = find_path(&engine, module, "values/rig/arm/x").unwrap();
    let y = find_path(&engine, module, "values/rig/arm/y").unwrap();
    assert_eq!(param_value(&engine, x), ParamValue::Float(1.25));
    assert_eq!(param_value(&engine, y), ParamValue::Float(2.5));
}

#[test]
fn incoming_values_do_not_echo_but_later_user_edits_still_send_feedback() {
    let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
    receiver.set_nonblocking(true).unwrap();
    let (mut engine, module, address) = receiving();
    let outputs = find_path(&engine, module, "connection/outputs").unwrap();
    let output = engine.nodes.get(outputs).unwrap().node_data().first_child.unwrap();
    let port = find_path(&engine, output, "remote_port").unwrap();
    set_param(
        &mut engine,
        port,
        ParamValue::Int(i32::from(receiver.local_addr().unwrap().port())),
    );
    let feedback = find_path(&engine, module, "parameters/processing/auto_feedback").unwrap();
    set_param(&mut engine, feedback, ParamValue::Bool(true));
    service_osc_module_turn(&mut engine);
    send(address, vec![message("/gain", vec![OscType::Int(42)])]);
    finish_input(&mut engine, module, "values/gain");
    let mut bytes = [0u8; 4096];
    assert!(matches!(receiver.recv(&mut bytes), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock));
    let gain = find_path(&engine, module, "values/gain").unwrap();
    // The second input writes an existing parameter, exercising echo suppression.
    send(
        address,
        vec![
            message("/gain", vec![OscType::Int(43)]),
            message("/gain", vec![OscType::Int(44)]),
        ],
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        service_osc_module_turn(&mut engine);
        let crate::app::AppNode::GenericOscModule(node) = engine.nodes.get(module).unwrap() else {
            unreachable!()
        };
        if param_value(&engine, gain) == ParamValue::Int(44) && !node.has_pending_incoming_messages_for_test() {
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(matches!(receiver.recv(&mut bytes), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock));
    set_param(&mut engine, gain, ParamValue::Int(45));
    loop {
        service_osc_module_turn(&mut engine);
        if let Ok(length) = receiver.recv(&mut bytes) {
            assert!(
                matches!(decoder::decode_udp(&bytes[..length]).unwrap().1, OscPacket::Message(message)
                if message.addr == "/gain" && message.args == [OscType::Int(45)])
            );
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}
