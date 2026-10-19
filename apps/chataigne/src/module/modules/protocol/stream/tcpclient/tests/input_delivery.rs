use super::*;
use golden_core::{events::EventKind, process_ctx::ProcessCtx};
use std::{io::Write, net::TcpStream};

fn connected() -> (crate::app::AppEngine, NodeId, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let (mut engine, module) = create_tcp_module();
    let port = tcp_module(&engine, module).remote_port.id();
    set_param(
        &mut engine,
        port,
        ParamValue::Int(i32::from(listener.local_addr().unwrap().port())),
    );
    settle_transport_state(&mut engine);
    wait_for_connected_value(&mut engine, module, true);
    let (peer, _) = listener.accept().unwrap();
    (engine, module, peer)
}

fn callbacks(engine: &crate::app::AppEngine, module: NodeId) -> Vec<String> {
    engine
        .ui_event_log()
        .iter()
        .filter_map(|event| {
            let EventKind::Custom(event) = &event.kind else {
                return None;
            };
            let name = event.payload.get("callback")?.as_str()?;
            (event.origin == Some(module) && ["dataReceived", "dataReceive", "textReceived"].contains(&name))
                .then(|| name.to_string())
        })
        .collect()
}

fn wait_for_value(engine: &mut crate::app::AppEngine, module: NodeId, name: &str, expected: i32) {
    let values = find_child_by_key(engine, module, "values").unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        engine.run_tick(Duration::from_millis(10)).unwrap();
        assert_eq!(engine.tick_stats().callbacks_fired, 0);
        let snapshot = engine.process_tree_snapshot();
        let value = snapshot
            .find_child(values, name)
            .and_then(|id| snapshot.node(id))
            .and_then(|node| node.param_value.as_ref());
        if value == Some(&ParamValue::Int(expected))
            && tcp_module(engine, module).pending_inputs.is_empty()
            && !tcp_module(engine, module).stream.has_pending_messages()
        {
            return;
        }
        assert!(Instant::now() < deadline, "passive TCP input did not apply {name}");
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn passive_input_applies_values_and_emits_each_callback_once_then_becomes_idle() {
    let (mut engine, module, mut peer) = connected();
    peer.write_all(b"level 7\n").unwrap();
    wait_for_value(&mut engine, module, "level", 7);
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
fn processing_edit_updates_native_input_on_the_existing_socket() {
    let (mut engine, module, mut peer) = connected();
    peer.write_all(b"old 1\n").unwrap();
    wait_for_value(&mut engine, module, "old", 1);
    let parameters = find_child_by_key(&engine, module, "parameters").unwrap();
    let processing = find_child_by_key(&engine, parameters, "processing").unwrap();
    let mode = find_child_by_key(&engine, processing, "parse_mode").unwrap();
    set_param(&mut engine, mode, ParamValue::Enum("json".into()));
    settle_transport_state(&mut engine);
    peer.write_all(br#"{"new":2}"#).unwrap();
    wait_for_value(&mut engine, module, "new", 2);
    assert!(tcp_module(&engine, module).retiring_transport.is_none());
    assert_eq!(callbacks(&engine, module).len(), 6);
}

#[test]
fn callback_only_service_needs_no_snapshot_and_destroy_undo_discards_pending_input() {
    use crate::app::module::common::streaming::{
        delivery::InputDelivery,
        input::{self, InputSnapshot, StreamInputConfig},
        parser::StreamingParser,
    };
    let (mut engine, module, _peer) = connected();
    let snapshot = InputSnapshot {
        generation: 0,
        config: StreamInputConfig {
            module,
            parse: Default::default(),
            processing_enabled: true,
            log_incoming: false,
        },
    };
    let packet = input::prepare(&mut StreamingParser::default(), &snapshot, b"stale 9\n", "TCP", None).unwrap();
    let crate::app::AppNode::TcpClientModule(node) = engine.nodes.get_mut(module).unwrap() else {
        panic!("TCP")
    };
    node.pending_inputs.push_back(InputDelivery::new(packet));
    assert!(!node.worker_wake_requires_tree_snapshot());
    assert!(node.worker_wake_when_disabled());
    for _ in 0..4 {
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
        let crate::app::AppNode::TcpClientModule(node) = engine.nodes.get_mut(module).unwrap() else {
            panic!("TCP")
        };
        node.on_worker_wake(&mut ctx);
        engine.absorb_edits(&mut ctx).unwrap();
        engine.apply_edits().unwrap();
    }
    assert_eq!(
        callbacks(&engine, module),
        ["dataReceived", "dataReceive", "textReceived"]
    );
    assert!(tcp_module(&engine, module).stream.has_pending_messages());
    assert!(tcp_module(&engine, module).worker_wake_requires_tree_snapshot());
    engine.edits.push(Edit::RemoveNode { node: module });
    engine.apply_edits().unwrap();
    assert!(engine.undo().unwrap());
    assert!(!tcp_module(&engine, module).stream.has_pending_messages());
    assert!(tcp_module(&engine, module).pending_inputs.is_empty());
    for _ in 0..10 {
        engine.run_tick(Duration::from_millis(10)).unwrap();
    }
    let values = find_child_by_key(&engine, module, "values").unwrap();
    assert!(engine.process_tree_snapshot().find_child(values, "stale").is_none());
}
