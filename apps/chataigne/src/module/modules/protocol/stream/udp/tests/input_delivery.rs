use super::*;
use golden_core::events::EventKind;
use std::{net::UdpSocket, time::Instant};

fn callbacks(engine: &crate::app::AppEngine, module: NodeId) -> Vec<String> {
    engine
        .ui_event_log()
        .iter()
        .filter_map(|event| {
            if let EventKind::Custom(custom) = &event.kind {
                let name = custom.payload.get("callback")?.as_str()?;
                if custom.origin == Some(module) && ["dataReceived", "dataReceive", "textReceived"].contains(&name) {
                    return Some(name.to_string());
                }
            }
            None
        })
        .collect()
}

#[test]
fn received_bytes_wake_the_passive_node_and_apply_values_with_exact_callbacks() {
    let (mut engine, module) = create_udp_module();
    let address = udp_module(&engine, module).transports.bound_address().unwrap();
    let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
    sender.send_to(b"level 7\n", ("127.0.0.1", address.port())).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    let values = child_by_decl(&engine, module, "values");
    loop {
        engine.run_tick(Duration::from_millis(10)).unwrap();
        assert_eq!(engine.tick_stats().callbacks_fired, 0);
        let snapshot = engine.process_tree_snapshot();
        let value = snapshot
            .find_child(values, "level")
            .and_then(|id| snapshot.node(id))
            .and_then(|node| node.param_value.clone());
        if value == Some(ParamValue::Int(7))
            && callbacks(&engine, module).len() == 3
            && !udp_module(&engine, module).stream.has_pending_messages()
            && udp_module(&engine, module).pending_inputs.is_empty()
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "notifications must carry incoming data without periodic node service"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(
        callbacks(&engine, module),
        ["dataReceived", "dataReceive", "textReceived"]
    );
    for _ in 0..10 {
        engine.run_tick(Duration::from_millis(10)).unwrap();
    }
    assert_eq!(callbacks(&engine, module).len(), 3);
    assert_eq!(engine.tick_stats().wake_callbacks_fired, 0);
    assert_eq!(engine.tick_stats().callbacks_fired, 0);
}

#[test]
fn root_processing_changes_update_the_worker_without_rebinding_the_socket() {
    let (mut engine, module) = create_udp_module();
    let address = udp_module(&engine, module).transports.bound_address().unwrap();
    let parameters = child_by_decl(&engine, module, "parameters");
    let processing = child_by_decl(&engine, parameters, "processing");
    let mode = child_by_decl(&engine, processing, "parse_mode");
    set_param(&mut engine, mode, ParamValue::Enum("json".into()));
    settle_transport_state(&mut engine);
    assert_eq!(udp_module(&engine, module).transports.bound_address(), Some(address));
    let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
    sender.send_to(br#"{"new":42}"#, ("127.0.0.1", address.port())).unwrap();
    let values = child_by_decl(&engine, module, "values");
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        engine.run_tick(Duration::from_millis(10)).unwrap();
        let snapshot = engine.process_tree_snapshot();
        if snapshot.find_child(values, "new").is_some() {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(udp_module(&engine, module).transports.bound_address(), Some(address));
}

#[test]
fn destroying_and_undoing_a_module_does_not_restore_queued_runtime_input() {
    use crate::app::module::common::received_values::{ReceivedValueMessage, ReceivedValuePayload};
    let (mut engine, module) = create_udp_module();
    let crate::app::AppNode::UdpModule(node) = engine.nodes.get_mut(module).unwrap() else {
        panic!("UDP");
    };
    node.stream.push_messages(
        vec![ReceivedValueMessage {
            path_segments: vec!["stale".into()],
            payload: ReceivedValuePayload::Single(ParamValue::Int(9)),
            source_description: "retirement fixture".into(),
        }]
        .try_into()
        .unwrap(),
    );
    assert!(node.stream.has_pending_messages());
    engine.edits.push(Edit::RemoveNode { node: module });
    engine.apply_edits().unwrap();
    assert!(engine.undo().unwrap());
    assert!(!udp_module(&engine, module).stream.has_pending_messages());
    for _ in 0..10 {
        engine.run_tick(Duration::from_millis(10)).unwrap();
    }
    let values = child_by_decl(&engine, module, "values");
    assert!(engine.process_tree_snapshot().find_child(values, "stale").is_none());
}

#[test]
fn failed_bind_recovers_from_its_deadline_without_periodic_node_updates() {
    let (mut engine, module) = create_udp_module();
    let blocker = UdpSocket::bind("0.0.0.0:0").unwrap();
    let port = blocker.local_addr().unwrap().port();
    let connection = child_by_decl(&engine, module, "connection");
    let input = child_by_decl(&engine, connection, "input");
    let input_port = child_by_decl(&engine, input, "port");
    set_param(&mut engine, input_port, ParamValue::Int(i32::from(port)));
    engine.apply_edits().unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while udp_module(&engine, module).transports.retry_in().is_none() {
        engine.run_tick(Duration::from_millis(10)).unwrap();
        assert_eq!(engine.tick_stats().callbacks_fired, 0);
        assert!(Instant::now() < deadline, "controlled bind failure must reach recovery");
        std::thread::sleep(Duration::from_millis(1));
    }
    drop(blocker);
    while !udp_module(&engine, module).transports.connected() {
        engine.run_tick(Duration::from_millis(10)).unwrap();
        assert_eq!(engine.tick_stats().callbacks_fired, 0);
        assert!(
            Instant::now() < deadline,
            "retry timer must wake the passive node after the port is released"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(
        udp_module(&engine, module).transports.bound_address().unwrap().port(),
        port
    );
}
