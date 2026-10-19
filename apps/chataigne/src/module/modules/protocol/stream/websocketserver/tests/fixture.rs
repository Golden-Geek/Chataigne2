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

pub(super) fn module(engine: &crate::app::AppEngine, id: NodeId) -> &WebSocketServerModule {
    let crate::app::AppNode::WebSocketServerModule(module) = engine.nodes.get(id).unwrap() else {
        panic!("WebSocket server")
    };
    module
}
pub(super) fn set(engine: &mut crate::app::AppEngine, node: NodeId, value: ParamValue) {
    engine.edits.push(Edit::SetParam {
        node,
        value,
        behaviour: ParameterEventBehaviour::Append,
    });
}
pub(super) fn enable(engine: &mut crate::app::AppEngine, node: NodeId, enabled: bool) {
    engine.edits.push(Edit::PatchMeta {
        node,
        patch: NodeMetaPatch {
            enabled: Some(enabled),
            ..Default::default()
        },
    });
}
pub(super) fn wait(engine: &mut crate::app::AppEngine, mut condition: impl FnMut(&crate::app::AppEngine) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
        engine.apply_edits().unwrap();
        engine.run_tick(Duration::from_millis(10)).unwrap();
        engine.apply_edits().unwrap();
        assert_eq!(engine.tick_stats().callbacks_fired, 0);
        if condition(engine) {
            return;
        }
        assert!(Instant::now() < deadline, "WebSocket server owner did not settle");
        thread::sleep(Duration::from_millis(1));
    }
}
pub(super) fn connected(engine: &crate::app::AppEngine, id: NodeId) -> bool {
    let snapshot = engine.process_tree_snapshot();
    let connection = snapshot.find_child_by_decl_id(id, "connection").unwrap();
    let connected = snapshot.find_child_by_decl_id(connection, "connected").unwrap();
    snapshot.node(connected).unwrap().param_value.as_ref() == Some(&ParamValue::Bool(true))
}
pub(super) fn configured(port: u16) -> (crate::app::AppEngine, NodeId) {
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
pub(super) fn connect(engine: &mut crate::app::AppEngine, id: NodeId, port: u16, path: &str) -> WebSocket<TcpStream> {
    wait(engine, |engine| {
        connected(engine, id) && !module(engine, id).transport_dirty
    });
    let stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let socket = client(format!("ws://127.0.0.1:{port}{path}"), stream).unwrap().0;
    wait(engine, |engine| !module(engine, id).client_infos.is_empty());
    socket
}
pub(super) fn send(engine: &mut crate::app::AppEngine, node: NodeId, text: &str) {
    engine.edits.push(Edit::CallNodeScriptMethod {
        node,
        call: golden_core::script::ScriptMethodCall::prepare("sendText", vec![ParamValue::Str(text.into())]).unwrap(),
    });
}
pub(super) fn no_stale_frame(socket: &mut WebSocket<TcpStream>) {
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
pub(super) fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}
