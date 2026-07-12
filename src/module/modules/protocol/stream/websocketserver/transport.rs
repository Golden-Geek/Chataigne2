use std::{
    net::{SocketAddr, TcpListener, ToSocketAddrs},
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle},
    time::Duration,
};

use crate::app::module::common::pending_channel::{pending_channel, PendingReceiver, PendingSender};

use futures_util::{SinkExt, StreamExt};
use tokio::runtime::{Builder, Runtime};
use tokio::time::timeout;
use tokio_tungstenite::{
    accept_hdr_async_with_config,
    tungstenite::{
        handshake::server::{Callback, ErrorResponse, Request, Response},
        http::{Response as HttpResponse, StatusCode},
        protocol::Message,
    },
    WebSocketStream,
};

use crate::app::module::common::streaming::commands::StreamingSendFrameKind;
use crate::app::module::common::streaming::websocket::{websocket_config, websocket_message};

const WEBSOCKET_SERVER_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(3);
const WEBSOCKET_SERVER_POLL_INTERVAL: Duration = Duration::from_millis(5);

type ServerWebSocketStream = WebSocketStream<tokio::net::TcpStream>;

struct PathValidatingCallback<'a> {
    expected_path: &'a str,
    request_path: &'a mut Option<String>,
}

impl Callback for PathValidatingCallback<'_> {
    fn on_request(
        self,
        request: &Request,
        response: Response,
    ) -> Result<Response, ErrorResponse> {
        let path = request.uri().path().to_string();
        if !self.expected_path.is_empty() && path != self.expected_path {
            return Err(websocket_rejection_response(
                StatusCode::NOT_FOUND,
                "websocket path not found",
            ));
        }

        *self.request_path = Some(path);
        Ok(response)
    }
}

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
    Bytes {
        client_id: String,
        frame_kind: StreamingSendFrameKind,
        bytes: Vec<u8>,
    },
    Error(String),
    Stopped(String),
}

pub(crate) struct WebSocketServerTransportHandle {
    command_tx: Sender<WebSocketServerWorkerCommand>,
    event_rx: PendingReceiver<WebSocketServerWorkerEvent>,
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
        let (event_tx, event_rx) = pending_channel();

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

    pub(crate) fn has_pending(&self) -> bool {
        self.event_rx.has_pending()
    }

    pub(crate) fn clear_pending(&self) {
        self.event_rx.clear_pending();
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
    stream: ServerWebSocketStream,
}

enum WebSocketPollOutcome {
    Message(Message),
    Closed,
    Idle,
}

enum ReceiveOutcome {
    Open,
    Closed,
}

fn worker_loop(
    listener: TcpListener,
    config: WebSocketServerTransportConfig,
    command_rx: Receiver<WebSocketServerWorkerCommand>,
    event_tx: PendingSender<WebSocketServerWorkerEvent>,
) {
    let runtime = match Builder::new_current_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            let _ = event_tx.send(WebSocketServerWorkerEvent::Stopped(format!(
                "failed to build WebSocket server runtime: {error}"
            )));
            return;
        }
    };

    let listener = {
        let _guard = runtime.enter();
        match tokio::net::TcpListener::from_std(listener) {
            Ok(listener) => listener,
            Err(error) => {
                let _ = event_tx.send(WebSocketServerWorkerEvent::Stopped(format!(
                    "failed to convert WebSocket listener into tokio listener: {error}"
                )));
                return;
            }
        }
    };

    let mut clients = Vec::new();

    loop {
        if drain_commands(&runtime, &command_rx, &event_tx, &mut clients, config.send_enabled) {
            break;
        }

        if accept_new_clients(&runtime, &listener, &event_tx, &mut clients, config.path.as_str()).is_err() {
            break;
        }

        if config.receive_enabled && read_client_frames(&runtime, &event_tx, &mut clients).is_err() {
            break;
        }

        match command_rx.recv_timeout(WEBSOCKET_SERVER_POLL_INTERVAL) {
            Ok(WebSocketServerWorkerCommand::Broadcast { frame_kind, bytes }) => {
                if broadcast_bytes(
                    &runtime,
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

    shutdown_clients(&runtime, &mut clients);
}

fn drain_commands(
    runtime: &Runtime,
    command_rx: &Receiver<WebSocketServerWorkerCommand>,
    event_tx: &PendingSender<WebSocketServerWorkerEvent>,
    clients: &mut Vec<WebSocketClientConnection>,
    send_enabled: bool,
) -> bool {
    loop {
        match command_rx.try_recv() {
            Ok(WebSocketServerWorkerCommand::Broadcast { frame_kind, bytes }) => {
                if broadcast_bytes(runtime, event_tx, clients, send_enabled, frame_kind, bytes.as_slice())
                    .is_err()
                {
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
    runtime: &Runtime,
    listener: &tokio::net::TcpListener,
    event_tx: &PendingSender<WebSocketServerWorkerEvent>,
    clients: &mut Vec<WebSocketClientConnection>,
    expected_path: &str,
) -> Result<(), ()> {
    loop {
        match runtime.block_on(async { timeout(Duration::ZERO, listener.accept()).await }) {
            Err(_) => return Ok(()),
            Ok(Ok((stream, remote_address))) => {
                if let Err(error) = stream.set_nodelay(true) {
                    let _ = event_tx.send(WebSocketServerWorkerEvent::Error(format!(
                        "failed to enable TCP_NODELAY for WebSocket client {remote_address}: {error}"
                    )));
                    continue;
                }

                let mut request_path = None::<String>;
                let callback = PathValidatingCallback {
                    expected_path,
                    request_path: &mut request_path,
                };

                match runtime.block_on(async {
                    timeout(
                        WEBSOCKET_SERVER_HANDSHAKE_TIMEOUT,
                        accept_hdr_async_with_config(stream, callback, Some(websocket_config())),
                    )
                    .await
                }) {
                    Err(_) => {
                        let _ = event_tx.send(WebSocketServerWorkerEvent::Error(format!(
                            "timed out during WebSocket server handshake for {remote_address}"
                        )));
                    }
                    Ok(Ok(stream)) => {
                        register_client(
                            event_tx,
                            clients,
                            remote_address,
                            request_path.unwrap_or_else(|| "/".to_string()),
                            stream,
                        );
                    }
                    Ok(Err(error)) => {
                        let _ = event_tx.send(WebSocketServerWorkerEvent::Error(format!(
                            "failed WebSocket server handshake for {remote_address}: {error}"
                        )));
                    }
                }
            }
            Ok(Err(error)) => {
                let _ = event_tx.send(WebSocketServerWorkerEvent::Stopped(format!(
                    "WebSocket server accept error: {error}"
                )));
                return Err(());
            }
        }
    }
}

fn register_client(
    event_tx: &PendingSender<WebSocketServerWorkerEvent>,
    clients: &mut Vec<WebSocketClientConnection>,
    remote_address: SocketAddr,
    request_path: String,
    stream: ServerWebSocketStream,
) {
    let client_id = remote_address.to_string();
    let info = format!("Remote address: {remote_address}, Path: {request_path}");
    clients.push(WebSocketClientConnection {
        client_id: client_id.clone(),
        stream,
    });
    let _ = event_tx.send(WebSocketServerWorkerEvent::ClientConnected { client_id, info });
}

fn read_client_frames(
    runtime: &Runtime,
    event_tx: &PendingSender<WebSocketServerWorkerEvent>,
    clients: &mut Vec<WebSocketClientConnection>,
) -> Result<(), ()> {
    let mut client_index = 0usize;
    while client_index < clients.len() {
        let client_id = clients[client_index].client_id.clone();
        let receive_result = {
            let client = &mut clients[client_index];
            receive_messages(runtime, &mut client.stream, client_id.as_str(), event_tx)
        };

        match receive_result {
            Ok(ReceiveOutcome::Open) => {
                client_index += 1;
            }
            Ok(ReceiveOutcome::Closed) => {
                disconnect_client(runtime, event_tx, clients, client_index, client_id, None)?;
            }
            Err(reason) => {
                disconnect_client(runtime, event_tx, clients, client_index, client_id, Some(reason))?;
            }
        }
    }

    Ok(())
}

fn receive_messages(
    runtime: &Runtime,
    stream: &mut ServerWebSocketStream,
    client_id: &str,
    event_tx: &PendingSender<WebSocketServerWorkerEvent>,
) -> Result<ReceiveOutcome, String> {
    let mut poll_interval = Duration::ZERO;

    loop {
        match poll_next_message(runtime, stream, poll_interval)? {
            WebSocketPollOutcome::Idle => return Ok(ReceiveOutcome::Open),
            WebSocketPollOutcome::Closed => return Ok(ReceiveOutcome::Closed),
            WebSocketPollOutcome::Message(message) => match message {
                Message::Text(text) => {
                    if event_tx
                        .send(WebSocketServerWorkerEvent::Bytes {
                            client_id: client_id.to_string(),
                            frame_kind: StreamingSendFrameKind::Text,
                            bytes: text.to_string().into_bytes(),
                        })
                        .is_err()
                    {
                        return Err("WebSocket server event channel disconnected".to_string());
                    }
                }
                Message::Binary(bytes) => {
                    if event_tx
                        .send(WebSocketServerWorkerEvent::Bytes {
                            client_id: client_id.to_string(),
                            frame_kind: StreamingSendFrameKind::Binary,
                            bytes: bytes.to_vec(),
                        })
                        .is_err()
                    {
                        return Err("WebSocket server event channel disconnected".to_string());
                    }
                }
                Message::Ping(_) => {
                    flush_stream(runtime, stream)
                        .map_err(|error| format!("failed to flush WebSocket ping response: {error}"))?;
                }
                Message::Pong(_) | Message::Frame(_) => {}
                Message::Close(_) => return Ok(ReceiveOutcome::Closed),
            },
        }

        poll_interval = Duration::ZERO;
    }
}

fn poll_next_message(
    runtime: &Runtime,
    stream: &mut ServerWebSocketStream,
    poll_interval: Duration,
) -> Result<WebSocketPollOutcome, String> {
    match runtime.block_on(async { timeout(poll_interval, stream.next()).await }) {
        Err(_) => Ok(WebSocketPollOutcome::Idle),
        Ok(None) => Ok(WebSocketPollOutcome::Closed),
        Ok(Some(Ok(message))) => Ok(WebSocketPollOutcome::Message(message)),
        Ok(Some(Err(error))) => Err(format!("WebSocket receive error: {error}")),
    }
}

fn flush_stream(runtime: &Runtime, stream: &mut ServerWebSocketStream) -> Result<(), String> {
    runtime
        .block_on(async { stream.flush().await })
        .map_err(|error| error.to_string())
}

fn disconnect_client(
    runtime: &Runtime,
    event_tx: &PendingSender<WebSocketServerWorkerEvent>,
    clients: &mut Vec<WebSocketClientConnection>,
    client_index: usize,
    client_id: String,
    reason: Option<String>,
) -> Result<(), ()> {
    let mut client = clients.remove(client_index);
    let _ = runtime.block_on(client.stream.close(None));
    event_tx
        .send(WebSocketServerWorkerEvent::ClientDisconnected { client_id, reason })
        .map_err(|_| ())
}

fn broadcast_bytes(
    runtime: &Runtime,
    event_tx: &PendingSender<WebSocketServerWorkerEvent>,
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

    let message = match websocket_message(frame_kind, bytes) {
        Ok(message) => message,
        Err(error) => {
            let _ = event_tx.send(WebSocketServerWorkerEvent::Error(error));
            return Ok(());
        }
    };

    let mut client_index = 0usize;
    while client_index < clients.len() {
        let client_id = clients[client_index].client_id.clone();
        let send_result = {
            let client = &mut clients[client_index];
            runtime.block_on(async { client.stream.send(message.clone()).await })
        };

        match send_result {
            Ok(()) => {
                client_index += 1;
            }
            Err(error) => {
                disconnect_client(
                    runtime,
                    event_tx,
                    clients,
                    client_index,
                    client_id,
                    Some(format!("failed to write WebSocket frame: {error}")),
                )?;
            }
        }
    }

    Ok(())
}

fn shutdown_clients(runtime: &Runtime, clients: &mut Vec<WebSocketClientConnection>) {
    for mut client in clients.drain(..) {
        let _ = runtime.block_on(client.stream.close(None));
    }
}

fn websocket_rejection_response(status: StatusCode, message: &str) -> ErrorResponse {
    HttpResponse::builder()
        .status(status)
        .header("content-type", "text/plain; charset=utf-8")
        .body(Some(message.to_string()))
        .expect("valid websocket rejection response")
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
