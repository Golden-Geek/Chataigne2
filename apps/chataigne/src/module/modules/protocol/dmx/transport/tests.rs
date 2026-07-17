use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    thread,
    time::{Duration, Instant},
};

use artnet_protocol::{ArtCommand, Output};

use super::*;

#[test]
fn artnet_worker_sends_a_protocol_encoded_frame() {
    let receiver = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    receiver
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    let destination = receiver.local_addr().unwrap();
    let mut transport = DmxTransportHandle::spawn(DmxTransportConfig {
        protocol: DmxProtocol::ArtNet,
        bind_ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
        listen_port: 6454,
        receive_enabled: false,
        universe: 2,
        destination: Some(destination),
    })
    .unwrap();

    transport
        .send(DmxFrame::with_metadata(2, 7, 100, vec![1, 2, 3]).unwrap())
        .unwrap();

    let mut bytes = [0_u8; 1_024];
    let (length, _) = receiver.recv_from(&mut bytes).unwrap();
    let ArtCommand::Output(output) = ArtCommand::from_buffer(&bytes[..length]).unwrap() else {
        panic!("expected ArtDMX output");
    };
    assert_eq!(u16::from(output.port_address), 1);
    assert_eq!(output.sequence, 7);
    assert_eq!(&output.data.as_ref()[..3], &[1, 2, 3]);
    transport.stop();
}

#[test]
fn artnet_worker_receives_latest_frame_without_an_unbounded_queue() {
    let started = Instant::now();
    let probe = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let listen_port = probe.local_addr().unwrap().port();
    drop(probe);
    let mut transport = DmxTransportHandle::spawn(DmxTransportConfig {
        protocol: DmxProtocol::ArtNet,
        bind_ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
        listen_port,
        receive_enabled: true,
        universe: 1,
        destination: Some(SocketAddr::from((Ipv4Addr::LOCALHOST, listen_port))),
    })
    .unwrap();
    let sender = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let send_frame = |value| {
        let packet = ArtCommand::Output(Output {
            data: vec![value].into(),
            ..Output::default()
        })
        .write_to_buffer()
        .unwrap();
        sender
            .send_to(packet.as_slice(), (Ipv4Addr::LOCALHOST, listen_port))
            .unwrap();
    };

    send_frame(10);
    let deadline = Instant::now() + Duration::from_millis(500);
    while !transport.has_pending() {
        assert!(Instant::now() < deadline, "timed out waiting for ArtDMX");
        thread::sleep(Duration::from_millis(1));
    }

    let mut replaced_frames = 0;
    let mut next_value = 20;
    while replaced_frames == 0 {
        send_frame(next_value);
        next_value = if next_value == 20 { 30 } else { 20 };
        thread::sleep(Duration::from_millis(1));
        replaced_frames += transport.take_replaced_frames();
        assert!(
            Instant::now() < deadline,
            "timed out creating deterministic latest-wins pressure"
        );
    }

    let event = transport
        .take_latest_event()
        .expect("latest-wins slot should retain a frame");
    let DmxWorkerEvent::Frame(frame) = event else {
        panic!("expected a received DMX frame");
    };
    assert!(matches!(frame.slots.first(), Some(20 | 30)));
    assert!(replaced_frames >= 1);
    transport.stop();
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[test]
fn sacn_worker_round_trips_a_unicast_frame() {
    let test_started = Instant::now();
    let probe = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let listen_port = probe.local_addr().unwrap().port();
    drop(probe);
    let mut transport = DmxTransportHandle::spawn(DmxTransportConfig {
        protocol: DmxProtocol::Sacn,
        bind_ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
        listen_port,
        receive_enabled: true,
        universe: 1,
        destination: Some(SocketAddr::from((Ipv4Addr::LOCALHOST, listen_port))),
    })
    .unwrap();
    transport
        .send(DmxFrame::with_metadata(1, 0, 110, vec![7, 8, 9]).unwrap())
        .unwrap();

    let deadline = Instant::now() + Duration::from_millis(500);
    let event = loop {
        if let Some(event) = transport.take_latest_event() {
            break event;
        }
        assert!(Instant::now() < deadline, "timed out waiting for sACN");
        thread::sleep(Duration::from_millis(5));
    };
    let DmxWorkerEvent::Frame(frame) = event else {
        panic!("expected a received sACN frame");
    };
    assert_eq!(frame.universe, 1);
    assert_eq!(frame.priority, 110);
    assert_eq!(&frame.slots[..3], &[7, 8, 9]);
    let stop_started = Instant::now();
    transport.stop();
    assert!(
        stop_started.elapsed() < Duration::from_secs(2),
        "sACN worker shutdown must interrupt a blocked receive"
    );
    assert!(
        test_started.elapsed() < Duration::from_secs(2),
        "local sACN round-trip and shutdown exceeded two seconds"
    );
}

#[test]
fn output_queue_reports_overload_instead_of_growing_without_bound() {
    let destination = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap();
    let transport = DmxTransportHandle::spawn(DmxTransportConfig {
        protocol: DmxProtocol::ArtNet,
        bind_ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
        listen_port: 6454,
        receive_enabled: false,
        universe: 1,
        destination: Some(destination),
    })
    .unwrap();
    let frame = DmxFrame::new(1, vec![1]).unwrap();
    let mut overload_observed = false;
    for _ in 0..10_000 {
        if transport.send(frame.clone()).is_err() {
            overload_observed = true;
            break;
        }
    }
    assert!(overload_observed);
}
