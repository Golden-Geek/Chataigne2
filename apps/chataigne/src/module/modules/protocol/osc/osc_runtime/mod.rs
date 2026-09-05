use std::{
    net::{SocketAddr, ToSocketAddrs},
    num::NonZeroUsize,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    thread::{self, JoinHandle},
};

use golden_io::{pending_channel, PendingDrain, PendingReceiver, PendingSender};

use mio::{net::UdpSocket, Events, Interest, Poll, Token, Waker};
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
    pub address: Arc<str>,
    pub payload: OscValuePayload,
    pub remote_address: SocketAddr,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum OscWorkerEvent {
    Message(OscDecodedMessage),
    Error(String),
}

pub(crate) struct OscTransportHandle {
    command_tx: Sender<OscWorkerCommand>,
    event_rx: PendingReceiver<OscWorkerEvent>,
    waker: Arc<Waker>,
    worker: Option<JoinHandle<()>>,
}

const OSC_SOCKET_TOKEN: Token = Token(0);
const OSC_WAKE_TOKEN: Token = Token(1);
const OSC_EVENT_DRAIN_BUDGET: NonZeroUsize =
    NonZeroUsize::new(1_024).expect("OSC event drain budget must be nonzero");

impl OscTransportHandle {
    pub(crate) fn spawn(config: OscTransportConfig) -> Result<Self, String> {
        let bind_address = resolve_socket_addr(config.bind_interface_host.as_str(), config.bind_port)?;

        let mut socket = UdpSocket::bind(bind_address)
            .map_err(|error| format!("failed to bind OSC socket on {bind_address}: {error}"))?;
        let poll = Poll::new().map_err(|error| format!("failed to create OSC poll loop: {error}"))?;
        poll.registry()
            .register(&mut socket, OSC_SOCKET_TOKEN, Interest::READABLE)
            .map_err(|error| format!("failed to register OSC socket with poll loop: {error}"))?;
        let waker = Arc::new(
            Waker::new(poll.registry(), OSC_WAKE_TOKEN)
                .map_err(|error| format!("failed to create OSC worker waker: {error}"))?,
        );

        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = pending_channel();

        let worker_waker = Arc::clone(&waker);
        let worker = thread::Builder::new()
            .name(format!("osc-module-{}", bind_address.port()))
            .spawn(move || {
                let _keep_waker_alive = worker_waker;
                worker_loop(socket, config.receive_enabled, command_rx, event_tx, poll);
            })
            .map_err(|error| format!("failed to start OSC worker thread: {error}"))?;

        Ok(Self {
            command_tx,
            event_rx,
            waker,
            worker: Some(worker),
        })
    }

    pub(crate) fn send(&self, message: OscOutboundMessage) -> Result<(), String> {
        self.command_tx
            .send(OscWorkerCommand::Send(message))
            .map_err(|_| "OSC worker is no longer running".to_string())?;
        self.waker
            .wake()
            .map_err(|error| format!("failed to wake OSC worker: {error}"))
    }

    pub(crate) fn drain_events(&self, events: &mut Vec<OscWorkerEvent>) -> PendingDrain {
        self.event_rx.drain_into(events, OSC_EVENT_DRAIN_BUDGET)
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.event_rx.has_pending()
    }

    pub(crate) fn stop(&mut self) {
        let _ = self.command_tx.send(OscWorkerCommand::Stop);
        let _ = self.waker.wake();
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
    mut poll: Poll,
) {
    let mut buffer = [0u8; 65_535];
    let mut events = Events::with_capacity(64);

    loop {
        if drain_commands(&command_rx, &event_tx, &socket) {
            break;
        }

        match poll.poll(&mut events, None) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => {
                let _ = event_tx.send(OscWorkerEvent::Error(format!("OSC poll loop error: {error}")));
                break;
            }
        }

        for event in events.iter() {
            match event.token() {
                OSC_WAKE_TOKEN if drain_commands(&command_rx, &event_tx, &socket) => return,
                OSC_SOCKET_TOKEN
                    if receive_enabled
                        && event.is_readable()
                        && !drain_socket(&socket, &event_tx, &mut buffer) =>
                {
                    return;
                }
                _ => {}
            }
        }
    }
}

fn drain_socket(
    socket: &UdpSocket,
    event_tx: &PendingSender<OscWorkerEvent>,
    buffer: &mut [u8; 65_535],
) -> bool {
    loop {
        match socket.recv_from(buffer) {
            Ok((length, _source)) => match decoder::decode_udp(&buffer[..length]) {
                Ok((_remaining, packet)) => {
                    for decoded in decode_packet_messages(packet) {
                        match decoded {
                            Ok(message) => {
                                if event_tx.send(OscWorkerEvent::Message(message)).is_err() {
                                    return false;
                                }
                            }
                            Err(error) => {
                                if event_tx.send(OscWorkerEvent::Error(error)).is_err() {
                                    return false;
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
                        return false;
                    }
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return true,
            Err(error) if should_ignore_receive_error(&error) => return true,
            Err(error) => {
                if event_tx
                    .send(OscWorkerEvent::Error(format!("OSC socket receive error: {error}")))
                    .is_err()
                {
                    return false;
                }
                return true;
            }
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
    let packet = match encode_packet(message.address.as_ref(), &message.payload) {
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

    if let Err(error) = socket.send_to(bytes.as_slice(), message.remote_address) {
        let _ = event_tx.send(OscWorkerEvent::Error(format!("failed to send OSC packet: {error}")));
    }
}

pub(crate) fn resolve_socket_addr(host: &str, port: u16) -> Result<SocketAddr, String> {
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
