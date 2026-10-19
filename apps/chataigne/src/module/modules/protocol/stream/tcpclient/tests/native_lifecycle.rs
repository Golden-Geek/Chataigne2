use super::*;
use std::io::Read;

fn wait_until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !condition() {
        assert!(Instant::now() < deadline, "TCP native owner did not settle");
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn tcp_client_idle_runtime_has_no_periodic_node_callbacks() {
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
    let (_peer, _) = listener.accept().unwrap();
    assert_eq!(
        tcp_module(&engine, module).execution_rule(),
        golden_core::engine::NodeExecutionRule::passive()
    );
    for _ in 0..10 {
        engine.run_tick(Duration::from_millis(20)).unwrap();
        assert!(!tcp_module(&engine, module).needs_update());
        assert_eq!(engine.tick_stats().callbacks_fired, 0);
    }
}

#[test]
fn endpoint_reconfiguration_finishes_the_old_admitted_prefix_before_opening_the_new_endpoint() {
    let first = TcpListener::bind("127.0.0.1:0").unwrap();
    let second = TcpListener::bind("127.0.0.1:0").unwrap();
    let (mut engine, module) = create_tcp_module();
    let port = tcp_module(&engine, module).remote_port.id();
    set_param(
        &mut engine,
        port,
        ParamValue::Int(i32::from(first.local_addr().unwrap().port())),
    );
    settle_transport_state(&mut engine);
    wait_for_connected_value(&mut engine, module, true);
    let (mut old_peer, _) = first.accept().unwrap();
    old_peer.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let mut expected = Vec::new();
    let native = tcp_module(&engine, module).transport.as_ref().unwrap();
    for index in 0..1000 {
        let value = index as u8;
        match native.send(&[value], true) {
            Ok(()) => expected.push(value),
            Err(error) => {
                assert!(error.contains("capacity exhausted"));
                break;
            }
        }
    }
    wait_until(|| native.event_metrics().rejected > 0);
    set_param(
        &mut engine,
        port,
        ParamValue::Int(i32::from(second.local_addr().unwrap().port())),
    );
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: module,
        call: golden_core::script::ScriptMethodCall::prepare("sendText", vec![ParamValue::Str("too-early".into())])
            .unwrap(),
    });
    assert!(
        engine.apply_edits().is_err(),
        "a send after the applied endpoint edit must not reach the stale handle"
    );
    settle_transport_state(&mut engine);
    wait_for_connected_value(&mut engine, module, true);
    let mut actual = Vec::new();
    old_peer.read_to_end(&mut actual).unwrap();
    assert_eq!(
        actual, expected,
        "an endpoint edit must not cancel or reroute admitted bytes"
    );
    let (mut new_peer, _) = second.accept().unwrap();
    new_peer.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: module,
        call: golden_core::script::ScriptMethodCall::prepare("sendText", vec![ParamValue::Str("new".into())]).unwrap(),
    });
    engine.apply_edits().unwrap();
    let mut bytes = [0; 3];
    new_peer.read_exact(&mut bytes).unwrap();
    assert_eq!(&bytes, b"new");
}

#[test]
fn disabling_a_module_drains_accepted_writes_while_keeping_its_endpoint_disabled() {
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
    let (mut peer, _) = listener.accept().unwrap();
    peer.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let native = tcp_module(&engine, module).transport.as_ref().unwrap();
    let mut expected = Vec::new();
    for index in 0..1000 {
        let value = index as u8;
        match native.send(&[value], true) {
            Ok(()) => expected.push(value),
            Err(error) => {
                assert!(error.contains("capacity exhausted"));
                break;
            }
        }
    }
    wait_until(|| native.event_metrics().rejected > 0);
    set_node_enabled(&mut engine, module, false);
    settle_transport_state(&mut engine);
    let mut actual = Vec::new();
    peer.read_to_end(&mut actual).unwrap();
    assert_eq!(actual, expected);
    assert!(tcp_module(&engine, module).transport.is_none());
    assert!(tcp_module(&engine, module).retiring_transport.is_none());
    assert_eq!(connected_value(&engine, module), Some(false));
}
