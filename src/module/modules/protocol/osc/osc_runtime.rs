use std::{
    net::{SocketAddr, ToSocketAddrs, UdpSocket},
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle},
    time::Duration,
};

use crate::app::module::common::pending_channel::{pending_channel, PendingReceiver, PendingSender};

use rosc::{decoder, encoder};

use super::osc_message::{decode_packet_messages, encode_packet, OscDecodedMessage, OscValuePayload};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OscTransportConfig {
    pub bind_interface_host: String,
    pub bind_port: u16,
    pub receive_enabled: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OscOutboundMessage {
    pub address: String,
    pub payload: OscValuePayload,
    pub remote_host: String,
    pub remote_port: u16,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum OscWorkerEvent {
    Message(OscDecodedMessage),
    Error(String),
}

pub(crate) struct OscTransportHandle {
    command_tx: Sender<OscWorkerCommand>,
    event_rx: PendingReceiver<OscWorkerEvent>,
    worker: Option<JoinHandle<()>>,
}

impl OscTransportHandle {
    pub(crate) fn spawn(config: OscTransportConfig) -> Result<Self, String> {
        let bind_address = resolve_socket_addr(config.bind_interface_host.as_str(), config.bind_port)?;

        let socket = UdpSocket::bind(bind_address)
            .map_err(|error| format!("failed to bind OSC socket on {bind_address}: {error}"))?;
        socket
            .set_nonblocking(true)
            .map_err(|error| format!("failed to set OSC socket to non-blocking mode: {error}"))?;

        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = pending_channel();

        let worker = thread::Builder::new()
            .name(format!("osc-module-{}", bind_address.port()))
            .spawn(move || worker_loop(socket, config.receive_enabled, command_rx, event_tx))
            .map_err(|error| format!("failed to start OSC worker thread: {error}"))?;

        Ok(Self {
            command_tx,
            event_rx,
            worker: Some(worker),
        })
    }

    pub(crate) fn send(&self, message: OscOutboundMessage) -> Result<(), String> {
        self.command_tx
            .send(OscWorkerCommand::Send(message))
            .map_err(|_| "OSC worker is no longer running".to_string())
    }

    pub(crate) fn try_recv(&self) -> Result<OscWorkerEvent, mpsc::TryRecvError> {
        self.event_rx.try_recv()
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.event_rx.has_pending()
    }

    pub(crate) fn clear_pending(&self) {
        self.event_rx.clear_pending();
    }

    pub(crate) fn stop(&mut self) {
        let _ = self.command_tx.send(OscWorkerCommand::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for OscTransportHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

enum OscWorkerCommand {
    Send(OscOutboundMessage),
    Stop,
}

fn worker_loop(
    socket: UdpSocket,
    receive_enabled: bool,
    command_rx: Receiver<OscWorkerCommand>,
    event_tx: PendingSender<OscWorkerEvent>,
) {
    let mut buffer = [0u8; 65_535];

    loop {
        if drain_commands(&command_rx, &event_tx, &socket) {
            break;
        }

        if receive_enabled {
            loop {
                match socket.recv_from(&mut buffer) {
                    Ok((length, _source)) => match decoder::decode_udp(&buffer[..length]) {
                        Ok((_remaining, packet)) => {
                            for decoded in decode_packet_messages(packet) {
                                match decoded {
                                    Ok(message) => {
                                        if event_tx.send(OscWorkerEvent::Message(message)).is_err() {
                                            return;
                                        }
                                    }
                                    Err(error) => {
                                        if event_tx.send(OscWorkerEvent::Error(error)).is_err() {
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                        Err(error) => {
                            if event_tx
                                .send(OscWorkerEvent::Error(format!("failed to decode OSC packet: {error}")))
                                .is_err()
                            {
                                return;
                            }
                        }
                    },
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(error) if should_ignore_receive_error(&error) => break,
                    Err(error) => {
                        if event_tx
                            .send(OscWorkerEvent::Error(format!("OSC socket receive error: {error}")))
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
            Ok(OscWorkerCommand::Send(message)) => {
                send_message(&socket, &event_tx, message);
            }
            Ok(OscWorkerCommand::Stop) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn drain_commands(
    command_rx: &Receiver<OscWorkerCommand>,
    event_tx: &PendingSender<OscWorkerEvent>,
    socket: &UdpSocket,
) -> bool {
    loop {
        match command_rx.try_recv() {
            Ok(OscWorkerCommand::Send(message)) => send_message(socket, event_tx, message),
            Ok(OscWorkerCommand::Stop) => return true,
            Err(mpsc::TryRecvError::Empty) => return false,
            Err(mpsc::TryRecvError::Disconnected) => return true,
        }
    }
}

fn should_ignore_receive_error(error: &std::io::Error) -> bool {
    error.raw_os_error() == Some(10054) || (cfg!(windows) && error.kind() == std::io::ErrorKind::ConnectionReset)
}

fn send_message(socket: &UdpSocket, event_tx: &PendingSender<OscWorkerEvent>, message: OscOutboundMessage) {
    let packet = match encode_packet(message.address.as_str(), &message.payload) {
        Ok(packet) => packet,
        Err(error) => {
            let _ = event_tx.send(OscWorkerEvent::Error(error));
            return;
        }
    };

    let bytes = match encoder::encode(&packet) {
        Ok(bytes) => bytes,
        Err(error) => {
            let _ = event_tx.send(OscWorkerEvent::Error(format!("failed to encode OSC packet: {error}")));
            return;
        }
    };

    let remote_address = match resolve_socket_addr(message.remote_host.as_str(), message.remote_port) {
        Ok(address) => address,
        Err(error) => {
            let _ = event_tx.send(OscWorkerEvent::Error(error));
            return;
        }
    };

    if let Err(error) = socket.send_to(bytes.as_slice(), remote_address) {
        let _ = event_tx.send(OscWorkerEvent::Error(format!("failed to send OSC packet: {error}")));
    }
}

fn resolve_socket_addr(host: &str, port: u16) -> Result<SocketAddr, String> {
    let address = format!("{}:{}", host.trim(), port);
    let mut resolved = address
        .to_socket_addrs()
        .map_err(|error| format!("failed to resolve OSC address '{address}': {error}"))?;

    resolved
        .next()
        .ok_or_else(|| format!("OSC address '{address}' did not resolve to a socket address"))
}

#[cfg(test)]
mod tests;
