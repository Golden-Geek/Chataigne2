use std::{
    io::{Read, Write},
    net::{Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs},
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle},
    time::Duration,
};

use golden_io::{pending_channel, PendingReceiver, PendingSender};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TcpServerTransportConfig {
    pub bind_host: String,
    pub bind_port: u16,
    pub receive_enabled: bool,
    pub send_enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TcpServerWorkerEvent {
    ClientConnected { client_id: String, info: String },
    ClientDisconnected { client_id: String, reason: Option<String> },
    Bytes { client_id: String, bytes: Vec<u8> },
    Error(String),
    Stopped(String),
}

pub(crate) struct TcpServerTransportHandle {
    command_tx: Sender<TcpServerWorkerCommand>,
    event_rx: PendingReceiver<TcpServerWorkerEvent>,
    worker: Option<JoinHandle<()>>,
}

impl TcpServerTransportHandle {
    pub(crate) fn spawn(config: TcpServerTransportConfig) -> Result<Self, String> {
        let bind_address = resolve_socket_addr(config.bind_host.as_str(), config.bind_port)?;
        let listener = TcpListener::bind(bind_address)
            .map_err(|error| format!("failed to bind TCP server socket on {bind_address}: {error}"))?;
        listener
            .set_nonblocking(true)
            .map_err(|error| format!("failed to set TCP server listener to non-blocking mode: {error}"))?;

        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = pending_channel();

        let worker = thread::Builder::new()
            .name(format!("streaming-tcp-server-{}", bind_address.port()))
            .spawn(move || worker_loop(listener, config, command_rx, event_tx))
            .map_err(|error| format!("failed to start TCP server worker thread: {error}"))?;

        Ok(Self {
            command_tx,
            event_rx,
            worker: Some(worker),
        })
    }

    pub(crate) fn send(&self, bytes: Vec<u8>) -> Result<(), String> {
        self.command_tx
            .send(TcpServerWorkerCommand::Broadcast(bytes))
            .map_err(|_| "TCP server worker is no longer running".to_string())
    }

    pub(crate) fn try_recv(&self) -> Result<TcpServerWorkerEvent, mpsc::TryRecvError> {
        self.event_rx.try_recv()
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.event_rx.has_pending()
    }

    pub(crate) fn clear_pending(&self) {
        self.event_rx.clear_pending();
    }

    pub(crate) fn stop(&mut self) {
        let _ = self.command_tx.send(TcpServerWorkerCommand::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for TcpServerTransportHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

enum TcpServerWorkerCommand {
    Broadcast(Vec<u8>),
    Stop,
}

struct TcpClientConnection {
    client_id: String,
    stream: TcpStream,
}

fn worker_loop(
    listener: TcpListener,
    config: TcpServerTransportConfig,
    command_rx: Receiver<TcpServerWorkerCommand>,
    event_tx: PendingSender<TcpServerWorkerEvent>,
) {
    let mut clients = Vec::new();

    loop {
        if drain_commands(&command_rx, &event_tx, &mut clients, config.send_enabled) {
            break;
        }

        if accept_new_clients(&listener, &event_tx, &mut clients).is_err() {
            break;
        }

        if config.receive_enabled && read_client_bytes(&event_tx, &mut clients).is_err() {
            break;
        }

        match command_rx.recv_timeout(Duration::from_millis(5)) {
            Ok(TcpServerWorkerCommand::Broadcast(bytes)) => {
                if broadcast_bytes(&event_tx, &mut clients, config.send_enabled, bytes.as_slice()).is_err() {
                    break;
                }
            }
            Ok(TcpServerWorkerCommand::Stop) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    shutdown_clients(&mut clients);
}

fn drain_commands(
    command_rx: &Receiver<TcpServerWorkerCommand>,
    event_tx: &PendingSender<TcpServerWorkerEvent>,
    clients: &mut Vec<TcpClientConnection>,
    send_enabled: bool,
) -> bool {
    loop {
        match command_rx.try_recv() {
            Ok(TcpServerWorkerCommand::Broadcast(bytes)) => {
                if broadcast_bytes(event_tx, clients, send_enabled, bytes.as_slice()).is_err() {
                    return true;
                }
            }
            Ok(TcpServerWorkerCommand::Stop) => return true,
            Err(mpsc::TryRecvError::Empty) => return false,
            Err(mpsc::TryRecvError::Disconnected) => return true,
        }
    }
}

fn accept_new_clients(
    listener: &TcpListener,
    event_tx: &PendingSender<TcpServerWorkerEvent>,
    clients: &mut Vec<TcpClientConnection>,
) -> Result<(), ()> {
    loop {
        match listener.accept() {
            Ok((stream, remote_address)) => {
                if let Err(error) = stream.set_nonblocking(true) {
                    let _ = event_tx.send(TcpServerWorkerEvent::Error(format!(
                        "failed to set TCP client {remote_address} to non-blocking mode: {error}"
                    )));
                    continue;
                }

                let client_id = remote_address.to_string();
                let info = format!("Remote address: {remote_address}");
                clients.push(TcpClientConnection {
                    client_id: client_id.clone(),
                    stream,
                });
                if event_tx
                    .send(TcpServerWorkerEvent::ClientConnected { client_id, info })
                    .is_err()
                {
                    return Err(());
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
            Err(error) => {
                let _ = event_tx.send(TcpServerWorkerEvent::Stopped(format!(
                    "TCP server accept error: {error}"
                )));
                return Err(());
            }
        }
    }
}

fn read_client_bytes(
    event_tx: &PendingSender<TcpServerWorkerEvent>,
    clients: &mut Vec<TcpClientConnection>,
) -> Result<(), ()> {
    let mut client_index = 0usize;
    while client_index < clients.len() {
        let mut remove_client = false;
        let client_id = clients[client_index].client_id.clone();
        let mut buffer = [0u8; 8192];

        loop {
            match clients[client_index].stream.read(&mut buffer) {
                Ok(0) => {
                    remove_client = true;
                    if event_tx
                        .send(TcpServerWorkerEvent::ClientDisconnected {
                            client_id: client_id.clone(),
                            reason: None,
                        })
                        .is_err()
                    {
                        return Err(());
                    }
                    break;
                }
                Ok(length) => {
                    if event_tx
                        .send(TcpServerWorkerEvent::Bytes {
                            client_id: client_id.clone(),
                            bytes: buffer[..length].to_vec(),
                        })
                        .is_err()
                    {
                        return Err(());
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) if is_disconnect_error(&error) => {
                    remove_client = true;
                    if event_tx
                        .send(TcpServerWorkerEvent::ClientDisconnected {
                            client_id: client_id.clone(),
                            reason: Some(format!("TCP client receive error: {error}")),
                        })
                        .is_err()
                    {
                        return Err(());
                    }
                    break;
                }
                Err(error) => {
                    if event_tx
                        .send(TcpServerWorkerEvent::Error(format!(
                            "TCP client {client_id} receive error: {error}"
                        )))
                        .is_err()
                    {
                        return Err(());
                    }
                    break;
                }
            }
        }

        if remove_client {
            clients.remove(client_index);
        } else {
            client_index += 1;
        }
    }

    Ok(())
}

fn broadcast_bytes(
    event_tx: &PendingSender<TcpServerWorkerEvent>,
    clients: &mut Vec<TcpClientConnection>,
    send_enabled: bool,
    bytes: &[u8],
) -> Result<(), ()> {
    if !send_enabled {
        let _ = event_tx.send(TcpServerWorkerEvent::Error(
            "TCP server sender is disabled; outgoing bytes were dropped".to_string(),
        ));
        return Ok(());
    }

    let mut client_index = 0usize;
    while client_index < clients.len() {
        let client_id = clients[client_index].client_id.clone();
        match clients[client_index].stream.write_all(bytes) {
            Ok(()) => {
                client_index += 1;
            }
            Err(error) if is_disconnect_error(&error) => {
                clients.remove(client_index);
                if event_tx
                    .send(TcpServerWorkerEvent::ClientDisconnected {
                        client_id,
                        reason: Some(format!("failed to write TCP stream: {error}")),
                    })
                    .is_err()
                {
                    return Err(());
                }
            }
            Err(error) => {
                if event_tx
                    .send(TcpServerWorkerEvent::Error(format!(
                        "TCP client {client_id} send error: {error}"
                    )))
                    .is_err()
                {
                    return Err(());
                }
                client_index += 1;
            }
        }
    }

    Ok(())
}

fn shutdown_clients(clients: &mut Vec<TcpClientConnection>) {
    for client in clients.drain(..) {
        let _ = client.stream.shutdown(Shutdown::Both);
    }
}

fn is_disconnect_error(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::NotConnected
            | std::io::ErrorKind::BrokenPipe
            | std::io::ErrorKind::UnexpectedEof
    ) || error.raw_os_error() == Some(10054)
}

fn resolve_socket_addr(host: &str, port: u16) -> Result<SocketAddr, String> {
    let address = format!("{}:{}", host.trim(), port);
    let mut resolved = address
        .to_socket_addrs()
        .map_err(|error| format!("failed to resolve TCP server address '{address}': {error}"))?;

    resolved
        .next()
        .ok_or_else(|| format!("TCP server address '{address}' did not resolve to a socket address"))
}
