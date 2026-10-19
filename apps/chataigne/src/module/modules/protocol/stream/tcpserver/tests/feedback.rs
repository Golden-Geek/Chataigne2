use super::input_delivery::{configured, idle, wait_owner};
use super::*;
use golden_core::process_ctx::ProcessCtx;

#[test]
fn discarded_tcp_client_projection_reconciles_without_another_peer_event() {
    let _guard = TCP_SERVER_TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let (mut engine, id, _) = configured();
    wait_owner(&mut engine, |engine| idle(engine, id));
    let snapshot = engine.process_tree_snapshot();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    let crate::app::AppNode::TcpServerModule(node) = engine.nodes.get_mut(id).unwrap() else {
        panic!("TCP server")
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
    wait_owner(&mut engine, |engine| idle(engine, id));
    let snapshot = engine.process_tree_snapshot();
    let connection = snapshot.find_child_by_decl_id(id, "connection").unwrap();
    let clients = snapshot.find_child_by_decl_id(connection, "clients").unwrap();
    assert!(
        snapshot.find_child(clients, "fixture").is_some(),
        "live TCP client projection must recover a discarded edit batch"
    );
    let count = snapshot.find_child_by_decl_id(connection, "connected_clients").unwrap();
    assert_eq!(
        snapshot.node(count).unwrap().param_value.as_ref(),
        Some(&ParamValue::Int(1))
    );
}

fn connected(engine: &crate::app::AppEngine, id: NodeId) -> bool {
    let snapshot = engine.process_tree_snapshot();
    let connection = snapshot.find_child_by_decl_id(id, "connection").unwrap();
    let connected = snapshot.find_child_by_decl_id(connection, "connected").unwrap();
    snapshot.node(connected).unwrap().param_value.as_ref() == Some(&ParamValue::Bool(true))
}

#[test]
fn partially_applied_projection_keeps_existing_client_ids_and_reconciles_its_suffix() {
    let _guard = TCP_SERVER_TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let (mut engine, id, _) = configured();
    wait_owner(&mut engine, |engine| connected(engine, id) && idle(engine, id));
    let snapshot = engine.process_tree_snapshot();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    let crate::app::AppNode::TcpServerModule(node) = engine.nodes.get_mut(id).unwrap() else {
        panic!("TCP server")
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
    wait_owner(&mut engine, |engine| idle(engine, id));
    let snapshot = engine.process_tree_snapshot();
    assert_eq!(snapshot.find_child(clients, "first"), Some(first));
    assert!(snapshot.find_child(clients, "second").is_some());
    assert_eq!(snapshot.child_ids_slice(clients).len(), 2);
}

#[test]
fn connection_and_warning_projection_retries_set_and_clear_without_another_native_transition() {
    let _guard = TCP_SERVER_TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let (mut engine, id, _) = configured();
    wait_owner(&mut engine, |engine| connected(engine, id) && idle(engine, id));
    for desired in [false, true] {
        let snapshot = engine.process_tree_snapshot();
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
        let crate::app::AppNode::TcpServerModule(node) = engine.nodes.get_mut(id).unwrap() else {
            panic!("TCP server")
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
        wait_owner(&mut engine, |engine| idle(engine, id));
        assert_eq!(connected(&engine, id), desired);
        let port = tcp_server_module(&engine, id).port.id();
        let snapshot = engine.process_tree_snapshot();
        let warning = snapshot
            .node(port)
            .unwrap()
            .presentation
            .warning(Some(super::super::TCP_SERVER_PORT_WARNING_ID));
        assert_eq!(
            warning.map(|warning| warning.message.as_str()),
            if desired { None } else { Some("fixture warning") }
        );
    }
}

#[test]
fn inflight_projection_reconciles_latest_state_after_the_prior_receipt_applies() {
    let _guard = TCP_SERVER_TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let (mut engine, id, _) = configured();
    wait_owner(&mut engine, |engine| connected(engine, id) && idle(engine, id));
    let snapshot = engine.process_tree_snapshot();
    let mut previous = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    let mut later = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    let crate::app::AppNode::TcpServerModule(node) = engine.nodes.get_mut(id).unwrap() else {
        panic!("server")
    };
    node.set_connected(false);
    node.set_port_warning("old warning");
    node.client_infos.insert("old".into(), "old information".into());
    node.client_list_dirty = true;
    node.sync_client_nodes(&mut previous, &snapshot);
    assert!(!previous.edits.pending.is_empty());
    node.set_connected(true);
    node.set_port_warning("new warning");
    node.client_infos.clear();
    node.client_infos.insert("new".into(), "new information".into());
    node.client_list_dirty = true;
    node.sync_client_nodes(&mut later, &snapshot);
    assert!(later.edits.pending.is_empty(), "only one projection may be in flight");
    engine.absorb_edits(&mut previous).unwrap();
    engine.apply_edits().unwrap();
    assert!(!connected(&engine, id));
    wait_owner(&mut engine, |engine| idle(engine, id));
    assert!(connected(&engine, id));
    let snapshot = engine.process_tree_snapshot();
    let connection = snapshot.find_child_by_decl_id(id, "connection").unwrap();
    let clients = snapshot.find_child_by_decl_id(connection, "clients").unwrap();
    assert!(snapshot.find_child(clients, "old").is_none());
    let new = snapshot.find_child(clients, "new").unwrap();
    assert_eq!(
        snapshot.node(new).unwrap().param_value.as_ref(),
        Some(&ParamValue::Str("new information".into()))
    );
    assert_eq!(snapshot.child_ids_slice(clients).len(), 1);
    let port = tcp_server_module(&engine, id).port.id();
    assert_eq!(
        snapshot
            .node(port)
            .unwrap()
            .presentation
            .warning(Some(super::super::TCP_SERVER_PORT_WARNING_ID))
            .unwrap()
            .message,
        "new warning"
    );
}
