use super::super::input::tests::{artnet_packet, config, sacn_packet};
use super::*;
use std::{net::Ipv4Addr, thread};

fn receiver() -> UdpSocket {
    let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    socket.set_nonblocking(true).unwrap();
    socket
}

fn received(socket: &UdpSocket, input: &mut Receiver, bytes: &mut [u8]) -> ReceivePoll {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match receive(Some(socket), input, bytes, Duration::ZERO).unwrap() {
            ReceivePoll::Empty => {
                assert!(Instant::now() < deadline, "loopback datagram was not received");
                thread::sleep(Duration::from_millis(1));
            }
            packet => return packet,
        }
    }
}

#[test]
fn artnet_filters_universes_and_survives_bad_and_whole_oversized_datagrams() {
    let socket = receiver();
    let destination = socket.local_addr().unwrap();
    let sender = receiver();
    let mut packet = [0; UDP_PACKET_CAPACITY];
    let mut config = config(DmxProtocol::ArtNet);
    config.universe = 2;
    let mut input = Receiver::new(&config);
    sender.send_to(&artnet_packet(1, 0, 0, &[31, 32]), destination).unwrap();
    assert!(matches!(
        received(&socket, &mut input, &mut packet),
        ReceivePoll::Ignored
    ));
    sender.send_to(b"not an Art-Net packet", destination).unwrap();
    assert!(matches!(
        received(&socket, &mut input, &mut packet),
        ReceivePoll::Invalid(_)
    ));
    sender.send_to(&vec![0; 32_000], destination).unwrap();
    assert!(matches!(
        received(&socket, &mut input, &mut packet),
        ReceivePoll::Invalid(_)
    ));
    sender.send_to(&artnet_packet(2, 0, 0, &[31, 32]), destination).unwrap();
    let ReceivePoll::Frame(frame) = received(&socket, &mut input, &mut packet) else {
        panic!("valid configured universe discarded");
    };
    assert_eq!(frame.universe, 2);
    assert_eq!(frame.slots, [31, 32]);
    assert!(matches!(
        receive(Some(&socket), &mut input, &mut packet, Duration::ZERO).unwrap(),
        ReceivePoll::Empty
    ));
}

#[test]
fn sacn_rejects_other_universes_and_unhandled_start_codes_but_accepts_priority_zero() {
    let socket = receiver();
    let destination = socket.local_addr().unwrap();
    let sender = receiver();
    let mut packet = [0; UDP_PACKET_CAPACITY];
    let mut input = Receiver::new(&config(DmxProtocol::Sacn));
    let mut bytes = sacn_packet(2, 0, 100, &[17]);
    let mut decoded = ::sacn::Packet::parse(&bytes).unwrap();
    if let ::sacn::packet::Payload::Data(ref mut data) = decoded.payload {
        data.universe = 2;
    }
    bytes = decoded.to_vec().unwrap();
    sender.send_to(&bytes, destination).unwrap();
    assert!(matches!(
        received(&socket, &mut input, &mut packet),
        ReceivePoll::Ignored
    ));
    let bytes = sacn_packet(2, 1, 100, &[200]);
    let mut decoded = ::sacn::Packet::parse(&bytes).unwrap();
    if let ::sacn::packet::Payload::Data(ref mut data) = decoded.payload {
        data.start_code = 0xab;
    }
    sender.send_to(&decoded.to_vec().unwrap(), destination).unwrap();
    assert!(matches!(
        received(&socket, &mut input, &mut packet),
        ReceivePoll::Ignored
    ));
    sender.send_to(&sacn_packet(2, 2, 0, &[19, 20]), destination).unwrap();
    let ReceivePoll::Frame(frame) = received(&socket, &mut input, &mut packet) else {
        panic!("valid zero priority packet discarded");
    };
    assert_eq!(frame.metadata.priority, Some(0));
    assert_eq!(&frame.slots[..2], &[19, 20]);
}

#[test]
fn sacn_bad_and_oversized_traffic_does_not_retire_the_socket() {
    let socket = receiver();
    let destination = socket.local_addr().unwrap();
    let sender = receiver();
    let mut packet = [0; UDP_PACKET_CAPACITY];
    let mut input = Receiver::new(&config(DmxProtocol::Sacn));
    for bytes in [
        b"not an E1.31 packet".as_slice(),
        &vec![0; 32_000],
        &sacn_packet(2, 0, 201, &[42]),
    ] {
        sender.send_to(bytes, destination).unwrap();
        assert!(matches!(
            received(&socket, &mut input, &mut packet),
            ReceivePoll::Invalid(_)
        ));
    }
    sender.send_to(&sacn_packet(2, 1, 100, &[42]), destination).unwrap();
    assert!(matches!(
        received(&socket, &mut input, &mut packet),
        ReceivePoll::Frame(_)
    ));
}

#[test]
fn multicast_memberships_remain_bounded_during_repeated_replacement() {
    let mut joined = Vec::new();
    let mut actual = std::collections::BTreeSet::from([1]);
    for generation in 0..16 {
        let desired: Vec<_> = (2..10).map(|universe| universe + generation * 8).collect();
        reconcile_memberships(&mut joined, &desired, 1, |universe, join| {
            if join {
                assert!(actual.insert(universe));
            } else {
                assert!(actual.remove(&universe));
            }
            assert!(actual.len() <= 9, "replacements must leave before joining");
            assert!(actual.contains(&1), "the configured data group remains joined");
            Ok(())
        })
        .unwrap();
        assert_eq!(joined, desired);
    }
    reconcile_memberships(&mut joined, &[1], 1, |universe, join| {
        assert!(!join);
        actual.remove(&universe);
        Ok(())
    })
    .unwrap();
    assert!(joined.is_empty());
    assert_eq!(actual, std::collections::BTreeSet::from([1]));
}

#[test]
fn membership_failure_records_only_completed_joins_before_endpoint_retirement() {
    let mut joined = Vec::new();
    let error = reconcile_memberships(&mut joined, &[2, 3, 4], 1, |universe, _| {
        if universe == 3 {
            Err("injected join failure".into())
        } else {
            Ok(())
        }
    })
    .unwrap_err();
    assert_eq!(error, "injected join failure");
    assert_eq!(joined, [2]);
    assert!(
        reconcile_memberships(&mut joined, &[9; 9], 1, |_, _| panic!(
            "over-capacity request must fail before I/O"
        ))
        .is_err()
    );
}
