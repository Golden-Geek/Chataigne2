use super::super::WebSocketServerModule;
use super::*;
use golden_core::{node::Node, parameter::ParamValue};
use tokio_tungstenite::tungstenite::Message;

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
    assert!(!module.worker_wake_requires_tree_snapshot());
    module.retry_at = Some(Instant::now() - Duration::from_millis(1));
    assert!(!module.needs_update());
    assert!(module.worker_wake_requires_tree_snapshot());
}
