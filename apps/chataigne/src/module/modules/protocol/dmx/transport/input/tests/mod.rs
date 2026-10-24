use super::*;
use artnet_protocol::{ArtCommand, Output};
use std::net::Ipv4Addr;

mod artnet_cases;
mod sacn_cases;

pub(crate) fn config(protocol: DmxProtocol) -> DmxTransportConfig {
    DmxTransportConfig {
        protocol,
        source_id: [1; 16],
        bind_ip: Ipv4Addr::LOCALHOST.into(),
        listen_port: 45678,
        receive_enabled: true,
        universe: 1,
        destination: Some((Ipv4Addr::LOCALHOST, 45679).into()),
    }
}

fn address(source: u8) -> SocketAddr {
    (Ipv4Addr::new(127, 0, 0, source), 45678).into()
}

pub(crate) fn artnet_packet(universe: u16, sequence: u8, physical: u8, levels: &[u8]) -> Vec<u8> {
    ArtCommand::Output(Output {
        port_address: (universe - 1).try_into().unwrap(),
        sequence,
        physical,
        data: levels.to_vec().into(),
        ..Output::default()
    })
    .write_to_buffer()
    .unwrap()
}

pub(crate) fn sacn_packet(source: u8, sequence: u8, priority: u8, levels: &[u8]) -> Vec<u8> {
    let mut source_core = ::sacn::Source::new(::sacn::SourceConfig::new(
        ::sacn::Cid::from_bytes([source; 16]),
        "receive fixture",
    ));
    let universe = ::sacn::Universe::new(1).unwrap();
    source_core.add_universe(::sacn::UniverseConfig::new(universe)).unwrap();
    source_core.update_levels(universe, levels);
    let mut poll = source_core.poll(::sacn::time::Instant::EPOCH);
    while let Some(transmission) = poll.next_transmission() {
        if !matches!(transmission.route, ::sacn::source::Route::Universe(_)) {
            continue;
        }
        let mut packet = ::sacn::Packet::parse(transmission.data).unwrap();
        let ::sacn::packet::Payload::Data(ref mut data) = packet.payload else {
            panic!("source emitted non-data");
        };
        data.sequence_number = ::sacn::SequenceNumber::new(sequence);
        data.priority = priority;
        return packet.to_vec().unwrap();
    }
    panic!("source did not emit data");
}

#[track_caller]
fn frame(poll: ReceivePoll) -> InputFrame {
    let ReceivePoll::Frame(frame) = poll else {
        panic!("expected received frame");
    };
    frame
}
