use super::*;
use artnet_protocol::Output;
use std::{
    net::Ipv4Addr,
    thread,
    time::{Duration, Instant},
};

fn receiver() -> UdpSocket {
    let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    socket.set_nonblocking(true).unwrap();
    socket
}

fn received(mut poll: impl FnMut() -> Result<ReceivePoll, String>) -> ReceivePoll {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match poll().unwrap() {
            ReceivePoll::Empty => {
                assert!(Instant::now() < deadline, "loopback datagram was not received");
                thread::sleep(Duration::from_millis(1));
            }
            packet => return packet,
        }
    }
}

fn artnet_packet(universe: u16) -> Vec<u8> {
    ArtCommand::Output(Output {
        port_address: (universe - 1).try_into().unwrap(),
        data: vec![31, 32].into(),
        ..Output::default()
    })
    .write_to_buffer()
    .unwrap()
}

#[test]
fn artnet_filters_universes_and_survives_bad_and_whole_oversized_datagrams() {
    let socket = receiver();
    let destination = socket.local_addr().unwrap();
    let sender = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let mut packet = [0; UDP_PACKET_CAPACITY];

    sender.send_to(&artnet_packet(1), destination).unwrap();
    assert!(matches!(
        received(|| receive_artnet(&socket, 2, &mut packet)),
        ReceivePoll::Ignored
    ));
    sender.send_to(b"not an Art-Net packet", destination).unwrap();
    assert!(matches!(
        received(|| receive_artnet(&socket, 2, &mut packet)),
        ReceivePoll::Invalid(_)
    ));
    sender.send_to(&vec![0; 32_000], destination).unwrap();
    assert!(matches!(
        received(|| receive_artnet(&socket, 2, &mut packet)),
        ReceivePoll::Invalid(_)
    ));
    sender.send_to(&artnet_packet(2), destination).unwrap();
    let ReceivePoll::Frame(frame) = received(|| receive_artnet(&socket, 2, &mut packet)) else {
        panic!("valid configured-universe packet must survive discarded traffic")
    };
    assert_eq!(frame.universe, 2);
    assert_eq!(frame.slots, [31, 32]);
    assert!(matches!(
        receive_artnet(&socket, 2, &mut packet).unwrap(),
        ReceivePoll::Empty
    ));
}

fn sacn_packet(universe: u16, start_code: u8, priority: u8, values: &[u8]) -> Vec<u8> {
    let mut source = sacn::Source::new(sacn::SourceConfig::new(
        sacn::Cid::from_bytes([2; 16]),
        "admission fixture",
    ));
    let universe = sacn::Universe::new(universe).unwrap();
    source.add_universe(sacn::UniverseConfig::new(universe)).unwrap();
    source.update_levels(universe, values);
    let mut poll = source.poll(sacn::time::Instant::EPOCH);
    while let Some(transmission) = poll.next_transmission() {
        if !matches!(transmission.route, sacn::source::Route::Universe(_)) {
            continue;
        }
        let mut packet = Packet::parse(transmission.data).unwrap();
        let Payload::Data(ref mut data) = packet.payload else {
            panic!("expected DMX data");
        };
        data.start_code = start_code;
        data.priority = priority;
        return packet.to_vec().unwrap();
    }
    panic!("source must emit fixture data");
}

#[test]
fn sacn_rejects_other_universes_and_start_codes_but_accepts_priority_zero() {
    let socket = receiver();
    let destination = socket.local_addr().unwrap();
    let sender = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let mut packet = [0; UDP_PACKET_CAPACITY];
    sender.send_to(&sacn_packet(2, 0, 100, &[17]), destination).unwrap();
    assert!(matches!(
        received(|| receive_sacn(Some(&socket), 1, &mut packet)),
        ReceivePoll::Ignored
    ));
    sender.send_to(&sacn_packet(1, 0xdd, 100, &[200]), destination).unwrap();
    assert!(matches!(
        received(|| receive_sacn(Some(&socket), 1, &mut packet)),
        ReceivePoll::Ignored
    ));
    sender.send_to(&sacn_packet(1, 0, 0, &[19, 20]), destination).unwrap();
    let ReceivePoll::Frame(frame) = received(|| receive_sacn(Some(&socket), 1, &mut packet)) else {
        panic!("valid zero-priority DMX packet must be accepted")
    };
    assert_eq!(frame.priority, 0);
    assert_eq!(frame.slots, [19, 20]);
}

#[test]
fn sacn_bad_and_oversized_traffic_does_not_retire_the_socket() {
    let socket = receiver();
    let destination = socket.local_addr().unwrap();
    let sender = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let mut packet = [0; UDP_PACKET_CAPACITY];
    sender.send_to(b"not an E1.31 packet", destination).unwrap();
    assert!(matches!(
        received(|| receive_sacn(Some(&socket), 1, &mut packet)),
        ReceivePoll::Invalid(_)
    ));
    sender.send_to(&vec![0; 32_000], destination).unwrap();
    assert!(matches!(
        received(|| receive_sacn(Some(&socket), 1, &mut packet)),
        ReceivePoll::Invalid(_)
    ));
    sender.send_to(&sacn_packet(1, 0, 201, &[42]), destination).unwrap();
    assert!(matches!(
        received(|| receive_sacn(Some(&socket), 1, &mut packet)),
        ReceivePoll::Invalid(_)
    ));
    sender.send_to(&sacn_packet(1, 0, 100, &[42]), destination).unwrap();
    assert!(matches!(
        received(|| receive_sacn(Some(&socket), 1, &mut packet)),
        ReceivePoll::Frame(_)
    ));
}
