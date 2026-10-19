use super::input_delivery::{configured, idle, source_callbacks, value, wait_owner};
use super::*;
use std::{io::Write, time::Instant};

fn admit_prefix(engine: &crate::app::AppEngine, module: NodeId) -> Vec<u8> {
    let transport = tcp_server_module(engine, module).transport.as_ref().unwrap();
    let mut expected = Vec::new();
    for index in 0..1000 {
        let byte = index as u8;
        match transport.send(&[byte]) {
            Ok(()) => expected.push(byte),
            Err(error) => {
                assert!(error.contains("capacity exhausted"));
                break;
            }
        }
    }
    assert!(!expected.is_empty());
    expected
}

#[test]
fn reconfiguration_drains_every_admitted_byte_before_the_new_listener_starts() {
    let _guard = TCP_SERVER_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut engine, module, port) = configured();
    let mut client = connect_when_ready(port);
    client.set_read_timeout(Some(Duration::from_secs(4))).unwrap();
    wait_for_connected_clients(&mut engine, module, 1);
    let expected = admit_prefix(&engine, module);
    let new_port = free_tcp_port();
    let parameter = tcp_server_module(&engine, module).port.id();
    set_param(&mut engine, parameter, ParamValue::Int(new_port.into()));
    settle_transport_state(&mut engine);
    let mut received = Vec::new();
    client.read_to_end(&mut received).unwrap();
    assert_eq!(
        received, expected,
        "retirement must neither discard nor replay the accepted prefix"
    );
    wait_owner(&mut engine, |engine| idle(engine, module));
    assert_eq!(
        source_callbacks(&engine, module, &client.local_addr().unwrap().to_string()),
        ["clientConnected", "clientDisconnected"]
    );
    let mut replacement = connect_when_ready(new_port);
    replacement.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    wait_for_connected_clients(&mut engine, module, 1);
    tcp_server_module(&engine, module)
        .transport
        .as_ref()
        .unwrap()
        .send(b"new")
        .unwrap();
    let mut bytes = [0; 3];
    replacement.read_exact(&mut bytes).unwrap();
    assert_eq!(&bytes, b"new");
}

#[test]
fn disabling_finishes_accepted_output_input_and_callbacks_without_restoring_live_clients() {
    let _guard = TCP_SERVER_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut engine, module, port) = configured();
    let mut client = connect_when_ready(port);
    client.set_read_timeout(Some(Duration::from_secs(4))).unwrap();
    wait_for_connected_clients(&mut engine, module, 1);
    wait_owner(&mut engine, |engine| idle(engine, module));
    client.write_all(b"accepted 7\n").unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while !tcp_server_module(&engine, module)
        .transport
        .as_ref()
        .unwrap()
        .has_pending()
    {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    let expected = admit_prefix(&engine, module);
    set_node_enabled(&mut engine, module, false);
    settle_transport_state(&mut engine);
    wait_owner(&mut engine, |engine| {
        value(engine, module, "accepted") == Some(ParamValue::Int(7)) && idle(engine, module)
    });
    let mut received = Vec::new();
    client.read_to_end(&mut received).unwrap();
    assert_eq!(received, expected);
    assert_eq!(connected_clients_value(&engine, module), Some(0));
    assert!(tcp_server_module(&engine, module).transport.is_none());
    assert!(tcp_server_module(&engine, module).retiring_transport.is_none());
    assert_eq!(
        source_callbacks(&engine, module, &client.local_addr().unwrap().to_string()),
        [
            "clientConnected",
            "dataReceived",
            "dataReceive",
            "textReceived",
            "clientDisconnected"
        ]
    );
    set_node_enabled(&mut engine, module, true);
    settle_transport_state(&mut engine);
    let _new = connect_when_ready(port);
    wait_for_connected_clients(&mut engine, module, 1);
}
