use std::{
    net::TcpListener,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use golden_core::{
    edit::Edit,
    node::{Folder, Node, NodeId, NodeMetaPatch},
    parameter::{ParamValue, Parameter, ParameterEventBehaviour},
    process_ctx::ExecutionPhase,
};

use super::TcpClientModule;

#[test]
fn tcp_module_root_enable_toggle_stops_and_restarts_transport() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("TCP test listener should bind");
    let port = listener
        .local_addr()
        .expect("TCP test listener should expose a port")
        .port();
    let (mut engine, module_id) = create_tcp_module();
    let remote_port_id = tcp_module(&engine, module_id).remote_port.id();

    set_param(&mut engine, remote_port_id, ParamValue::Int(i32::from(port)));
    settle_transport_state(&mut engine);

    let module = tcp_module(&engine, module_id);
    assert!(
        module.transport.is_some(),
        "TCP module should start a transport while enabled"
    );
    assert!(
        module.last_transport_config.is_some(),
        "TCP module should retain its transport config while enabled"
    );

    set_node_enabled(&mut engine, module_id, false);
    settle_transport_state(&mut engine);

    let module = tcp_module(&engine, module_id);
    assert!(
        module.transport.is_none(),
        "TCP module should stop its transport when disabled"
    );
    assert!(
        module.last_transport_config.is_none(),
        "TCP module should clear cached transport config while disabled"
    );

    set_node_enabled(&mut engine, module_id, true);
    settle_transport_state(&mut engine);

    let module = tcp_module(&engine, module_id);
    assert!(
        module.transport.is_some(),
        "TCP module should restart its transport when re-enabled"
    );
    assert!(
        module.last_transport_config.is_some(),
        "TCP module should restore transport config after re-enable"
    );

    drop(listener);
}

#[test]
fn tcp_module_recovers_when_server_appears_and_after_connection_loss() {
    let port = free_tcp_port();
    let (mut engine, module_id) = create_tcp_module();
    let remote_port_id = tcp_module(&engine, module_id).remote_port.id();

    set_param(&mut engine, remote_port_id, ParamValue::Int(i32::from(port)));
    settle_transport_state(&mut engine);
    wait_for_transport_io();
    settle_transport_state(&mut engine);

    let module = tcp_module(&engine, module_id);
    assert!(
        module.transport.is_some(),
        "TCP module should keep a transport worker alive while reconnecting"
    );
    assert_eq!(
        connected_value(&engine, module_id),
        Some(false),
        "TCP module should report disconnected while the server is unavailable"
    );

    let listener = TcpListener::bind(("127.0.0.1", port)).expect("TCP test listener should bind when server appears");
    let (stage_tx, stage_rx) = mpsc::channel();
    let (drop_first_tx, drop_first_rx) = mpsc::channel();
    let (finish_tx, finish_rx) = mpsc::channel();

    let accept_thread = thread::spawn(move || {
        let (first_stream, _) = listener.accept().expect("first TCP client connection should arrive");
        stage_tx.send(1u8).expect("first TCP connection stage should send");
        drop_first_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("test should signal the first TCP connection to close");
        drop(first_stream);

        let (second_stream, _) = listener.accept().expect("TCP client should reconnect after disconnect");
        stage_tx.send(2u8).expect("second TCP connection stage should send");
        finish_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("test should release the second TCP connection");
        drop(second_stream);
    });

    assert_eq!(
        stage_rx.recv_timeout(Duration::from_secs(2)).expect("TCP client should connect once the server appears"),
        1
    );
    wait_for_connected(&mut engine, module_id, true);
    assert_eq!(
        connected_value(&engine, module_id),
        Some(true),
        "TCP module should report connected after the server appears"
    );

    drop_first_tx
        .send(())
        .expect("test should signal the first TCP connection to close");
    assert_eq!(
        stage_rx.recv_timeout(Duration::from_secs(3)).expect("TCP client should reconnect after the connection drops"),
        2
    );
    wait_for_connected(&mut engine, module_id, true);
    assert_eq!(
        connected_value(&engine, module_id),
        Some(true),
        "TCP module should report connected after reconnecting"
    );

    finish_tx
        .send(())
        .expect("test should release the second TCP connection");
    accept_thread.join().expect("TCP accept thread should exit cleanly");
}

fn create_tcp_module() -> (crate::app::AppEngine, NodeId) {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(TcpClientModule::create().into(), None);
    engine.apply_edits().expect("TCP module should attach");
    for _ in 0..4 {
        engine.apply_edits().expect("TCP defaults should materialize");
    }
    engine.resolve().expect("TCP runtime schedule should resolve");

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("TCP module should be attached under root");

    (engine, module_id)
}

fn tcp_module(engine: &crate::app::AppEngine, module_id: NodeId) -> &TcpClientModule {
    let crate::app::AppNode::TcpClientModule(module) = engine.nodes.get(module_id).expect("TCP module should exist")
    else {
        panic!("expected TcpClientModule node");
    };

    module
}

fn set_node_enabled(engine: &mut crate::app::AppEngine, node: NodeId, enabled: bool) {
    engine.edits.push(Edit::PatchMeta {
        node,
        patch: NodeMetaPatch {
            enabled: Some(enabled),
            ..Default::default()
        },
    });
}

fn set_param(engine: &mut crate::app::AppEngine, node: NodeId, value: ParamValue) {
    engine.edits.push(Edit::SetParam {
        node,
        value,
        behaviour: ParameterEventBehaviour::Coalesce,
    });
}

fn settle_transport_state(engine: &mut crate::app::AppEngine) {
    engine.apply_edits().expect("pending TCP edits should apply");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("pending TCP edits should dispatch");
    engine.apply_edits().expect("TCP event reactions should apply");
    engine
        .run_tick(Duration::from_millis(20))
        .expect("TCP transport tick should succeed");
    engine.apply_edits().expect("TCP transport edits should apply");
}

fn wait_for_transport_io() {
    thread::sleep(Duration::from_millis(40));
}

fn wait_for_connected(engine: &mut crate::app::AppEngine, module_id: NodeId, expected: bool) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        settle_transport_state(engine);
        if connected_value(engine, module_id) == Some(expected) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "TCP connected state should become {expected} before the timeout"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn free_tcp_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("test helper should bind an ephemeral TCP port")
        .local_addr()
        .expect("test helper should expose its ephemeral port")
        .port()
}

fn connection_folder_id(engine: &crate::app::AppEngine, module_id: NodeId) -> Option<NodeId> {
    find_child_by_key(engine, module_id, "connection")
}

fn connected_value(engine: &crate::app::AppEngine, module_id: NodeId) -> Option<bool> {
    let connection_id = connection_folder_id(engine, module_id)?;
    let param_id = find_child_by_key(engine, connection_id, "connected")?;
    parameter_node(engine, param_id).get().as_bool()
}

fn parameter_node(engine: &crate::app::AppEngine, node_id: NodeId) -> &Parameter {
    let crate::app::AppNode::Parameter(parameter) = engine
        .nodes
        .get(node_id)
        .expect("parameter node should exist")
    else {
        panic!("expected Parameter node");
    };

    parameter
}

fn find_child_by_key(engine: &crate::app::AppEngine, parent: NodeId, key: &str) -> Option<NodeId> {
    let mut current = engine.nodes.get(parent)?.node_data().first_child;
    while let Some(child_id) = current {
        let child = engine.nodes.get(child_id)?;
        if node_key_matches(child.node_data(), key) {
            return Some(child_id);
        }
        current = child.node_data().next_sibling;
    }
    None
}

fn node_key_matches(node: &golden_core::node::NodeData, key: &str) -> bool {
    node.meta.decl_id.0 == key
        || node.meta.decl_id.0.rsplit('/').next() == Some(key)
        || node.meta.short_name == key
        || node.meta.label == key
}
