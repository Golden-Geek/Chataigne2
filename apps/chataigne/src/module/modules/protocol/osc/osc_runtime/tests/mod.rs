use std::{io, net::UdpSocket, sync::Arc, time::Duration};

use golden_core::parameter::ParamValue;
use golden_io::PendingDrainState;
use rosc::{decoder, encoder, OscMessage, OscPacket, OscType};

use super::{
    should_ignore_receive_error, OscOutboundMessage, OscTransportConfig, OscTransportHandle, OscValuePayload,
};

#[test]
fn ignores_windows_udp_icmp_connreset_receive_error() {
    let error = io::Error::from_raw_os_error(10054);
    assert!(should_ignore_receive_error(&error));
}

#[test]
fn keeps_reporting_unrelated_receive_errors() {
    let error = io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe");
    assert!(!should_ignore_receive_error(&error));
}

#[test]
fn worker_send_uses_waker_and_pre_resolved_remote_address() {
    let receiver = UdpSocket::bind("127.0.0.1:0").expect("test receiver should bind");
    receiver
        .set_read_timeout(Some(Duration::from_secs(1)))
        .expect("test receiver timeout should be configurable");
    let remote_address = receiver
        .local_addr()
        .expect("test receiver should have a local address");

    let mut handle = OscTransportHandle::spawn(OscTransportConfig {
        bind_interface_host: "127.0.0.1".to_string(),
        bind_port: 0,
        receive_enabled: false,
    })
    .expect("OSC worker should start");

    handle
        .send(OscOutboundMessage {
            address: Arc::from("/test"),
            payload: OscValuePayload::Single(ParamValue::Int(7)),
            remote_address,
        })
        .expect("OSC worker should accept send command");

    let mut buffer = [0u8; 1024];
    let (length, _source) = receiver
        .recv_from(&mut buffer)
        .expect("OSC worker should send without waiting for a timeout tick");
    let (_remaining, packet) = decoder::decode_udp(&buffer[..length]).expect("packet should decode");
    let rosc::OscPacket::Message(message) = packet else {
        panic!("expected OSC message packet");
    };

    assert_eq!(message.addr, "/test");
    assert_eq!(message.args, vec![rosc::OscType::Int(7)]);

    handle.stop();
}

#[test]
fn received_packet_publishes_pending_without_an_unrelated_wakeup() {
    let reserved = UdpSocket::bind("127.0.0.1:0").expect("test port should be reservable");
    let port = reserved
        .local_addr()
        .expect("reserved socket should expose its address")
        .port();
    drop(reserved);

    let mut handle = OscTransportHandle::spawn(OscTransportConfig {
        bind_interface_host: "127.0.0.1".to_string(),
        bind_port: port,
        receive_enabled: true,
    })
    .expect("OSC receiver should start");
    let sender = UdpSocket::bind("127.0.0.1:0").expect("test sender should bind");
    let packet = OscPacket::Message(OscMessage {
        addr: "/pending/only-packet".to_string(),
        args: vec![OscType::Int(17)],
    });
    let bytes = encoder::encode(&packet).expect("test OSC packet should encode");
    sender
        .send_to(bytes.as_slice(), ("127.0.0.1", port))
        .expect("test OSC packet should send");

    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while !handle.has_pending() {
        assert!(
            std::time::Instant::now() < deadline,
            "the only OSC packet must publish readiness without a second packet or command wakeup"
        );
        std::thread::yield_now();
    }

    let mut events = Vec::new();
    let drain = handle.drain_events(&mut events);
    assert_ne!(drain.state, PendingDrainState::Disconnected);
    assert_eq!(
        events,
        vec![super::OscWorkerEvent::Message(super::OscDecodedMessage {
            address: "/pending/only-packet".to_string(),
            payload: OscValuePayload::Single(ParamValue::Int(17)),
        })]
    );

    handle.stop();
}
