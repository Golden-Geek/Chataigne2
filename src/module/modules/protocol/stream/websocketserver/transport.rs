use std::{
    net::{Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs},
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle},
    time::Duration,
};

use crate::app::module::common::streaming::commands::StreamingSendFrameKind;
use crate::app::module::common::streaming::websocket::{
    WebSocketFrameKind, WebSocketHttpRequest, WebSocketIncomingFrame, WebSocketReadStatus,
    accept_server_handshake, is_websocket_upgrade_request, read_available_bytes, read_http_request,
    try_take_frame, write_control_frame, write_data_frame, write_http_error_response,
};

const WEBSOCKET_SERVER_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WebSocketServerTransportConfig {
    pub bind_host: String,
    pub bind_port: u16,
    pub path: String,
    pub receive_enabled: bool,
    pub send_enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum WebSocketServerWorkerEvent {
    ClientConnected { client_id: String, info: String },
    ClientDisconnected { client_id: String, reason: Option<String> },
    Bytes { client_id: String, bytes: Vec<u8> },
    Error(String),
    Stopped(String),
}

pub(crate) struct WebSocketServerTransportHandle {
    command_tx: Sender<WebSocketServerWorkerCommand>,
    event_rx: Receiver<WebSocketServerWorkerEvent>,
    worker: Option<JoinHandle<()>>,
}

impl WebSocketServerTransportHandle {
    pub(crate) fn spawn(config: WebSocketServerTransportConfig) -> Result<Self, String> {
        let bind_address = resolve_socket_addr(config.bind_host.as_str(), config.bind_port)?;
        let listener = TcpListener::bind(bind_address)
            .map_err(|error| format!("failed to bind WebSocket server socket on {bind_address}: {error}"))?;
        listener
            .set_nonblocking(true)
            .map_err(|error| format!("failed to set WebSocket server listener to non-blocking mode: {error}"))?;

        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();

        let worker = thread::Builder::new()
            .name(format!("streaming-websocket-server-{}", bind_address.port()))
            .spawn(move || worker_loop(listener, config, command_rx, event_tx))
            .map_err(|error| format!("failed to start WebSocket server worker thread: {error}"))?;

        Ok(Self {
            command_tx,
            event_rx,
            worker: Some(worker),
        })
    }

    pub(crate) fn send(&self, frame_kind: StreamingSendFrameKind, bytes: Vec<u8>) -> Result<(), String> {
        self.command_tx
            .send(WebSocketServerWorkerCommand::Broadcast { frame_kind, bytes })
            .map_err(|_| "WebSocket server worker is no longer running".to_string())
    }

    pub(crate) fn try_recv(&self) -> Result<WebSocketServerWorkerEvent, mpsc::TryRecvError> {
        self.event_rx.try_recv()
    }

    pub(crate) fn stop(&mut self) {
        let _ = self.command_tx.send(WebSocketServerWorkerCommand::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for WebSocketServerTransportHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

enum WebSocketServerWorkerCommand {
    Broadcast {
        frame_kind: StreamingSendFrameKind,
        bytes: Vec<u8>,
    },
    Stop,
}

struct WebSocketClientConnection {
    client_id: String,
    stream: TcpStream,
    pending_bytes: Vec<u8>,
}

fn worker_loop(
    listener: TcpListener,
    config: WebSocketServerTransportConfig,
    command_rx: Receiver<WebSocketServerWorkerCommand>,
    event_tx: Sender<WebSocketServerWorkerEvent>,
) {
    let mut clients = Vec::new();

    loop {
        if drain_commands(&command_rx, &event_tx, &mut clients, config.send_enabled) {
            break;
        }

        if accept_new_clients(&listener, &event_tx, &mut clients, config.path.as_str()).is_err() {
            break;
        }

        if config.receive_enabled && read_client_frames(&event_tx, &mut clients).is_err() {
            break;
        }

        match command_rx.recv_timeout(Duration::from_millis(5)) {
            Ok(WebSocketServerWorkerCommand::Broadcast { frame_kind, bytes }) => {
                if broadcast_bytes(
                    &event_tx,
                    &mut clients,
                    config.send_enabled,
                    frame_kind,
                    bytes.as_slice(),
                )
                .is_err()
                {
                    break;
                }
            }
            Ok(WebSocketServerWorkerCommand::Stop) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    shutdown_clients(&mut clients);
}

fn drain_commands(
    command_rx: &Receiver<WebSocketServerWorkerCommand>,
    event_tx: &Sender<WebSocketServerWorkerEvent>,
    clients: &mut Vec<WebSocketClientConnection>,
    send_enabled: bool,
) -> bool {
    loop {
        match command_rx.try_recv() {
            Ok(WebSocketServerWorkerCommand::Broadcast { frame_kind, bytes }) => {
                if broadcast_bytes(event_tx, clients, send_enabled, frame_kind, bytes.as_slice()).is_err() {
                    return true;
                }
            }
            Ok(WebSocketServerWorkerCommand::Stop) => return true,
            Err(mpsc::TryRecvError::Empty) => return false,
            Err(mpsc::TryRecvError::Disconnected) => return true,
        }
    }
}

fn accept_new_clients(
    listener: &TcpListener,
    event_tx: &Sender<WebSocketServerWorkerEvent>,
    clients: &mut Vec<WebSocketClientConnection>,
    expected_path: &str,
) -> Result<(), ()> {
    loop {
        match listener.accept() {
            Ok((mut stream, remote_address)) => {
                if let Err(error) = stream.set_nodelay(true) {
                    let _ = event_tx.send(WebSocketServerWorkerEvent::Error(format!(
                        "failed to enable TCP_NODELAY for WebSocket client {remote_address}: {error}"
                    )));
                    continue;
                }
                if let Err(error) = stream.set_read_timeout(Some(WEBSOCKET_SERVER_HANDSHAKE_TIMEOUT)) {
                    let _ = event_tx.send(WebSocketServerWorkerEvent::Error(format!(
                        "failed to set WebSocket handshake read timeout for {remote_address}: {error}"
                    )));
                    continue;
                }
                if let Err(error) = stream.set_write_timeout(Some(WEBSOCKET_SERVER_HANDSHAKE_TIMEOUT)) {
                    let _ = event_tx.send(WebSocketServerWorkerEvent::Error(format!(
                        "failed to set WebSocket handshake write timeout for {remote_address}: {error}"
                    )));
                    continue;
                }

                let request = match read_http_request(&mut stream) {
                    Ok(request) => request,
                    Err(error) => {
                        let _ = event_tx.send(WebSocketServerWorkerEvent::Error(format!(
                            "failed to read WebSocket upgrade request from {remote_address}: {error}"
                        )));
                        let _ = stream.shutdown(Shutdown::Both);
                        continue;
                    }
                };

                if !is_websocket_upgrade_request(&request) {
                    let _ = write_http_error_response(
                        &mut stream,
                        "426 Upgrade Required",
                        "websocket upgrade required",
                    );
                    let _ = stream.shutdown(Shutdown::Both);
                    continue;
                }

                if !expected_path.is_empty() && request.path != expected_path {
                    let _ = write_http_error_response(&mut stream, "404 Not Found", "websocket path not found");
                    let _ = stream.shutdown(Shutdown::Both);
                    continue;
                }

                if let Err(error) = accept_server_handshake(&mut stream, &request) {
                    let _ = event_tx.send(WebSocketServerWorkerEvent::Error(format!(
                        "failed WebSocket server handshake for {remote_address}: {error}"
                    )));
                    let _ = stream.shutdown(Shutdown::Both);
                    continue;
                }

                if let Err(error) = stream.set_read_timeout(None) {
                    let _ = event_tx.send(WebSocketServerWorkerEvent::Error(format!(
                        "failed to clear WebSocket client read timeout for {remote_address}: {error}"
                    )));
                    let _ = stream.shutdown(Shutdown::Both);
                    continue;
                }
                if let Err(error) = stream.set_write_timeout(None) {
                    let _ = event_tx.send(WebSocketServerWorkerEvent::Error(format!(
                        "failed to clear WebSocket client write timeout for {remote_address}: {error}"
                    )));
                    let _ = stream.shutdown(Shutdown::Both);
                    continue;
                }
                if let Err(error) = stream.set_nonblocking(true) {
                    let _ = event_tx.send(WebSocketServerWorkerEvent::Error(format!(
                        "failed to set WebSocket client {remote_address} to non-blocking mode: {error}"
                    )));
                    let _ = stream.shutdown(Shutdown::Both);
                    continue;
                }

                register_client(event_tx, clients, remote_address, stream, request);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
            Err(error) => {
                let _ = event_tx.send(WebSocketServerWorkerEvent::Stopped(format!(
                    "WebSocket server accept error: {error}"
                )));
                return Err(());
            }
        }
    }
}

fn register_client(
    event_tx: &Sender<WebSocketServerWorkerEvent>,
    clients: &mut Vec<WebSocketClientConnection>,
    remote_address: SocketAddr,
    stream: TcpStream,
    request: WebSocketHttpRequest,
) {
    let client_id = remote_address.to_string();
    let info = format!("Remote address: {remote_address}, Path: {}", request.path);
    clients.push(WebSocketClientConnection {
        client_id: client_id.clone(),
        stream,
        pending_bytes: Vec::new(),
    });
    let _ = event_tx.send(WebSocketServerWorkerEvent::ClientConnected { client_id, info });
}

fn read_client_frames(
    event_tx: &Sender<WebSocketServerWorkerEvent>,
    clients: &mut Vec<WebSocketClientConnection>,
) -> Result<(), ()> {
    let mut client_index = 0usize;
    while client_index < clients.len() {
        let client_id = clients[client_index].client_id.clone();
        let read_status = {
            let client = &mut clients[client_index];
            match read_available_bytes(&mut client.stream, &mut client.pending_bytes) {
            Ok(status) => status,
            Err(error) => {
                disconnect_client(
                    event_tx,
                    clients,
                    client_index,
                    client_id,
                    Some(format!("WebSocket receive error: {error}")),
                )?;
                continue;
            }
            }
        };

        let mut remove_reason = None;
        loop {
            let frame = match try_take_frame(&mut clients[client_index].pending_bytes, true) {
                Ok(Some(frame)) => frame,
                Ok(None) => break,
                Err(error) => {
                    remove_reason = Some(format!("WebSocket frame decode error: {error}"));
                    break;
                }
            };

            match frame {
                WebSocketIncomingFrame::Text(text) => {
                    if event_tx
                        .send(WebSocketServerWorkerEvent::Bytes {
                            client_id: client_id.clone(),
                            bytes: text.into_bytes(),
                        })
                        .is_err()
                    {
                        return Err(());
                    }
                }
                WebSocketIncomingFrame::Binary(bytes) => {
                    if event_tx
                        .send(WebSocketServerWorkerEvent::Bytes {
                            client_id: client_id.clone(),
                            bytes,
                        })
                        .is_err()
                    {
                        return Err(());
                    }
                }
                WebSocketIncomingFrame::Ping(payload) => {
                    if let Err(error) = write_control_frame(
                        &mut clients[client_index].stream,
                        0xA,
                        payload.as_slice(),
                        false,
                    ) {
                        remove_reason = Some(format!("failed to respond to WebSocket ping: {error}"));
                        break;
                    }
                }
                WebSocketIncomingFrame::Pong(_) => {}
                WebSocketIncomingFrame::Close => {
                    remove_reason = Some("WebSocket client closed the connection".to_string());
                    break;
                }
            }
        }

        if let Some(reason) = remove_reason {
            disconnect_client(event_tx, clients, client_index, client_id, Some(reason))?;
            continue;
        }

        if matches!(read_status, WebSocketReadStatus::Closed) {
            disconnect_client(event_tx, clients, client_index, client_id, None)?;
            continue;
        }

        client_index += 1;
    }

    Ok(())
}

fn disconnect_client(
    event_tx: &Sender<WebSocketServerWorkerEvent>,
    clients: &mut Vec<WebSocketClientConnection>,
    client_index: usize,
    client_id: String,
    reason: Option<String>,
) -> Result<(), ()> {
    let client = clients.remove(client_index);
    let _ = client.stream.shutdown(Shutdown::Both);
    event_tx
        .send(WebSocketServerWorkerEvent::ClientDisconnected { client_id, reason })
        .map_err(|_| ())
}

fn broadcast_bytes(
    event_tx: &Sender<WebSocketServerWorkerEvent>,
    clients: &mut Vec<WebSocketClientConnection>,
    send_enabled: bool,
    frame_kind: StreamingSendFrameKind,
    bytes: &[u8],
) -> Result<(), ()> {
    if !send_enabled {
        let _ = event_tx.send(WebSocketServerWorkerEvent::Error(
            "WebSocket server sender is disabled; outgoing bytes were dropped".to_string(),
        ));
        return Ok(());
    }

    let mut client_index = 0usize;
    while client_index < clients.len() {
        let client_id = clients[client_index].client_id.clone();
        match write_data_frame(
            &mut clients[client_index].stream,
            websocket_frame_kind(frame_kind),
            bytes,
            false,
        ) {
            Ok(()) => {
                client_index += 1;
            }
            Err(error) if is_disconnect_error(&error) => {
                disconnect_client(
                    event_tx,
                    clients,
                    client_index,
                    client_id,
                    Some(format!("failed to write WebSocket frame: {error}")),
                )?;
            }
            Err(error) => {
                if event_tx
                    .send(WebSocketServerWorkerEvent::Error(format!(
                        "WebSocket client {client_id} send error: {error}"
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

fn shutdown_clients(clients: &mut Vec<WebSocketClientConnection>) {
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
        .map_err(|error| format!("failed to resolve WebSocket server address '{address}': {error}"))?;

    resolved.next().ok_or_else(|| {
        format!("WebSocket server address '{address}' did not resolve to a socket address")
    })
}

fn websocket_frame_kind(frame_kind: StreamingSendFrameKind) -> WebSocketFrameKind {
    match frame_kind {
        StreamingSendFrameKind::Text => WebSocketFrameKind::Text,
        StreamingSendFrameKind::Binary => WebSocketFrameKind::Binary,
    }
}