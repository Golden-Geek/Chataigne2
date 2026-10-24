use super::super::frame::{DMX_SLOT_COUNT, DmxFrame};
use super::{DmxProtocol, DmxTransportConfig, ReceivePoll, output::ClockedOutput};
use artnet_protocol::ArtCommand;
use sacn::{Packet, packet::Payload};
use std::{
    io,
    net::{IpAddr, SocketAddr, UdpSocket},
    time::{Duration, Instant},
};

const SACN_PACKET_CAPACITY: usize = sacn::packet::MAX_PACKET_SIZE;
const ARTNET_PACKET_CAPACITY: usize = 1024;
// Whole UDP input avoids Windows WSAEMSGSIZE before protocol admission.
const UDP_PACKET_CAPACITY: usize = 65_536;

#[cfg(test)]
mod tests;

pub(super) struct WorkerEndpoint {
    socket: UdpSocket,
    receiver: Option<UdpSocket>,
    protocol: DmxProtocol,
    listen_universe: u16,
    packet: Box<[u8; UDP_PACKET_CAPACITY]>,
    output: ClockedOutput,
    origin: Instant,
}

impl WorkerEndpoint {
    pub(super) fn open(config: &DmxTransportConfig) -> Result<Self, String> {
        let port = if config.protocol == DmxProtocol::ArtNet && config.receive_enabled {
            config.listen_port
        } else {
            0
        };
        let socket = UdpSocket::bind(SocketAddr::new(config.bind_ip, port))
            .map_err(|error| format!("failed to bind DMX output socket: {error}"))?;
        socket.set_nonblocking(true).map_err(|error| error.to_string())?;
        if config.protocol == DmxProtocol::ArtNet {
            socket.set_broadcast(true).map_err(|error| error.to_string())?;
        }
        let receiver = if config.protocol == DmxProtocol::Sacn {
            open_sacn_receiver(config)?
        } else {
            None
        };
        Ok(Self {
            socket,
            receiver,
            protocol: config.protocol,
            listen_universe: config.universe,
            packet: Box::new([0; UDP_PACKET_CAPACITY]),
            output: ClockedOutput::new(config),
            origin: Instant::now(),
        })
    }

    pub(super) fn send(&mut self, frame: &DmxFrame) -> Result<(), String> {
        let socket = &self.socket;
        self.output
            .send(frame, self.origin.elapsed(), &mut |bytes, destination| {
                send(socket, bytes, destination)
            })
    }

    pub(super) fn command_at(&self) -> Instant {
        self.origin + self.output.command_at()
    }
    pub(super) fn deadline(&self) -> Option<Instant> {
        self.output.deadline().map(|deadline| self.origin + deadline)
    }

    pub(super) fn service(&mut self) -> Result<(), String> {
        let socket = &self.socket;
        self.output.poll(self.origin.elapsed(), &mut |bytes, destination| {
            send(socket, bytes, destination)
        })?;
        Ok(())
    }

    pub(super) fn receive(&mut self) -> Result<ReceivePoll, String> {
        match self.protocol {
            DmxProtocol::ArtNet => receive_artnet(&self.socket, self.listen_universe, self.packet.as_mut()),
            DmxProtocol::Sacn => receive_sacn(self.receiver.as_ref(), self.listen_universe, self.packet.as_mut()),
        }
    }

    pub(super) fn shutdown(&mut self) {
        // Retirement retains this native slot. The engine never waits for these timed sends.
        self.output.terminate();
        let limit = Instant::now() + Duration::from_millis(200);
        while let Some(deadline) = self.deadline() {
            if deadline > limit || Instant::now() >= limit {
                break;
            }
            std::thread::sleep(deadline.saturating_duration_since(Instant::now()));
            if self.service().is_err() {
                break;
            }
        }
    }
}

fn send(socket: &UdpSocket, bytes: &[u8], destination: SocketAddr) -> Result<(), String> {
    let length = socket
        .send_to(bytes, destination)
        .map_err(|error| format!("failed to submit DMX datagram: {error}"))?;
    if length != bytes.len() {
        return Err("DMX socket reported a partial datagram submission".into());
    }
    Ok(())
}

fn open_sacn_receiver(config: &DmxTransportConfig) -> Result<Option<UdpSocket>, String> {
    if !config.receive_enabled {
        return Ok(None);
    }
    let socket = UdpSocket::bind(SocketAddr::new(config.bind_ip, config.listen_port))
        .map_err(|error| format!("failed to bind sACN UDP socket: {error}"))?;
    socket.set_nonblocking(true).map_err(|error| error.to_string())?;
    if config.listen_port == sacn::proto::SACN_PORT {
        match config.bind_ip {
            IpAddr::V4(interface) => socket.join_multicast_v4(&sacn::ipv4_multicast(config.universe), &interface),
            IpAddr::V6(_) => socket.join_multicast_v6(&sacn::ipv6_multicast(config.universe), 0),
        }
        .map_err(|error| format!("failed to join sACN multicast universe: {error}"))?;
    }
    Ok(Some(socket))
}

fn receive_artnet(socket: &UdpSocket, listen_universe: u16, bytes: &mut [u8]) -> Result<ReceivePoll, String> {
    let length = match socket.recv_from(bytes) {
        Ok((length, _)) => length,
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(ReceivePoll::Empty),
        Err(error) => return Err(format!("failed to receive Art-Net packet: {error}")),
    };
    if length > ARTNET_PACKET_CAPACITY {
        return Ok(ReceivePoll::Invalid("Oversized Art-Net packet ignored"));
    }
    let command = match ArtCommand::from_buffer(&bytes[..length]) {
        Ok(command) => command,
        Err(_) => return Ok(ReceivePoll::Invalid("Malformed Art-Net packet ignored")),
    };
    let ArtCommand::Output(output) = command else {
        return Ok(ReceivePoll::Ignored);
    };
    let universe = u16::from(output.port_address).saturating_add(1);
    if universe != listen_universe {
        return Ok(ReceivePoll::Ignored);
    }
    if output.data.as_ref().len() > DMX_SLOT_COUNT {
        return Ok(ReceivePoll::Invalid("ArtDMX data exceeds 512 channels"));
    }
    DmxFrame::with_metadata(universe, output.sequence, 100, output.data.as_ref().to_vec()).map(ReceivePoll::Frame)
}

fn receive_sacn(receiver: Option<&UdpSocket>, listen_universe: u16, bytes: &mut [u8]) -> Result<ReceivePoll, String> {
    let Some(receiver) = receiver else {
        return Ok(ReceivePoll::Empty);
    };
    let length = match receiver.recv_from(bytes) {
        Ok((length, _)) => length,
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(ReceivePoll::Empty),
        Err(error) => return Err(format!("failed to receive sACN packet: {error}")),
    };
    if length > SACN_PACKET_CAPACITY {
        return Ok(ReceivePoll::Invalid("Oversized sACN packet ignored"));
    }
    let packet = match Packet::parse(&bytes[..length]) {
        Ok(packet) => packet,
        Err(_) => return Ok(ReceivePoll::Invalid("Malformed sACN packet ignored")),
    };
    let Payload::Data(frame) = packet.payload else {
        return Ok(ReceivePoll::Ignored);
    };
    if frame.universe != listen_universe
        || frame.preview
        || frame.stream_terminated
        || frame.sync_address != 0
        || frame.start_code != 0
    {
        return Ok(ReceivePoll::Ignored);
    }
    if frame.priority > 200 || frame.values.len() > DMX_SLOT_COUNT {
        return Ok(ReceivePoll::Invalid("Invalid sACN priority or slot count"));
    }
    DmxFrame::with_metadata(
        frame.universe,
        frame.sequence_number.get(),
        frame.priority,
        frame.values.to_vec(),
    )
    .map(ReceivePoll::Frame)
}
