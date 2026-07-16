use std::{
    net::{SocketAddr, ToSocketAddrs, UdpSocket},
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle},
    time::Duration,
};

use golden_io::{pending_channel, PendingReceiver, PendingSender};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UdpStreamingTransportConfig {
    pub bind_interface_host: String,
    pub bind_port: u16,
    pub receive_enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UdpStreamingOutboundPacket {
    pub bytes: Vec<u8>,
    pub remote_host: String,
    pub remote_port: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum StreamingWorkerEvent {
    Bytes(Vec<u8>),
    Error(String),
}

pub(crate) struct UdpStreamingTransportHandle {
    command_tx: Sender<UdpStreamingWorkerCommand>,
    event_rx: PendingReceiver<StreamingWorkerEvent>,
    worker: Option<JoinHandle<()>>,
}

impl UdpStreamingTransportHandle {
    pub(crate) fn spawn(config: UdpStreamingTransportConfig) -> Result<Self, String> {
        let bind_address = resolve_socket_addr(config.bind_interface_host.as_str(), config.bind_port)?;

        let socket = UdpSocket::bind(bind_address)
            .map_err(|error| format!("failed to bind UDP socket on {bind_address}: {error}"))?;
        socket
            .set_nonblocking(true)
            .map_err(|error| format!("failed to set UDP socket to non-blocking mode: {error}"))?;

        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = pending_channel();

        let worker = thread::Builder::new()
            .name(format!("streaming-udp-{}", bind_address.port()))
            .spawn(move || worker_loop(socket, config.receive_enabled, command_rx, event_tx))
            .map_err(|error| format!("failed to start UDP worker thread: {error}"))?;

        Ok(Self {
            command_tx,
            event_rx,
            worker: Some(worker),
        })
    }

    pub(crate) fn send(&self, packet: UdpStreamingOutboundPacket) -> Result<(), String> {
        self.command_tx
            .send(UdpStreamingWorkerCommand::Send(packet))
            .map_err(|_| "UDP worker is no longer running".to_string())
    }

    pub(crate) fn try_recv(&self) -> Result<StreamingWorkerEvent, mpsc::TryRecvError> {
        self.event_rx.try_recv()
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.event_rx.has_pending()
    }

    pub(crate) fn clear_pending(&self) {
        self.event_rx.clear_pending();
    }

    pub(crate) fn stop(&mut self) {
        let _ = self.command_tx.send(UdpStreamingWorkerCommand::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for UdpStreamingTransportHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

enum UdpStreamingWorkerCommand {
    Send(UdpStreamingOutboundPacket),
    Stop,
}

fn worker_loop(
    socket: UdpSocket,
    receive_enabled: bool,
    command_rx: Receiver<UdpStreamingWorkerCommand>,
    event_tx: PendingSender<StreamingWorkerEvent>,
) {
    let mut buffer = [0u8; 65_535];

    loop {
        if drain_commands(&command_rx, &event_tx, &socket) {
            break;
        }

        if receive_enabled {
            loop {
                match socket.recv_from(&mut buffer) {
                    Ok((length, _source)) => {
                        if event_tx
                            .send(StreamingWorkerEvent::Bytes(buffer[..length].to_vec()))
                            .is_err()
                        {
                            return;
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(error) if should_ignore_receive_error(&error) => break,
                    Err(error) => {
                        if event_tx
                            .send(StreamingWorkerEvent::Error(format!("UDP socket receive error: {error}")))
                            .is_err()
                        {
                            return;
                        }
                        break;
                    }
                }
            }
        }

        match command_rx.recv_timeout(Duration::from_millis(5)) {
            Ok(UdpStreamingWorkerCommand::Send(packet)) => send_packet(&socket, &event_tx, packet),
            Ok(UdpStreamingWorkerCommand::Stop) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn drain_commands(
    command_rx: &Receiver<UdpStreamingWorkerCommand>,
    event_tx: &PendingSender<StreamingWorkerEvent>,
    socket: &UdpSocket,
) -> bool {
    loop {
        match command_rx.try_recv() {
            Ok(UdpStreamingWorkerCommand::Send(packet)) => send_packet(socket, event_tx, packet),
            Ok(UdpStreamingWorkerCommand::Stop) => return true,
            Err(mpsc::TryRecvError::Empty) => return false,
            Err(mpsc::TryRecvError::Disconnected) => return true,
        }
    }
}

fn should_ignore_receive_error(error: &std::io::Error) -> bool {
    error.raw_os_error() == Some(10054) || (cfg!(windows) && error.kind() == std::io::ErrorKind::ConnectionReset)
}

fn send_packet(socket: &UdpSocket, event_tx: &PendingSender<StreamingWorkerEvent>, packet: UdpStreamingOutboundPacket) {
    let remote_address = match resolve_socket_addr(packet.remote_host.as_str(), packet.remote_port) {
        Ok(address) => address,
        Err(error) => {
            let _ = event_tx.send(StreamingWorkerEvent::Error(error));
            return;
        }
    };

    if let Err(error) = socket.send_to(packet.bytes.as_slice(), remote_address) {
        let _ = event_tx.send(StreamingWorkerEvent::Error(format!("failed to send UDP packet: {error}")));
    }
}

fn resolve_socket_addr(host: &str, port: u16) -> Result<SocketAddr, String> {
    let address = format!("{}:{}", host.trim(), port);
    let mut resolved = address
        .to_socket_addrs()
        .map_err(|error| format!("failed to resolve UDP address '{address}': {error}"))?;

    resolved
        .next()
        .ok_or_else(|| format!("UDP address '{address}' did not resolve to a socket address"))
}
