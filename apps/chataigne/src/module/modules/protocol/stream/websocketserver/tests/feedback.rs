use super::input_delivery::idle;
use super::*;
use golden_core::{
    edit::Edit,
    node::NodeId,
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::{ExecutionPhase, ProcessCtx},
};

#[test]
fn discarded_client_projection_reconciles_from_live_state_without_another_peer_event() {
    let (mut engine, id) = configured(free_port());
    wait(&mut engine, |engine| idle(engine, id));
    let snapshot = engine.process_tree_snapshot();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    let crate::app::AppNode::WebSocketServerModule(node) = engine.nodes.get_mut(id).unwrap() else {
        panic!("WebSocket server")
    };
    node.client_infos
        .insert("fixture".into(), "retained information".into());
    node.client_list_dirty = true;
    node.sync_client_nodes(&mut ctx, &snapshot);
    engine.edits.push(Edit::SetParam {
        node: NodeId(u64::MAX),
        value: ParamValue::Int(1),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.absorb_edits(&mut ctx).unwrap();
    assert!(engine.apply_edits().is_err());
    wait(&mut engine, |engine| idle(engine, id));
    let snapshot = engine.process_tree_snapshot();
    let connection = snapshot.find_child_by_decl_id(id, "connection").unwrap();
    let clients = snapshot.find_child_by_decl_id(connection, "clients").unwrap();
    assert!(
        snapshot.find_child(clients, "fixture").is_some(),
        "live client projection must recover a discarded edit batch"
    );
    let count = snapshot.find_child_by_decl_id(connection, "connected_clients").unwrap();
    assert_eq!(
        snapshot.node(count).unwrap().param_value.as_ref(),
        Some(&ParamValue::Int(1))
    );
}

#[test]
fn partially_applied_projection_keeps_existing_client_ids_and_reconciles_its_suffix() {
    let (mut engine, id) = configured(free_port());
    wait(&mut engine, |engine| connected(engine, id) && idle(engine, id));
    let snapshot = engine.process_tree_snapshot();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    let crate::app::AppNode::WebSocketServerModule(node) = engine.nodes.get_mut(id).unwrap() else {
        panic!("WebSocket server")
    };
    node.client_infos.insert("first".into(), "first information".into());
    node.client_infos.insert("second".into(), "second information".into());
    node.client_list_dirty = true;
    node.sync_client_nodes(&mut ctx, &snapshot);
    let last = ctx.edits.pending.pop().unwrap();
    ctx.edits.push(Edit::SetParam {
        node: NodeId(u64::MAX),
        value: ParamValue::Int(1),
        behaviour: ParameterEventBehaviour::Append,
    });
    ctx.edits.pending.push(last);
    engine.absorb_edits(&mut ctx).unwrap();
    assert!(engine.apply_edits().is_err());
    let snapshot = engine.process_tree_snapshot();
    let connection = snapshot.find_child_by_decl_id(id, "connection").unwrap();
    let clients = snapshot.find_child_by_decl_id(connection, "clients").unwrap();
    let first = snapshot.find_child(clients, "first").unwrap();
    assert!(snapshot.find_child(clients, "second").is_none());
    wait(&mut engine, |engine| idle(engine, id));
    let snapshot = engine.process_tree_snapshot();
    assert_eq!(snapshot.find_child(clients, "first"), Some(first));
    assert!(snapshot.find_child(clients, "second").is_some());
    assert_eq!(snapshot.child_ids_slice(clients).len(), 2);
}

#[test]
fn connection_and_warning_projection_retries_set_and_clear_without_another_native_transition() {
    let (mut engine, id) = configured(free_port());
    wait(&mut engine, |engine| connected(engine, id) && idle(engine, id));
    for desired in [false, true] {
        let snapshot = engine.process_tree_snapshot();
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
        let crate::app::AppNode::WebSocketServerModule(node) = engine.nodes.get_mut(id).unwrap() else {
            panic!("WebSocket server")
        };
        node.set_connected(desired);
        if desired {
            node.clear_port_warning();
        } else {
            node.set_port_warning("fixture warning");
        }
        node.sync_client_nodes(&mut ctx, &snapshot);
        engine.edits.push(Edit::SetParam {
            node: NodeId(u64::MAX),
            value: ParamValue::Int(1),
            behaviour: ParameterEventBehaviour::Append,
        });
        engine.absorb_edits(&mut ctx).unwrap();
        assert!(engine.apply_edits().is_err());
        wait(&mut engine, |engine| idle(engine, id));
        assert_eq!(connected(&engine, id), desired);
        let port = module(&engine, id).port.id();
        let snapshot = engine.process_tree_snapshot();
        let warning = snapshot
            .node(port)
            .unwrap()
            .presentation
            .warning(Some(super::super::WEBSOCKET_SERVER_PORT_WARNING_ID));
        assert_eq!(
            warning.map(|warning| warning.message.as_str()),
            if desired { None } else { Some("fixture warning") }
        );
    }
}
