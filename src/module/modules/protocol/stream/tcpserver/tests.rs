use std::{
    io::Read,
    net::{TcpListener, TcpStream},
    thread,
    time::Duration,
};

use golden_core::{
    edit::Edit,
    node::{Folder, Node, NodeId, NodeMetaPatch},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::ExecutionPhase,
    script::ScriptSource,
};

use super::{
    transport::{TcpServerTransportConfig, TcpServerTransportHandle, TcpServerWorkerEvent},
    TcpServerModule,
};

#[test]
fn tcp_server_script_template_scaffolds_server_stream_callbacks() {
    let config = crate::app::module::script_api::module_script_config(TcpServerModule::NODE_TYPE);
    let ScriptSource::Inline(source) = config.source else {
        panic!("tcp server module script template should resolve to inline source");
    };

    assert!(source.contains("function textReceived"));
    assert!(source.contains("function clientConnected"));
    assert!(!source.contains("function noteOnReceived"));
}

#[test]
fn tcp_server_module_root_enable_toggle_stops_and_restarts_transport() {
    let port = free_tcp_port();
    let (mut engine, module_id) = create_tcp_server_module();
    let port_id = tcp_server_module(&engine, module_id).port.id();

    set_param(&mut engine, port_id, ParamValue::Int(i32::from(port)));
    settle_transport_state(&mut engine);

    let module = tcp_server_module(&engine, module_id);
    assert!(
        module.transport.is_some(),
        "TCP server module should start a transport while enabled"
    );
    assert!(
        module.last_transport_config.is_some(),
        "TCP server module should retain its transport config while enabled"
    );

    set_node_enabled(&mut engine, module_id, false);
    settle_transport_state(&mut engine);

    let module = tcp_server_module(&engine, module_id);
    assert!(
        module.transport.is_none(),
        "TCP server module should stop its transport when disabled"
    );
    assert!(
        module.last_transport_config.is_none(),
        "TCP server module should clear cached transport config while disabled"
    );

    set_node_enabled(&mut engine, module_id, true);
    settle_transport_state(&mut engine);

    let module = tcp_server_module(&engine, module_id);
    assert!(
        module.transport.is_some(),
        "TCP server module should restart its transport when re-enabled"
    );
    assert!(
        module.last_transport_config.is_some(),
        "TCP server module should restore transport config after re-enable"
    );
}

#[test]
fn tcp_server_module_tracks_live_clients_in_runtime_folder() {
    let port = free_tcp_port();
    let (mut engine, module_id) = create_tcp_server_module();
    let port_id = tcp_server_module(&engine, module_id).port.id();

    set_param(&mut engine, port_id, ParamValue::Int(i32::from(port)));
    settle_transport_state(&mut engine);

    let client = TcpStream::connect(("127.0.0.1", port)).expect("TCP client should connect to test server");
    wait_for_transport_io();
    settle_transport_state(&mut engine);

    assert_eq!(connected_clients_value(&engine, module_id), Some(1));

    let clients_id = clients_folder_id(&engine, module_id).expect("TCP server clients folder should exist");
    let client_ids = direct_children(&engine, clients_id);
    assert_eq!(client_ids.len(), 1, "TCP server clients folder should contain one connected client");

    let client_param = parameter_node(&engine, client_ids[0]);
    assert!(
        client_param.node_data().meta.label.contains("127.0.0.1"),
        "TCP server client info label should use the remote address"
    );
    let expected_info = format!(
        "Remote address: {}",
        client
            .local_addr()
            .expect("client should know its local address")
    );
    assert_eq!(
        client_param.get().as_str(),
        Some(expected_info)
    );
    assert!(client_param.read_only, "TCP server client info parameter should be read-only");

    drop(client);
    wait_for_transport_io();
    settle_transport_state(&mut engine);

    assert_eq!(connected_clients_value(&engine, module_id), Some(0));
    let clients_id = clients_folder_id(&engine, module_id).expect("TCP server clients folder should still exist");
    assert!(
        direct_children(&engine, clients_id).is_empty(),
        "TCP server clients folder should remove client info when the client disconnects"
    );
}

#[test]
fn tcp_server_transport_closes_client_connections_when_stopped() {
    let port = free_tcp_port();
    let mut transport = TcpServerTransportHandle::spawn(TcpServerTransportConfig {
        bind_host: "127.0.0.1".to_string(),
        bind_port: port,
        receive_enabled: true,
        send_enabled: true,
    })
    .expect("TCP server transport should bind for the shutdown test");

    let mut client = TcpStream::connect(("127.0.0.1", port)).expect("TCP client should connect to transport test server");
    client
        .set_read_timeout(Some(Duration::from_secs(1)))
        .expect("TCP shutdown test client should accept a read timeout");

    wait_for_client_connected(&transport);

    transport.stop();

    let mut buffer = [0u8; 1];
    match client.read(&mut buffer) {
        Ok(0) => {}
        Ok(length) => panic!("expected TCP shutdown to terminate the client stream, got {length} byte(s)"),
        Err(error) if client_disconnect_detected(&error) => {}
        Err(error) => panic!("expected TCP shutdown to terminate the client stream, got {error}"),
    }
}

fn create_tcp_server_module() -> (crate::app::AppEngine, NodeId) {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(TcpServerModule::create().into(), None);
    engine.apply_edits().expect("TCP server module should attach");
    for _ in 0..4 {
        engine.apply_edits().expect("TCP server defaults should materialize");
    }
    engine.resolve().expect("TCP server runtime schedule should resolve");

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("TCP server module should be attached under root");

    (engine, module_id)
}

fn tcp_server_module(engine: &crate::app::AppEngine, module_id: NodeId) -> &TcpServerModule {
    let crate::app::AppNode::TcpServerModule(module) = engine
        .nodes
        .get(module_id)
        .expect("TCP server module should exist")
    else {
        panic!("expected TcpServerModule node");
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
    engine.apply_edits().expect("pending TCP server edits should apply");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("pending TCP server edits should dispatch");
    engine.apply_edits().expect("TCP server event reactions should apply");
    engine
        .run_tick(Duration::from_millis(20))
        .expect("TCP server transport tick should succeed");
    engine.apply_edits().expect("TCP server transport edits should apply");
}

fn wait_for_transport_io() {
    thread::sleep(Duration::from_millis(25));
}

fn wait_for_client_connected(transport: &TcpServerTransportHandle) {
    for _ in 0..40 {
        match transport.try_recv() {
            Ok(TcpServerWorkerEvent::ClientConnected { .. }) => return,
            Ok(_) => {}
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                panic!("TCP server transport worker should stay alive while waiting for the client")
            }
        }
        wait_for_transport_io();
    }

    panic!("TCP server transport should report a connected client before shutdown")
}

fn client_disconnect_detected(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::NotConnected
            | std::io::ErrorKind::BrokenPipe
            | std::io::ErrorKind::UnexpectedEof
    ) || error.raw_os_error() == Some(10054)
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

fn clients_folder_id(engine: &crate::app::AppEngine, module_id: NodeId) -> Option<NodeId> {
    let connection_id = connection_folder_id(engine, module_id)?;
    find_child_by_key(engine, connection_id, "clients")
}

fn connected_clients_value(engine: &crate::app::AppEngine, module_id: NodeId) -> Option<i32> {
    let connection_id = connection_folder_id(engine, module_id)?;
    let param_id = find_child_by_key(engine, connection_id, "connected_clients")?;
    parameter_node(engine, param_id).get().as_int()
}

fn parameter_node(engine: &crate::app::AppEngine, node_id: NodeId) -> &golden_core::parameter::Parameter {
    let crate::app::AppNode::Parameter(parameter) = engine
        .nodes
        .get(node_id)
        .expect("parameter node should exist")
    else {
        panic!("expected Parameter node");
    };

    parameter
}

fn direct_children(engine: &crate::app::AppEngine, parent: NodeId) -> Vec<NodeId> {
    let mut children = Vec::new();
    let mut current = engine.nodes.get(parent).and_then(|node| node.node_data().first_child);
    while let Some(child_id) = current {
        let child = engine
            .nodes
            .get(child_id)
            .expect("child id from node links should exist");
        children.push(child_id);
        current = child.node_data().next_sibling;
    }
    children
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
