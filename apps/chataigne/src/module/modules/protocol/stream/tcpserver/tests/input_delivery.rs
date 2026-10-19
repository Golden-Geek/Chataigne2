use super::super::{CallbackDelivery, ServerDelivery};
use super::*;
use crate::app::module::common::streaming::{
    delivery::InputDelivery,
    input::{self, InputSnapshot, StreamInputConfig},
    parser::StreamingParser,
};
use crate::app::module::script_api::NativeCallbackPayload;
use golden_core::{engine::NodeExecutionRule, events::EventKind, process_ctx::ProcessCtx};
use std::{io::Write, time::Instant};

pub(super) fn configured() -> (crate::app::AppEngine, NodeId, u16) {
    let (mut engine, module, port) = create_tcp_server_module();
    let port_id = tcp_server_module(&engine, module).port.id();
    set_param(&mut engine, port_id, ParamValue::Int(port.into()));
    settle_transport_state(&mut engine);
    (engine, module, port)
}
pub(super) fn wait_owner(
    engine: &mut crate::app::AppEngine,
    mut condition: impl FnMut(&crate::app::AppEngine) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        engine.run_tick(Duration::from_millis(10)).unwrap();
        engine.apply_edits().unwrap();
        assert_eq!(engine.tick_stats().callbacks_fired, 0);
        if condition(engine) {
            return;
        }
        assert!(Instant::now() < deadline, "passive TCP server did not settle");
        thread::sleep(Duration::from_millis(1));
    }
}
pub(super) fn idle(engine: &crate::app::AppEngine, module: NodeId) -> bool {
    let node = tcp_server_module(engine, module);
    !node.transport_dirty
        && !node.client_list_dirty
        && !node.feedback.is_pending()
        && node.retiring_transport.is_none()
        && node.pending_deliveries.is_empty()
        && !node.stream.has_pending_messages()
        && !node.transport.as_ref().is_some_and(|transport| transport.has_pending())
}
pub(super) fn value(engine: &crate::app::AppEngine, module: NodeId, name: &str) -> Option<ParamValue> {
    let snapshot = engine.process_tree_snapshot();
    let values = find_child_by_key(engine, module, "values")?;
    snapshot
        .find_child(values, name)
        .and_then(|id| snapshot.node(id))
        .and_then(|node| node.param_value.clone())
}
pub(super) fn source_callbacks(engine: &crate::app::AppEngine, module: NodeId, source: &str) -> Vec<String> {
    engine
        .ui_event_log()
        .iter()
        .filter_map(|event| {
            let EventKind::Custom(event) = &event.kind else {
                return None;
            };
            if event.origin != Some(module) {
                return None;
            }
            let name = event.payload["callback"].as_str()?;
            let args = &event.payload["args"];
            let peer = if name.starts_with("client") { &args[0] } else { &args[1] };
            (peer.as_str() == Some(source)).then(|| name.to_string())
        })
        .collect()
}

#[test]
fn passive_per_client_input_applies_values_and_becomes_idle_without_compiled_callbacks() {
    let _guard = TCP_SERVER_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut engine, module, port) = configured();
    let mut left = connect_when_ready(port);
    let mut right = connect_when_ready(port);
    wait_for_connected_clients(&mut engine, module, 2);
    assert_eq!(
        tcp_server_module(&engine, module).execution_rule(),
        NodeExecutionRule::passive()
    );
    left.write_all(b"left ").unwrap();
    right.write_all(b"right 2\n").unwrap();
    wait_owner(&mut engine, |engine| {
        value(engine, module, "right") == Some(ParamValue::Int(2))
    });
    left.write_all(b"1\n").unwrap();
    wait_owner(&mut engine, |engine| {
        value(engine, module, "left") == Some(ParamValue::Int(1)) && idle(engine, module)
    });
    let callbacks = source_callbacks(&engine, module, &right.local_addr().unwrap().to_string());
    assert_eq!(
        callbacks,
        ["clientConnected", "dataReceived", "dataReceive", "textReceived"]
    );
    for _ in 0..20 {
        engine.run_tick(Duration::from_millis(10)).unwrap();
        assert_eq!(engine.tick_stats().callbacks_fired, 0);
        assert_eq!(engine.tick_stats().wake_callbacks_fired, 0);
    }
}

#[test]
fn processing_edit_updates_native_input_on_the_existing_clients() {
    let _guard = TCP_SERVER_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut engine, module, port) = configured();
    let mut client = connect_when_ready(port);
    wait_for_connected_clients(&mut engine, module, 1);
    client.write_all(b"old 1\n").unwrap();
    wait_owner(&mut engine, |engine| {
        value(engine, module, "old") == Some(ParamValue::Int(1)) && idle(engine, module)
    });
    let parameters = find_child_by_key(&engine, module, "parameters").unwrap();
    let processing = find_child_by_key(&engine, parameters, "processing").unwrap();
    let mode = find_child_by_key(&engine, processing, "parse_mode").unwrap();
    set_param(&mut engine, mode, ParamValue::Enum("json".into()));
    settle_transport_state(&mut engine);
    assert!(tcp_server_module(&engine, module).retiring_transport.is_none());
    client.write_all(br#"{"new":2}"#).unwrap();
    wait_owner(&mut engine, |engine| {
        value(engine, module, "new") == Some(ParamValue::Int(2)) && idle(engine, module)
    });
    assert_eq!(connected_clients_value(&engine, module), Some(1));
    assert_eq!(
        source_callbacks(&engine, module, &client.local_addr().unwrap().to_string()).len(),
        7
    );
}

#[test]
fn callback_receipts_retry_only_discarded_publication_without_rebuilding_input_or_reordering() {
    let _guard = TCP_SERVER_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut engine, module, _) = configured();
    wait_owner(&mut engine, |engine| idle(engine, module));
    let packet = input::prepare(
        &mut StreamingParser::default(),
        &InputSnapshot {
            generation: 0,
            config: StreamInputConfig {
                module,
                parse: Default::default(),
                processing_enabled: true,
                log_incoming: false,
            },
        },
        b"value 7\n",
        "TCP server",
        Some("fixture"),
        true,
    )
    .unwrap();
    let crate::app::AppNode::TcpServerModule(node) = engine.nodes.get_mut(module).unwrap() else {
        panic!("TCP server")
    };
    let connected = NativeCallbackPayload::prepare("clientConnected", vec!["fixture".into(), "info".into()]).unwrap();
    node.pending_deliveries
        .push_back(ServerDelivery::Callback(CallbackDelivery::new(connected.event(module))));
    node.pending_deliveries
        .push_back(ServerDelivery::Input(InputDelivery::new(packet)));
    assert!(!node.worker_wake_requires_tree_snapshot());
    for turn in 0..12 {
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
        let crate::app::AppNode::TcpServerModule(node) = engine.nodes.get_mut(module).unwrap() else {
            panic!("TCP server")
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
        source_callbacks(&engine, module, "fixture"),
        ["clientConnected", "dataReceived", "dataReceive", "textReceived"]
    );
    assert!(tcp_server_module(&engine, module).stream.has_pending_messages());
    wait_owner(&mut engine, |engine| {
        value(engine, module, "value") == Some(ParamValue::Int(7)) && idle(engine, module)
    });
    assert_eq!(source_callbacks(&engine, module, "fixture").len(), 4);
}

#[test]
fn destroy_undo_discards_runtime_delivery_and_rejects_the_previous_wake_source() {
    let _guard = TCP_SERVER_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut engine, module, _) = configured();
    wait_owner(&mut engine, |engine| idle(engine, module));
    let packet = input::prepare(
        &mut StreamingParser::default(),
        &InputSnapshot {
            generation: 0,
            config: StreamInputConfig {
                module,
                parse: Default::default(),
                processing_enabled: true,
                log_incoming: false,
            },
        },
        b"stale 9\n",
        "TCP server",
        Some("stale"),
        true,
    )
    .unwrap();
    let crate::app::AppNode::TcpServerModule(node) = engine.nodes.get_mut(module).unwrap() else {
        panic!("TCP server")
    };
    let old = node.wake.as_ref().unwrap().notifier.clone();
    node.pending_deliveries
        .push_back(ServerDelivery::Input(InputDelivery::new(packet)));
    engine.edits.push(Edit::RemoveNode { node: module });
    engine.apply_edits().unwrap();
    assert!(engine.undo().unwrap());
    assert!(tcp_server_module(&engine, module).pending_deliveries.is_empty());
    wait_owner(&mut engine, |engine| idle(engine, module));
    old.notify();
    engine.run_tick(Duration::from_millis(10)).unwrap();
    assert_eq!(engine.tick_stats().wake_callbacks_fired, 0);
    assert!(value(&engine, module, "stale").is_none());
    assert!(source_callbacks(&engine, module, "stale").is_empty());
}
