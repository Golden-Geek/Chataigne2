use super::*;
use std::io::ErrorKind;

fn send(engine: &mut crate::app::AppEngine, module: NodeId, text: &str) {
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: module,
        method: "sendText".into(),
        args: vec![ParamValue::Str(text.into())],
    });
}
fn assert_no_stale_bytes(client: &mut TcpStream) {
    client.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
    match client.read(&mut [0; 32]) {
        Ok(0) => {}
        Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
        result => panic!("an unadmitted command reached the old peer: {result:?}"),
    }
}

#[test]
fn applied_port_and_disable_edits_fence_script_admission_before_callbacks_dispatch() {
    let _guard = TCP_SERVER_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut engine, module, first_port) = create_tcp_server_module();
    let port = tcp_server_module(&engine, module).port.id();
    set_param(&mut engine, port, ParamValue::Int(first_port.into()));
    settle_transport_state(&mut engine);
    let mut first = connect_when_ready(first_port);
    wait_for_connected_clients(&mut engine, module, 1);
    send(&mut engine, module, "valid");
    engine.apply_edits().unwrap();
    first.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let mut valid = [0; 5];
    first.read_exact(&mut valid).unwrap();
    assert_eq!(&valid, b"valid");

    let second_port = free_tcp_port();
    set_param(&mut engine, port, ParamValue::Int(second_port.into()));
    send(&mut engine, module, "stale");
    assert!(engine.apply_edits().is_err());
    assert_no_stale_bytes(&mut first);
    settle_transport_state(&mut engine);
    let mut second = connect_when_ready(second_port);
    wait_for_connected_clients(&mut engine, module, 1);
    send(&mut engine, module, "new");
    engine.apply_edits().unwrap();
    second.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let mut new = [0; 3];
    second.read_exact(&mut new).unwrap();
    assert_eq!(&new, b"new");
    set_node_enabled(&mut engine, module, false);
    send(&mut engine, module, "disabled");
    assert!(engine.apply_edits().is_err());
    assert_no_stale_bytes(&mut second);
}

#[test]
fn capacity_retry_wait_requests_no_periodic_work_and_defers_snapshots_until_due() {
    let mut module = TcpServerModule::create();
    module.transport_dirty = true;
    module.client_list_dirty = false;
    module.retry_at = Some(std::time::Instant::now() + Duration::from_secs(1));
    assert!(!module.transport_refresh_due());
    assert!(!module.needs_update());
    assert!(!module.worker_wake_requires_tree_snapshot());
    module.retry_at = Some(std::time::Instant::now() - Duration::from_millis(1));
    assert!(module.transport_refresh_due());
    assert!(!module.needs_update());
    assert!(module.worker_wake_requires_tree_snapshot());
}
