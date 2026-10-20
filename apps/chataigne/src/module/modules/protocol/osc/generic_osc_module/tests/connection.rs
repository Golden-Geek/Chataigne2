use super::*;

#[test]
fn osc_module_connection_indicators_follow_receiver_and_outputs() {
    let receiver = UdpSocket::bind("127.0.0.1:0").expect("test receiver should bind");
    let output_port = receiver
        .local_addr()
        .expect("test receiver should expose a local address")
        .port();
    let (mut engine, module_id) = create_osc_module_with_output(output_port);

    let connected_param =
        find_path(&engine, module_id, "connection/connected").expect("module connection state parameter should exist");
    let can_receive_param = find_path(&engine, module_id, "connection/can_receive")
        .expect("module incoming capability parameter should exist");
    let can_send_param = find_path(&engine, module_id, "connection/can_send")
        .expect("module outgoing capability parameter should exist");
    let receiver_folder = find_path(&engine, module_id, "connection/input").expect("receiver folder should exist");
    let receiver_port_param =
        find_path(&engine, module_id, "connection/input/port").expect("receiver port param should exist");
    let outputs_id = find_path(&engine, module_id, "connection/outputs").expect("outputs folder should exist");

    assert!(
        engine
            .nodes
            .get(outputs_id)
            .expect("outputs node should exist")
            .node_data()
            .meta
            .can_be_disabled,
        "OSC Outputs should expose an enabled toggle"
    );

    assert_bool_param(
        &engine,
        connected_param,
        true,
        "output-only OSC transport should be connected",
    );
    assert_bool_param(
        &engine,
        can_receive_param,
        false,
        "disabled Receiver should hide the incoming indicator",
    );
    assert_bool_param(
        &engine,
        can_send_param,
        true,
        "enabled Outputs should show the outgoing indicator",
    );

    set_node_enabled(&mut engine, outputs_id, false);
    settle_osc_module_state(&mut engine);
    assert_bool_param(
        &engine,
        connected_param,
        false,
        "OSC module with Receiver and Outputs disabled should not report connected",
    );
    assert_bool_param(
        &engine,
        can_receive_param,
        false,
        "disabled Receiver should still hide the incoming indicator",
    );
    assert_bool_param(
        &engine,
        can_send_param,
        false,
        "disabled Outputs should hide the outgoing indicator",
    );

    set_param(
        &mut engine,
        receiver_port_param,
        ParamValue::Int(i32::from(available_udp_port())),
    );
    set_node_enabled(&mut engine, receiver_folder, true);
    settle_osc_module_state(&mut engine);
    assert_bool_param(
        &engine,
        connected_param,
        true,
        "OSC receiver should report connected after binding",
    );
    assert_bool_param(
        &engine,
        can_receive_param,
        true,
        "enabled Receiver should show the incoming indicator",
    );
    assert_bool_param(
        &engine,
        can_send_param,
        false,
        "disabled Outputs should still hide the outgoing indicator",
    );

    set_node_enabled(&mut engine, outputs_id, true);
    settle_osc_module_state(&mut engine);
    assert_bool_param(
        &engine,
        can_receive_param,
        true,
        "enabled Receiver should keep the incoming indicator visible",
    );
    assert_bool_param(
        &engine,
        can_send_param,
        true,
        "enabled Outputs should show the outgoing indicator again",
    );
}

#[test]
fn osc_module_root_enable_toggle_stops_and_restarts_transport() {
    let receiver = UdpSocket::bind("127.0.0.1:0").expect("test receiver should bind");
    let output_port = receiver
        .local_addr()
        .expect("test receiver should expose a local address")
        .port();
    let (mut engine, module_id) = create_osc_module_with_output(output_port);

    let connected_param =
        find_path(&engine, module_id, "connection/connected").expect("module connection state parameter should exist");
    assert_bool_param(
        &engine,
        connected_param,
        true,
        "OSC module should report connected while enabled",
    );

    set_node_enabled(&mut engine, module_id, false);
    settle_osc_module_state(&mut engine);
    assert_bool_param(
        &engine,
        connected_param,
        false,
        "OSC module should disconnect as soon as its root node is disabled",
    );

    set_node_enabled(&mut engine, module_id, true);
    settle_osc_module_state(&mut engine);
    assert_bool_param(
        &engine,
        connected_param,
        true,
        "OSC module should reconnect after its root node is re-enabled",
    );
}

#[test]
fn failed_native_bind_retries_and_recovers_after_the_port_is_released() {
    // Reserve the same wildcard address: Windows permits a wildcard socket to
    // coexist with a specific loopback binding on the same port.
    let busy = UdpSocket::bind("0.0.0.0:0").unwrap();
    let port = busy.local_addr().unwrap().port();
    let output = UdpSocket::bind("127.0.0.1:0").unwrap();
    let (mut engine, module) = create_osc_module_with_output(output.local_addr().unwrap().port());
    let receiver = find_path(&engine, module, "connection/input").unwrap();
    let input_port = find_path(&engine, module, "connection/input/port").unwrap();
    let connected = find_path(&engine, module, "connection/connected").unwrap();
    set_param(&mut engine, input_port, ParamValue::Int(i32::from(port)));
    set_node_enabled(&mut engine, receiver, true);
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        service_osc_module_turn(&mut engine);
        if matches!(engine.nodes.get(module), Some(AppNode::GenericOscModule(module)) if module.base.transport_retrying_for_test())
        {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "port={:?}, receiver={}, {}",
            param_value(&engine, input_port),
            engine.nodes.get(receiver).unwrap().node_data().meta.enabled,
            match engine.nodes.get(module).unwrap() {
                AppNode::GenericOscModule(module) => module.base.transport_state_for_test(),
                _ => unreachable!(),
            }
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(!bool_param_value(&engine, connected));
    drop(busy);
    settle_osc_module_state(&mut engine);
    assert!(bool_param_value(&engine, connected));
}
