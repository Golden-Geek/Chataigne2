use super::super::WebSocketClientModule;
use super::*;
use golden_core::{
    edit::Edit,
    engine::NodeExecutionRule,
    events::EventKind,
    node::{Folder, Node, NodeId, NodeMetaPatch},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::{ExecutionPhase, ProcessCtx},
};
use std::{net::TcpStream, sync::mpsc};
use tokio_tungstenite::tungstenite::{
    accept_hdr,
    handshake::server::{Callback, ErrorResponse, Request, Response},
    Message, WebSocket,
};

type Server = (u16, mpsc::Receiver<WebSocket<TcpStream>>, thread::JoinHandle<()>);
struct ExpectedPath(&'static str);
impl Callback for ExpectedPath {
    fn on_request(self, request: &Request, response: Response) -> Result<Response, ErrorResponse> {
        assert_eq!(request.uri().path(), self.0);
        Ok(response)
    }
}
fn server(path: &'static str) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::channel();
    let task = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        let peer = loop {
            match listener.accept() {
                Ok((peer, _)) => break peer,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "WebSocket app fixture did not connect");
                    thread::sleep(Duration::from_millis(1));
                }
                Err(error) => panic!("{error}"),
            }
        };
        peer.set_nonblocking(false).unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(4))).unwrap();
        peer.set_write_timeout(Some(Duration::from_secs(4))).unwrap();
        let peer = accept_hdr(peer, ExpectedPath(path)).unwrap();
        tx.send(peer).unwrap();
    });
    (port, rx, task)
}
fn module(engine: &crate::app::AppEngine, id: NodeId) -> &WebSocketClientModule {
    let crate::app::AppNode::WebSocketClientModule(module) = engine.nodes.get(id).unwrap() else {
        panic!("WebSocket client")
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
fn tick(engine: &mut crate::app::AppEngine) {
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine.apply_edits().unwrap();
    engine.run_tick(Duration::from_millis(10)).unwrap();
    engine.apply_edits().unwrap();
    assert_eq!(engine.tick_stats().callbacks_fired, 0);
}
fn wait(engine: &mut crate::app::AppEngine, mut condition: impl FnMut(&crate::app::AppEngine) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        tick(engine);
        if condition(engine) {
            return;
        }
        assert!(Instant::now() < deadline, "passive WebSocket owner did not settle");
        thread::sleep(Duration::from_millis(1));
    }
}
fn path(engine: &crate::app::AppEngine, parent: NodeId, name: &str) -> NodeId {
    let snapshot = engine.process_tree_snapshot();
    name.split('/').fold(parent, |parent, segment| {
        snapshot
            .find_child_by_decl_id(parent, segment)
            .or_else(|| snapshot.find_child(parent, segment))
            .unwrap()
    })
}
fn value(engine: &crate::app::AppEngine, id: NodeId, name: &str) -> Option<ParamValue> {
    let snapshot = engine.process_tree_snapshot();
    let values = snapshot.find_child_by_decl_id(id, "values")?;
    snapshot
        .find_child(values, name)
        .and_then(|id| snapshot.node(id))
        .and_then(|node| node.param_value.clone())
}
fn connected(engine: &crate::app::AppEngine, id: NodeId) -> bool {
    let connected = path(engine, id, "connection/connected");
    engine
        .process_tree_snapshot()
        .node(connected)
        .unwrap()
        .param_value
        .as_ref()
        == Some(&ParamValue::Bool(true))
}
fn idle(engine: &crate::app::AppEngine, id: NodeId) -> bool {
    let node = module(engine, id);
    !node.transport_dirty
        && node.retiring_transport.is_none()
        && node.pending_inputs.is_empty()
        && !node.stream.has_pending_messages()
        && !node.transport.as_ref().is_some_and(|transport| transport.has_pending())
}
fn configured() -> (crate::app::AppEngine, NodeId, WebSocket<TcpStream>) {
    let (port, peer, task) = server("/fixture");
    let mut engine = crate::app::AppEngine::new(Folder::new("root").into());
    let mut node = WebSocketClientModule::create();
    node.node_data_mut().meta.enabled = false;
    engine.add_node(node.into(), None);
    for _ in 0..5 {
        engine.apply_edits().unwrap();
    }
    engine.resolve().unwrap();
    let id = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    assert!(module(&engine, id).transport.is_none());
    let port_node = module(&engine, id).remote_port.id();
    let path_node = module(&engine, id).path.id();
    set(&mut engine, port_node, ParamValue::Int(port.into()));
    set(&mut engine, path_node, ParamValue::Str("/fixture".into()));
    engine.edits.push(Edit::PatchMeta {
        node: id,
        patch: NodeMetaPatch {
            enabled: Some(true),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    wait(&mut engine, |engine| connected(engine, id) && idle(engine, id));
    let peer = peer.recv_timeout(Duration::from_secs(2)).unwrap();
    task.join().unwrap();
    (engine, id, peer)
}
fn send(engine: &mut crate::app::AppEngine, id: NodeId, text: &str) {
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: id,
        call: golden_core::script::ScriptMethodCall::prepare("sendText", vec![ParamValue::Str(text.into())]).unwrap(),
    });
}
fn callbacks(engine: &crate::app::AppEngine, id: NodeId) -> Vec<String> {
    engine
        .ui_event_log()
        .iter()
        .filter_map(|event| {
            let EventKind::Custom(event) = &event.kind else {
                return None;
            };
            let name = event.payload.get("callback")?.as_str()?;
            (event.origin == Some(id) && ["dataReceived", "dataReceive", "textReceived"].contains(&name))
                .then(|| name.to_string())
        })
        .collect()
}
fn admit_prefix(engine: &crate::app::AppEngine, id: NodeId) -> Vec<u8> {
    let native = module(engine, id).transport.as_ref().unwrap();
    let mut bytes = Vec::new();
    for index in 0..1000 {
        let byte = index as u8;
        match native.send(
            crate::app::module::common::streaming::commands::StreamingSendFrameKind::Binary,
            &[byte],
            false,
        ) {
            Ok(()) => bytes.push(byte),
            Err(error) => {
                assert!(error.contains("capacity exhausted"));
                break;
            }
        }
    }
    assert!(!bytes.is_empty());
    bytes
}
fn read_prefix(peer: &mut WebSocket<TcpStream>) -> Vec<u8> {
    let mut bytes = Vec::new();
    loop {
        match peer.read().unwrap() {
            Message::Binary(payload) => bytes.extend_from_slice(&payload),
            Message::Close(_) => return bytes,
            frame => panic!("unadmitted frame: {frame:?}"),
        }
    }
}

#[test]
fn passive_text_and_binary_input_preserve_callbacks_and_then_leave_no_periodic_work() {
    let (mut engine, id, mut peer) = configured();
    assert_eq!(module(&engine, id).execution_rule(), NodeExecutionRule::passive());
    peer.send(Message::text("text 7\n")).unwrap();
    wait(&mut engine, |engine| {
        value(engine, id, "text") == Some(ParamValue::Int(7)) && idle(engine, id)
    });
    assert_eq!(callbacks(&engine, id), ["dataReceived", "dataReceive", "textReceived"]);
    peer.send(Message::binary(b"binary 8\n".to_vec())).unwrap();
    wait(&mut engine, |engine| {
        value(engine, id, "binary") == Some(ParamValue::Int(8)) && idle(engine, id)
    });
    assert_eq!(
        callbacks(&engine, id),
        [
            "dataReceived",
            "dataReceive",
            "textReceived",
            "dataReceived",
            "dataReceive"
        ]
    );
    for _ in 0..20 {
        tick(&mut engine);
        assert_eq!(engine.tick_stats().wake_callbacks_fired, 0);
    }
}

#[test]
fn applied_endpoint_edits_fence_new_sends_and_retirement_drains_every_accepted_frame() {
    let (mut engine, id, mut peer) = configured();
    send(&mut engine, id, "valid");
    engine.apply_edits().unwrap();
    assert_eq!(peer.read().unwrap(), Message::text("valid"));
    let expected = admit_prefix(&engine, id);
    let (port, next, task) = server("/next");
    let port_node = module(&engine, id).remote_port.id();
    let path_node = module(&engine, id).path.id();
    set(&mut engine, port_node, ParamValue::Int(port.into()));
    set(&mut engine, path_node, ParamValue::Str("/next".into()));
    send(&mut engine, id, "stale");
    assert!(engine.apply_edits().is_err());
    wait(&mut engine, |engine| connected(engine, id) && idle(engine, id));
    assert_eq!(read_prefix(&mut peer), expected);
    let mut next = next.recv_timeout(Duration::from_secs(2)).unwrap();
    task.join().unwrap();
    send(&mut engine, id, "new");
    engine.apply_edits().unwrap();
    assert_eq!(next.read().unwrap(), Message::text("new"));
}

#[test]
fn disabled_service_finishes_admitted_input_callbacks_and_output_before_reenable() {
    let (mut engine, id, mut peer) = configured();
    peer.send(Message::text("accepted 7\n")).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while !module(&engine, id).transport.as_ref().unwrap().has_pending() {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    let expected = admit_prefix(&engine, id);
    engine.edits.push(Edit::PatchMeta {
        node: id,
        patch: NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    send(&mut engine, id, "disabled");
    assert!(engine.apply_edits().is_err());
    wait(&mut engine, |engine| {
        value(engine, id, "accepted") == Some(ParamValue::Int(7)) && idle(engine, id)
    });
    assert!(module(&engine, id).transport.is_none());
    assert_eq!(read_prefix(&mut peer), expected);
    assert_eq!(callbacks(&engine, id), ["dataReceived", "dataReceive", "textReceived"]);
    let (port, next, task) = server("/fixture");
    let port_node = module(&engine, id).remote_port.id();
    set(&mut engine, port_node, ParamValue::Int(port.into()));
    engine.edits.push(Edit::PatchMeta {
        node: id,
        patch: NodeMetaPatch {
            enabled: Some(true),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    wait(&mut engine, |engine| connected(engine, id) && idle(engine, id));
    let _peer = next.recv_timeout(Duration::from_secs(2)).unwrap();
    task.join().unwrap();
}

#[test]
fn grammar_changes_update_native_input_without_replacing_the_live_socket() {
    let (mut engine, id, mut peer) = configured();
    let mode = path(&engine, id, "parameters/processing/parse_mode");
    set(&mut engine, mode, ParamValue::Enum("json".into()));
    engine.apply_edits().unwrap();
    wait(&mut engine, |engine| idle(engine, id));
    peer.send(Message::text(r#"{"json":9}"#)).unwrap();
    wait(&mut engine, |engine| {
        value(engine, id, "json") == Some(ParamValue::Int(9)) && idle(engine, id)
    });
    assert!(module(&engine, id).retiring_transport.is_none());
}

#[test]
fn snapshot_free_callback_recovery_retries_only_discarded_publication_and_undo_fences_old_delivery() {
    use crate::app::module::common::streaming::{
        delivery::InputDelivery,
        input::{self, InputSnapshot, StreamInputConfig},
        parser::StreamingParser,
    };
    let (mut engine, id, _peer) = configured();
    let packet = input::prepare(
        &mut StreamingParser::default(),
        &InputSnapshot {
            generation: 0,
            config: StreamInputConfig {
                module: id,
                parse: Default::default(),
                processing_enabled: true,
                log_incoming: false,
            },
        },
        b"stale 9\n",
        "WebSocket",
        None,
        true,
    )
    .unwrap();
    let crate::app::AppNode::WebSocketClientModule(node) = engine.nodes.get_mut(id).unwrap() else {
        panic!("WebSocket");
    };
    let old = node.wake.as_ref().unwrap().notifier.clone();
    node.pending_inputs.push_back(InputDelivery::new(packet));
    assert!(!node.worker_wake_requires_tree_snapshot());
    for turn in 0..10 {
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
        let crate::app::AppNode::WebSocketClientModule(node) = engine.nodes.get_mut(id).unwrap() else {
            panic!("WebSocket");
        };
        node.on_worker_wake(&mut ctx);
        if turn == 1 {
            engine.edits.push(Edit::SetParam {
                node: NodeId(u64::MAX),
                value: ParamValue::Int(1),
                behaviour: ParameterEventBehaviour::Append,
            });
        }
        engine.absorb_edits(&mut ctx).unwrap();
        if turn == 1 {
            assert!(engine.apply_edits().is_err());
        } else {
            engine.apply_edits().unwrap();
        }
    }
    assert_eq!(callbacks(&engine, id), ["dataReceived", "dataReceive", "textReceived"]);
    assert!(module(&engine, id).stream.has_pending_messages());
    engine.edits.push(Edit::RemoveNode { node: id });
    engine.apply_edits().unwrap();
    assert!(engine.undo().unwrap());
    assert!(module(&engine, id).pending_inputs.is_empty());
    wait(&mut engine, |engine| idle(engine, id));
    old.notify();
    tick(&mut engine);
    assert_eq!(engine.tick_stats().wake_callbacks_fired, 0);
    assert!(value(&engine, id, "stale").is_none());
}
