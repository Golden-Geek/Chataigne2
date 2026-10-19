use super::super::WebSocketServerModule;
use super::*;
use golden_core::{
    edit::Edit,
    node::{Folder, Node, NodeId, NodeMetaPatch},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::ExecutionPhase,
};
use std::net::TcpStream;
use tokio_tungstenite::tungstenite::{client, Error, Message, WebSocket};

fn module(engine: &crate::app::AppEngine, id: NodeId) -> &WebSocketServerModule {
    let crate::app::AppNode::WebSocketServerModule(module) = engine.nodes.get(id).unwrap() else {
        panic!("WebSocket server")
    };
    module
}
fn set(engine: &mut crate::app::AppEngine, node: NodeId, value: ParamValue) {
    engine.edits.push(Edit::SetParam {
        node,
        value,
        behaviour: ParameterEventBehaviour::Append,
    });
}
fn enable(engine: &mut crate::app::AppEngine, node: NodeId, enabled: bool) {
    engine.edits.push(Edit::PatchMeta {
        node,
        patch: NodeMetaPatch {
            enabled: Some(enabled),
            ..Default::default()
        },
    });
}
fn wait(engine: &mut crate::app::AppEngine, mut condition: impl FnMut(&crate::app::AppEngine) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
        engine.apply_edits().unwrap();
        engine.run_tick(Duration::from_millis(10)).unwrap();
        engine.apply_edits().unwrap();
        if condition(engine) {
            return;
        }
        assert!(Instant::now() < deadline, "WebSocket server owner did not settle");
        thread::sleep(Duration::from_millis(1));
    }
}
fn connected(engine: &crate::app::AppEngine, id: NodeId) -> bool {
    let snapshot = engine.process_tree_snapshot();
    let connection = snapshot.find_child_by_decl_id(id, "connection").unwrap();
    let connected = snapshot.find_child_by_decl_id(connection, "connected").unwrap();
    snapshot.node(connected).unwrap().param_value.as_ref() == Some(&ParamValue::Bool(true))
}
fn configured(port: u16) -> (crate::app::AppEngine, NodeId) {
    let mut engine = crate::app::AppEngine::new(Folder::new("root").into());
    let mut node = WebSocketServerModule::create();
    node.node_data_mut().meta.enabled = false;
    engine.add_node(node.into(), None);
    for _ in 0..5 {
        engine.apply_edits().unwrap();
    }
    engine.resolve().unwrap();
    let id = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    let port_node = module(&engine, id).port.id();
    set(&mut engine, port_node, ParamValue::Int(port.into()));
    enable(&mut engine, id, true);
    engine.apply_edits().unwrap();
    (engine, id)
}
fn connect(engine: &mut crate::app::AppEngine, id: NodeId, port: u16, path: &str) -> WebSocket<TcpStream> {
    wait(engine, |engine| {
        connected(engine, id) && !module(engine, id).transport_dirty
    });
    let stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let socket = client(format!("ws://127.0.0.1:{port}{path}"), stream).unwrap().0;
    wait(engine, |engine| !module(engine, id).client_infos.is_empty());
    socket
}
fn send(engine: &mut crate::app::AppEngine, node: NodeId, text: &str) {
    engine.edits.push(Edit::CallNodeScriptMethod {
        node,
        method: "sendText".into(),
        args: vec![ParamValue::Str(text.into())],
    });
}
fn no_stale_frame(socket: &mut WebSocket<TcpStream>) {
    socket
        .get_mut()
        .set_read_timeout(Some(Duration::from_millis(50)))
        .unwrap();
    match socket.read() {
        Err(Error::Io(error))
            if matches!(
                error.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            ) => {}
        Err(_) | Ok(Message::Close(_)) => {}
        other => panic!("unadmitted frame reached old peer: {other:?}"),
    }
}
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

#[test]
fn applied_port_path_and_disable_edits_fence_script_admission_before_callbacks() {
    let first_port = free_port();
    let (mut engine, id) = configured(first_port);
    let mut first = connect(&mut engine, id, first_port, "/");
    send(&mut engine, id, "valid");
    engine.apply_edits().unwrap();
    assert_eq!(first.read().unwrap(), Message::text("valid"));
    let port_node = module(&engine, id).port.id();
    let second_port = free_port();
    set(&mut engine, port_node, ParamValue::Int(second_port.into()));
    send(&mut engine, id, "stale");
    assert!(engine.apply_edits().is_err());
    no_stale_frame(&mut first);
    let mut second = connect(&mut engine, id, second_port, "/");
    let path_node = module(&engine, id).path.id();
    set(&mut engine, path_node, ParamValue::Str("/new".into()));
    send(&mut engine, id, "old path");
    assert!(engine.apply_edits().is_err());
    no_stale_frame(&mut second);
    let mut third = connect(&mut engine, id, second_port, "/new");
    send(&mut engine, id, "new");
    engine.apply_edits().unwrap();
    assert_eq!(third.read().unwrap(), Message::text("new"));
    enable(&mut engine, id, false);
    send(&mut engine, id, "disabled");
    assert!(engine.apply_edits().is_err());
    no_stale_frame(&mut third);
}

#[test]
fn occupied_bind_is_disconnected_until_native_recovery_without_an_owner_edit() {
    let occupied = TcpListener::bind("0.0.0.0:0").unwrap();
    let port = occupied.local_addr().unwrap().port();
    let (mut engine, id) = configured(port);
    let port_node = module(&engine, id).port.id();
    wait(&mut engine, |engine| {
        engine
            .nodes
            .get(port_node)
            .unwrap()
            .node_data()
            .meta
            .presentation
            .warning(Some(super::super::WEBSOCKET_SERVER_PORT_WARNING_ID))
            .is_some()
    });
    assert!(!connected(&engine, id));
    drop(occupied);
    let _socket = connect(&mut engine, id, port, "/");
    assert!(connected(&engine, id));
}

#[test]
fn native_capacity_retry_defers_owner_work_and_snapshots_until_its_deadline() {
    let mut module = WebSocketServerModule::create();
    module.client_list_dirty = false;
    module.transport_dirty = true;
    module.retry_at = Some(Instant::now() + Duration::from_secs(1));
    assert!(!module.needs_update());
    assert!(!module.update_requires_tree_snapshot());
    module.retry_at = Some(Instant::now() - Duration::from_millis(1));
    assert!(module.needs_update());
    assert!(module.update_requires_tree_snapshot());
}
