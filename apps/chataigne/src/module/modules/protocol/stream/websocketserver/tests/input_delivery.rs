use super::super::{CallbackDelivery, ServerDelivery};
use super::*;
use crate::app::module::{
    common::streaming::{
        delivery::InputDelivery,
        input::{self, InputSnapshot, StreamInputConfig},
        parser::StreamingParser,
    },
    script_api::NativeCallbackPayload,
};
use golden_core::{
    edit::Edit,
    engine::NodeExecutionRule,
    events::EventKind,
    node::{Node, NodeId},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::{ExecutionPhase, ProcessCtx},
};
use tokio_tungstenite::tungstenite::Message;

pub(super) fn idle(engine: &crate::app::AppEngine, id: NodeId) -> bool {
    let node = module(engine, id);
    !node.transport_dirty
        && !node.client_list_dirty
        && !node.feedback.is_pending()
        && node.retiring_transport.is_none()
        && node.pending_deliveries.is_empty()
        && !node.stream.has_pending_messages()
        && !node.transport.as_ref().is_some_and(|t| t.has_pending())
}
pub(super) fn value(engine: &crate::app::AppEngine, id: NodeId, name: &str) -> Option<ParamValue> {
    let snapshot = engine.process_tree_snapshot();
    let values = snapshot.find_child_by_decl_id(id, "values")?;
    snapshot
        .find_child(values, name)
        .and_then(|id| snapshot.node(id))
        .and_then(|node| node.param_value.clone())
}
pub(super) fn source_callbacks(engine: &crate::app::AppEngine, id: NodeId, source: &str) -> Vec<String> {
    engine
        .ui_event_log()
        .iter()
        .filter_map(|event| {
            let EventKind::Custom(event) = &event.kind else {
                return None;
            };
            if event.origin != Some(id) {
                return None;
            }
            let name = event.payload["callback"].as_str()?;
            let args = &event.payload["args"];
            let peer = if name.starts_with("client") { &args[0] } else { &args[1] };
            (peer.as_str() == Some(source)).then(|| name.to_owned())
        })
        .collect()
}
fn packet(id: NodeId, bytes: &[u8], source: &str) -> golden_io::RetainedPayload<input::PreparedStreamInput> {
    input::prepare(
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
        bytes,
        "WebSocket server",
        Some(source),
        true,
    )
    .unwrap()
}

#[test]
fn passive_per_peer_input_applies_values_and_then_requests_no_idle_work() {
    let port = free_port();
    let (mut engine, id) = configured(port);
    let mut left = connect(&mut engine, id, port, "/");
    let mut right = connect(&mut engine, id, port, "/");
    assert_eq!(module(&engine, id).execution_rule(), NodeExecutionRule::passive());
    left.send(Message::text("left ")).unwrap();
    right.send(Message::text("right 2\n")).unwrap();
    wait(&mut engine, |engine| {
        value(engine, id, "right") == Some(ParamValue::Int(2))
    });
    assert!(value(&engine, id, "left").is_none());
    left.send(Message::text("1\n")).unwrap();
    wait(&mut engine, |engine| {
        value(engine, id, "left") == Some(ParamValue::Int(1)) && idle(engine, id)
    });
    assert_eq!(
        source_callbacks(&engine, id, &right.get_ref().local_addr().unwrap().to_string()),
        ["clientConnected", "dataReceived", "dataReceive", "textReceived"]
    );
    for _ in 0..20 {
        engine.run_tick(Duration::from_millis(10)).unwrap();
        assert_eq!(engine.tick_stats().callbacks_fired, 0);
        assert_eq!(engine.tick_stats().wake_callbacks_fired, 0);
    }
}

#[test]
fn callback_receipts_retry_discarded_publication_without_rebuilding_or_reordering_input() {
    let (mut engine, id) = configured(free_port());
    wait(&mut engine, |engine| idle(engine, id));
    let packet = packet(id, b"value 7\n", "fixture");
    let crate::app::AppNode::WebSocketServerModule(node) = engine.nodes.get_mut(id).unwrap() else {
        panic!("WebSocket server")
    };
    let connected = NativeCallbackPayload::prepare("clientConnected", vec!["fixture".into(), "info".into()]).unwrap();
    node.pending_deliveries
        .push_back(ServerDelivery::Callback(CallbackDelivery::new(connected.event(id))));
    node.pending_deliveries
        .push_back(ServerDelivery::Input(InputDelivery::new(packet)));
    assert!(!node.worker_wake_requires_tree_snapshot());
    for turn in 0..12 {
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
        let crate::app::AppNode::WebSocketServerModule(node) = engine.nodes.get_mut(id).unwrap() else {
            panic!("WebSocket server")
        };
        node.on_worker_wake(&mut ctx);
        if turn == 2 {
            engine.edits.push(Edit::SetParam {
                node: NodeId(u64::MAX),
                value: ParamValue::Int(1),
                behaviour: ParameterEventBehaviour::Append,
            });
        }
        engine.absorb_edits(&mut ctx).unwrap();
        if turn == 2 {
            assert!(engine.apply_edits().is_err());
        } else {
            engine.apply_edits().unwrap();
        }
    }
    assert_eq!(
        source_callbacks(&engine, id, "fixture"),
        ["clientConnected", "dataReceived", "dataReceive", "textReceived"]
    );
    assert!(module(&engine, id).stream.has_pending_messages());
    wait(&mut engine, |engine| {
        value(engine, id, "value") == Some(ParamValue::Int(7)) && idle(engine, id)
    });
    assert_eq!(source_callbacks(&engine, id, "fixture").len(), 4);
}

#[test]
fn destroy_undo_discards_runtime_input_and_fences_the_previous_wake_source() {
    let (mut engine, id) = configured(free_port());
    wait(&mut engine, |engine| idle(engine, id));
    let packet = packet(id, b"stale 9\n", "stale");
    let crate::app::AppNode::WebSocketServerModule(node) = engine.nodes.get_mut(id).unwrap() else {
        panic!("WebSocket server")
    };
    let old = node.wake.as_ref().unwrap().notifier.clone();
    let old_source = node.wake.as_ref().unwrap().source();
    node.pending_deliveries
        .push_back(ServerDelivery::Input(InputDelivery::new(packet)));
    engine.edits.push(Edit::RemoveNode { node: id });
    engine.apply_edits().unwrap();
    assert!(engine.undo().unwrap());
    assert!(module(&engine, id).pending_deliveries.is_empty());
    wait(&mut engine, |engine| idle(engine, id));
    assert!(old_source.is_closed());
    assert_eq!(old_source.wake(), Err(golden_core::engine::NodeWakeError::Closed));
    old.notify();
    engine.run_tick(Duration::from_millis(10)).unwrap();
    assert!(value(&engine, id, "stale").is_none());
    assert!(source_callbacks(&engine, id, "stale").is_empty());
}

#[test]
fn processing_edit_updates_existing_peers_without_retiring_the_listener() {
    let port = free_port();
    let (mut engine, id) = configured(port);
    let mut client = connect(&mut engine, id, port, "/");
    client.send(Message::text("old 1\n")).unwrap();
    wait(&mut engine, |engine| {
        value(engine, id, "old") == Some(ParamValue::Int(1)) && idle(engine, id)
    });
    let snapshot = engine.process_tree_snapshot();
    let parameters = snapshot.find_child_by_decl_id(id, "parameters").unwrap();
    let processing = snapshot.find_child_by_decl_id(parameters, "processing").unwrap();
    let mode = snapshot.find_child_by_decl_id(processing, "parse_mode").unwrap();
    set(&mut engine, mode, ParamValue::Enum("json".into()));
    engine.apply_edits().unwrap();
    wait(&mut engine, |engine| idle(engine, id));
    assert!(module(&engine, id).retiring_transport.is_none());
    client.send(Message::text(r#"{"new":2}"#)).unwrap();
    wait(&mut engine, |engine| {
        value(engine, id, "new") == Some(ParamValue::Int(2)) && idle(engine, id)
    });
    assert_eq!(module(&engine, id).client_infos.len(), 1);
    assert_eq!(
        source_callbacks(&engine, id, &client.get_ref().local_addr().unwrap().to_string()).len(),
        7
    );
}
