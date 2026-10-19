use super::input_delivery::{idle, source_callbacks, value};
use super::*;
use crate::app::module::common::streaming::commands::StreamingSendFrameKind;
use golden_core::{
    node::{Node, NodeId},
    parameter::ParamValue,
};
use std::net::TcpStream;
use tokio_tungstenite::tungstenite::{Message, WebSocket};

fn admit_prefix(engine: &crate::app::AppEngine, id: NodeId) -> Vec<u8> {
    let transport = module(engine, id).transport.as_ref().unwrap();
    let mut expected = Vec::new();
    for index in 0..1000 {
        let byte = index as u8;
        match transport.send(StreamingSendFrameKind::Binary, &[byte], false) {
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
fn receive_prefix(client: &mut WebSocket<TcpStream>) -> Vec<u8> {
    let mut received = Vec::new();
    loop {
        match client.read().unwrap() {
            Message::Binary(bytes) => {
                assert_eq!(bytes.len(), 1);
                received.push(bytes[0]);
            }
            Message::Close(_) => return received,
            other => panic!("unexpected retirement frame: {other:?}"),
        }
    }
}

#[test]
fn reconfiguration_drains_each_admitted_frame_before_starting_the_replacement() {
    let port = free_port();
    let (mut engine, id) = configured(port);
    let mut client = connect(&mut engine, id, port, "/");
    wait(&mut engine, |engine| idle(engine, id));
    let expected = admit_prefix(&engine, id);
    let new_port = free_port();
    let parameter = module(&engine, id).port.id();
    set(&mut engine, parameter, ParamValue::Int(new_port.into()));
    engine.apply_edits().unwrap();
    wait(&mut engine, |engine| idle(engine, id));
    assert_eq!(receive_prefix(&mut client), expected);
    assert_eq!(
        source_callbacks(&engine, id, &client.get_ref().local_addr().unwrap().to_string()),
        ["clientConnected", "clientDisconnected"]
    );
    let mut replacement = connect(&mut engine, id, new_port, "/");
    send(&mut engine, id, "new");
    engine.apply_edits().unwrap();
    assert_eq!(replacement.read().unwrap(), Message::text("new"));
}

#[test]
fn disable_finishes_accepted_frames_input_and_callbacks_without_restoring_live_clients() {
    let port = free_port();
    let (mut engine, id) = configured(port);
    let mut client = connect(&mut engine, id, port, "/");
    wait(&mut engine, |engine| idle(engine, id));
    client.send(Message::text("accepted 7\n")).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while !module(&engine, id).transport.as_ref().unwrap().has_pending() {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    let expected = admit_prefix(&engine, id);
    enable(&mut engine, id, false);
    engine.apply_edits().unwrap();
    wait(&mut engine, |engine| {
        value(engine, id, "accepted") == Some(ParamValue::Int(7)) && idle(engine, id)
    });
    assert_eq!(receive_prefix(&mut client), expected);
    assert_eq!(module(&engine, id).client_infos.len(), 0);
    assert!(module(&engine, id).transport.is_none());
    assert!(module(&engine, id).retiring_transport.is_none());
    assert_eq!(
        source_callbacks(&engine, id, &client.get_ref().local_addr().unwrap().to_string()),
        [
            "clientConnected",
            "dataReceived",
            "dataReceive",
            "textReceived",
            "clientDisconnected"
        ]
    );
    assert!(!module(&engine, id).worker_wake_when_disabled());
    enable(&mut engine, id, true);
    engine.apply_edits().unwrap();
    let _new = connect(&mut engine, id, port, "/");
}
