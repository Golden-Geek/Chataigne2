use std::{io, net::UdpSocket, sync::Arc, time::Duration};

use golden_core::parameter::ParamValue;
use rosc::decoder;

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
