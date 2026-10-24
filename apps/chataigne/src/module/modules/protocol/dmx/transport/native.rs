use super::super::frame::DmxFrame;
use super::{
    DmxProtocol, DmxTransportConfig, ReceivePoll, continuity::OutputSession, input::Receiver, worker::StopReason,
};
use std::{
    io,
    net::{IpAddr, SocketAddr, UdpSocket},
    time::{Duration, Instant},
};

// Whole UDP input avoids Windows WSAEMSGSIZE before protocol admission.
const UDP_PACKET_CAPACITY: usize = 65_536;

#[cfg(test)]
mod tests;

pub(super) struct WorkerEndpoint {
    socket: UdpSocket,
    receiver: Option<UdpSocket>,
    protocol: DmxProtocol,
    listen_universe: u16,
    bind_ip: IpAddr,
    multicast: bool,
    input: Option<Receiver>,
    sync_groups: Vec<u16>,
    packet: Box<[u8; UDP_PACKET_CAPACITY]>,
    output: OutputSession,
    origin: Instant,
}

impl WorkerEndpoint {
    pub(super) fn open(config: &DmxTransportConfig, output: OutputSession) -> Result<Self, String> {
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
            bind_ip: config.bind_ip,
            multicast: config.listen_port == sacn::proto::SACN_PORT,
            input: config.receive_enabled.then(|| Receiver::new(config)),
            sync_groups: Vec::new(),
            packet: Box::new([0; UDP_PACKET_CAPACITY]),
            output,
            origin: Instant::now(),
        })
    }

    pub(super) fn send(&mut self, frame: &DmxFrame) -> Result<(), String> {
        let socket = &self.socket;
        self.output.send(frame, Instant::now(), &mut |bytes, destination| {
            send(socket, bytes, destination)
        })
    }

    pub(super) fn command_at(&self) -> Instant {
        self.output.command_at(Instant::now())
    }
    pub(super) fn deadline(&self) -> Option<Instant> {
        self.output.deadline(Instant::now())
    }

    pub(super) fn service(&mut self) -> Result<(), String> {
        let socket = &self.socket;
        self.output.poll(Instant::now(), &mut |bytes, destination| {
            send(socket, bytes, destination)
        })?;
        Ok(())
    }

    pub(super) fn receive(&mut self) -> Result<ReceivePoll, String> {
        let Some(input) = self.input.as_mut() else {
            return Ok(ReceivePoll::Empty);
        };
        let socket = match self.protocol {
            DmxProtocol::ArtNet => Some(&self.socket),
            DmxProtocol::Sacn => self.receiver.as_ref(),
        };
        let result = receive(socket, input, self.packet.as_mut(), self.origin.elapsed());
        self.update_sync_groups()?;
        result
    }

    fn update_sync_groups(&mut self) -> Result<(), String> {
        if !self.multicast {
            return Ok(());
        }
        let Some(socket) = self.receiver.as_ref() else {
            return Ok(());
        };
        let desired = self.input.as_ref().map_or_else(Vec::new, Receiver::sync_groups);
        // Remove old memberships before joining replacements: eight dynamic groups plus
        // the configured data group is a bound through churn, not just after it settles.
        reconcile_memberships(
            &mut self.sync_groups,
            &desired,
            self.listen_universe,
            |universe, join| membership(socket, self.bind_ip, universe, join),
        )
    }

    pub(super) fn shutdown(&mut self, reason: StopReason) {
        // Retirement retains this native slot. The engine never waits for these timed sends.
        if reason == StopReason::Interrupted || !self.output.terminate() {
            return;
        }
        let limit = Instant::now() + Duration::from_millis(200);
        while let Some(deadline) = self.deadline() {
            if deadline > limit || Instant::now() >= limit {
                break;
            }
            std::thread::sleep(deadline.saturating_duration_since(Instant::now()));
            if Instant::now() >= limit {
                break;
            }
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
        membership(&socket, config.bind_ip, config.universe, true)?;
    }
    Ok(Some(socket))
}

fn membership(socket: &UdpSocket, bind_ip: IpAddr, universe: u16, join: bool) -> Result<(), String> {
    let result = match bind_ip {
        IpAddr::V4(interface) => {
            if join {
                socket.join_multicast_v4(&sacn::ipv4_multicast(universe), &interface)
            } else {
                socket.leave_multicast_v4(&sacn::ipv4_multicast(universe), &interface)
            }
        }
        IpAddr::V6(_) => {
            if join {
                socket.join_multicast_v6(&sacn::ipv6_multicast(universe), 0)
            } else {
                socket.leave_multicast_v6(&sacn::ipv6_multicast(universe), 0)
            }
        }
    };
    result.map_err(|error| format!("failed to update sACN multicast membership: {error}"))
}

fn reconcile_memberships(
    joined: &mut Vec<u16>,
    desired: &[u16],
    data_universe: u16,
    mut update: impl FnMut(u16, bool) -> Result<(), String>,
) -> Result<(), String> {
    if desired.len() > super::input::MAX_SOURCES {
        return Err("sACN sync membership limit exceeded".into());
    }
    for index in (0..joined.len()).rev() {
        let universe = joined[index];
        if !desired.contains(&universe) {
            update(universe, false)?;
            joined.remove(index);
        }
    }
    for &universe in desired {
        if universe != data_universe && !joined.contains(&universe) {
            update(universe, true)?;
            joined.push(universe);
        }
    }
    Ok(())
}

fn receive(
    receiver: Option<&UdpSocket>,
    input: &mut Receiver,
    bytes: &mut [u8],
    now: Duration,
) -> Result<ReceivePoll, String> {
    if let Some(frame) = input.poll(now) {
        return Ok(ReceivePoll::Frame(frame));
    }
    let Some(receiver) = receiver else {
        return Ok(ReceivePoll::Empty);
    };
    let (length, from) = match receiver.recv_from(bytes) {
        Ok(datagram) => datagram,
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(ReceivePoll::Empty),
        Err(error) => return Err(format!("failed to receive DMX packet: {error}")),
    };
    Ok(input.handle(now, from, &bytes[..length]))
}
