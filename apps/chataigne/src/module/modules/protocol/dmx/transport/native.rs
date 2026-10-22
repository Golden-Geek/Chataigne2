use super::super::frame::{DmxFrame, DMX_SLOT_COUNT};
use super::{DmxProtocol, DmxTransportConfig};
use artnet_protocol::{ArtCommand, Output};
use sacn::{
    packet::{
        universe_to_ipv4_multicast_addr, universe_to_ipv6_multicast_addr, AcnRootLayerProtocol, E131RootLayerData,
        ACN_SDT_MULTICAST_PORT,
    },
    source::SacnSource,
};
use std::{
    collections::BTreeSet,
    io,
    net::{IpAddr, SocketAddr, UdpSocket},
};
const SACN_PACKET_CAPACITY: usize = 1144;
pub(super) enum WorkerEndpoint {
    ArtNet {
        socket: UdpSocket,
    },
    Sacn {
        source: SacnSource,
        receiver: Option<UdpSocket>,
        listen_universe: u16,
        registered_universes: BTreeSet<u16>,
    },
}

impl WorkerEndpoint {
    pub(super) fn open(config: &DmxTransportConfig) -> Result<Self, String> {
        match config.protocol {
            DmxProtocol::ArtNet => {
                let bind_port = if config.receive_enabled { config.listen_port } else { 0 };
                let socket = UdpSocket::bind(SocketAddr::new(config.bind_ip, bind_port))
                    .map_err(|error| format!("failed to bind Art-Net UDP socket: {error}"))?;
                socket
                    .set_nonblocking(true)
                    .map_err(|error| format!("failed to configure Art-Net socket: {error}"))?;
                socket
                    .set_broadcast(true)
                    .map_err(|error| format!("failed to enable Art-Net broadcast: {error}"))?;
                Ok(Self::ArtNet { socket })
            }
            DmxProtocol::Sacn => {
                let mut source = SacnSource::with_ip("Chataigne2", SocketAddr::new(config.bind_ip, 0))
                    .map_err(|error| format!("failed to create sACN source: {error}"))?;
                if config.destination.is_some() {
                    source.set_is_sending_discovery(false);
                }
                // sacn 0.11 terminates via multicast even after unicast sends.
                // Contain loopback tests without skipping native destruction.
                #[cfg(test)]
                if config.bind_ip.is_loopback() && config.bind_ip.is_ipv4() {
                    source.set_multicast_ttl(0).map_err(|error| error.to_string())?;
                    source.set_multicast_loop_v4(false).map_err(|error| error.to_string())?;
                }
                let receiver = open_sacn_receiver(config)?;
                Ok(Self::Sacn {
                    source,
                    receiver,
                    listen_universe: config.universe,
                    registered_universes: BTreeSet::new(),
                })
            }
        }
    }

    pub(super) fn send(&mut self, frame: &DmxFrame, destination: Option<SocketAddr>) -> Result<(), String> {
        match self {
            Self::ArtNet { socket } => send_artnet(socket, frame, destination),
            Self::Sacn {
                source,
                registered_universes,
                ..
            } => send_sacn(source, registered_universes, frame, destination),
        }
    }

    pub(super) fn receive(&mut self) -> Result<Option<DmxFrame>, String> {
        match self {
            Self::ArtNet { socket } => receive_artnet(socket),
            Self::Sacn {
                receiver,
                listen_universe,
                ..
            } => receive_sacn(receiver.as_ref(), *listen_universe),
        }
    }

    pub(super) fn shutdown(&mut self) {
        if let Self::Sacn {
            source,
            registered_universes,
            ..
        } = self
        {
            for universe in std::mem::take(registered_universes) {
                let _ = source.terminate_stream(universe, 0);
            }
        }
    }
}

fn open_sacn_receiver(config: &DmxTransportConfig) -> Result<Option<UdpSocket>, String> {
    if !config.receive_enabled {
        return Ok(None);
    }

    let socket = UdpSocket::bind(SocketAddr::new(config.bind_ip, config.listen_port))
        .map_err(|error| format!("failed to bind sACN UDP socket: {error}"))?;
    socket
        .set_nonblocking(true)
        .map_err(|error| format!("failed to configure sACN socket: {error}"))?;
    if config.listen_port == ACN_SDT_MULTICAST_PORT {
        join_sacn_multicast(&socket, config.bind_ip, config.universe)?;
    }
    Ok(Some(socket))
}

fn join_sacn_multicast(socket: &UdpSocket, bind_ip: IpAddr, universe: u16) -> Result<(), String> {
    match bind_ip {
        IpAddr::V4(interface) => {
            let multicast = universe_to_ipv4_multicast_addr(universe)
                .map_err(|error| format!("failed to resolve sACN multicast address: {error}"))?
                .as_socket()
                .ok_or_else(|| "sACN IPv4 multicast address was not an IP socket".to_string())?;
            let SocketAddr::V4(multicast) = multicast else {
                return Err("sACN IPv4 multicast address used the wrong IP family".to_string());
            };
            socket
                .join_multicast_v4(multicast.ip(), &interface)
                .map_err(|error| format!("failed to join sACN multicast universe: {error}"))
        }
        IpAddr::V6(_) => {
            let multicast = universe_to_ipv6_multicast_addr(universe)
                .map_err(|error| format!("failed to resolve sACN multicast address: {error}"))?
                .as_socket()
                .ok_or_else(|| "sACN IPv6 multicast address was not an IP socket".to_string())?;
            let SocketAddr::V6(multicast) = multicast else {
                return Err("sACN IPv6 multicast address used the wrong IP family".to_string());
            };
            socket
                .join_multicast_v6(multicast.ip(), 0)
                .map_err(|error| format!("failed to join sACN multicast universe: {error}"))
        }
    }
}

fn send_artnet(socket: &UdpSocket, frame: &DmxFrame, destination: Option<SocketAddr>) -> Result<(), String> {
    let destination = destination.ok_or_else(|| "Art-Net destination is missing".to_string())?;
    let port_address = (frame.universe - 1)
        .try_into()
        .map_err(|error| format!("invalid Art-Net universe: {error}"))?;
    let output = Output {
        sequence: frame.sequence,
        port_address,
        data: frame.slots.clone().into(),
        ..Output::default()
    };
    let packet = ArtCommand::Output(output)
        .write_to_buffer()
        .map_err(|error| format!("failed to encode ArtDMX frame: {error}"))?;
    socket
        .send_to(packet.as_slice(), destination)
        .map_err(|error| format!("failed to send ArtDMX frame: {error}"))?;
    Ok(())
}

fn receive_artnet(socket: &UdpSocket) -> Result<Option<DmxFrame>, String> {
    let mut bytes = [0_u8; 1_024];
    let length = match socket.recv_from(&mut bytes) {
        Ok((length, _)) => length,
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(None),
        Err(error) => return Err(format!("failed to receive Art-Net packet: {error}")),
    };
    let command = ArtCommand::from_buffer(&bytes[..length])
        .map_err(|error| format!("failed to decode Art-Net packet: {error}"))?;
    let ArtCommand::Output(output) = command else {
        return Ok(None);
    };
    let universe = u16::from(output.port_address).saturating_add(1);
    let slots = output.data.as_ref().iter().copied().take(DMX_SLOT_COUNT).collect();
    DmxFrame::with_metadata(universe, output.sequence, 100, slots).map(Some)
}

fn send_sacn(
    source: &mut SacnSource,
    registered_universes: &mut BTreeSet<u16>,
    frame: &DmxFrame,
    destination: Option<SocketAddr>,
) -> Result<(), String> {
    if registered_universes.insert(frame.universe) {
        source
            .register_universe(frame.universe)
            .map_err(|error| format!("failed to register sACN universe: {error}"))?;
    }
    let mut payload = Vec::with_capacity(frame.slots.len() + 1);
    payload.push(0);
    payload.extend_from_slice(frame.slots.as_slice());
    source
        .send(
            &[frame.universe],
            payload.as_slice(),
            Some(frame.priority),
            destination,
            None,
        )
        .map_err(|error| format!("failed to send sACN frame: {error}"))
}

fn receive_sacn(receiver: Option<&UdpSocket>, listen_universe: u16) -> Result<Option<DmxFrame>, String> {
    let Some(receiver) = receiver else {
        return Ok(None);
    };
    let mut bytes = [0_u8; SACN_PACKET_CAPACITY];
    let length = match receiver.recv_from(&mut bytes) {
        Ok((length, _)) => length,
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(None),
        Err(error) => return Err(format!("failed to receive sACN packet: {error}")),
    };
    let packet = AcnRootLayerProtocol::parse(&bytes[..length])
        .map_err(|error| format!("failed to decode sACN packet: {error}"))?;
    let E131RootLayerData::DataPacket(frame) = packet.pdu.data else {
        return Ok(None);
    };
    if frame.universe != listen_universe
        || frame.preview_data
        || frame.stream_terminated
        || frame.synchronization_address != 0
    {
        return Ok(None);
    }
    let mut slots = frame.data.property_values.into_owned();
    if slots.first() == Some(&0) {
        slots.remove(0);
    }
    slots.truncate(DMX_SLOT_COUNT);
    DmxFrame::with_metadata(frame.universe, frame.sequence_number, frame.priority, slots).map(Some)
}
