use super::*;

#[test]
fn steady_osc_module_has_no_scheduled_or_worker_polling_and_no_idle_snapshots() {
    let (mut engine, module) = create_osc_module_with_output(9);
    let AppNode::GenericOscModule(node) = engine.nodes.get_mut(module).unwrap() else {
        unreachable!()
    };
    node.base.fake_discovery_for_test();
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    let mut quiet = 0;
    while quiet < 10 {
        service_osc_module_turn(&mut engine);
        let stats = engine.tick_stats();
        quiet = if stats.wake_callbacks_fired == 0 && stats.snapshot_rebuilds == 0 {
            quiet + 1
        } else {
            0
        };
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    for _ in 0..100 {
        service_osc_module_turn(&mut engine);
        let stats = engine.tick_stats();
        assert_eq!(stats.callbacks_fired, 0, "OSC must not request scheduled updates");
        assert_eq!(
            stats.wake_callbacks_fired, 0,
            "unchanged discovery must not wake the actor"
        );
        assert_eq!(
            stats.snapshot_rebuilds, 0,
            "an idle OSC module must not request snapshots"
        );
    }
}

#[test]
fn disable_and_enable_restarts_passive_native_service_and_preserves_script_output() {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    socket.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let (mut engine, module) = create_osc_module_with_output(socket.local_addr().unwrap().port());
    set_node_enabled(&mut engine, module, false);
    service_osc_module_turn(&mut engine);
    set_node_enabled(&mut engine, module, true);
    settle_osc_module_state(&mut engine);
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: module,
        call: golden_core::script::ScriptMethodCall::prepare(
            "sendOSC",
            vec![ParamValue::Str("/recovered".into()), ParamValue::Int(7)],
        )
        .unwrap(),
    });
    engine.apply_edits().unwrap();
    let mut bytes = [0u8; 512];
    let length = socket.recv(&mut bytes).unwrap();
    assert!(
        matches!(decoder::decode_udp(&bytes[..length]).unwrap().1, OscPacket::Message(message)
        if message.addr == "/recovered" && message.args == [OscType::Int(7)])
    );
}
