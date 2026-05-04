use std::{
    net::{Shutdown, SocketAddr, TcpStream, ToSocketAddrs},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crate::app::module::common::streaming::commands::StreamingSendFrameKind;
use crate::app::module::common::streaming::websocket::{
    WebSocketFrameKind, WebSocketIncomingFrame, WebSocketReadStatus, perform_client_handshake,
    read_available_bytes, try_take_frame, write_control_frame, write_data_frame,
};

const WEBSOCKET_CONNECT_TIMEOUT: Duration = Duration::from_millis(500);
const WEBSOCKET_RECONNECT_BASE_DELAY: Duration = Duration::from_millis(250);
const WEBSOCKET_RECONNECT_MAX_DELAY: Duration = Duration::from_secs(5);
const WEBSOCKET_WORKER_POLL_INTERVAL: Duration = Duration::from_millis(5);
const WEBSOCKET_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WebSocketClientTransportConfig {
    pub remote_host: String,
    pub remote_port: u16,
    pub path: String,
    pub receive_enabled: bool,
    pub send_enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum WebSocketClientConnectionStatus {
    Connected { remote_address: SocketAddr },
    Recovering { remote_address: SocketAddr, message: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum StreamingWorkerEvent {
    Bytes(Vec<u8>),
    Error(String),
    Status(WebSocketClientConnectionStatus),
}

pub(crate) struct WebSocketClientTransportHandle {
    command_tx: Sender<WebSocketClientWorkerCommand>,
    event_rx: Receiver<StreamingWorkerEvent>,
    connected: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl WebSocketClientTransportHandle {
    pub(crate) fn spawn(config: WebSocketClientTransportConfig) -> Result<Self, String> {
        let remote_address = resolve_socket_addr(config.remote_host.as_str(), config.remote_port)?;
        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let connected = Arc::new(AtomicBool::new(false));
        let worker_connected = Arc::clone(&connected);

        let worker = thread::Builder::new()
            .name(format!("streaming-websocket-client-{}", remote_address.port()))
            .spawn(move || worker_loop(config, remote_address, command_rx, event_tx, worker_connected))
            .map_err(|error| format!("failed to start WebSocket client worker thread: {error}"))?;

        Ok(Self {
            command_tx,
            event_rx,
            connected,
            worker: Some(worker),
        })
    }

    pub(crate) fn send(&self, frame_kind: StreamingSendFrameKind, bytes: Vec<u8>) -> Result<(), String> {
        if !self.connected.load(Ordering::Acquire) {
            return Err("WebSocket client transport is not connected".to_string());
        }

        self.command_tx
            .send(WebSocketClientWorkerCommand::Send { frame_kind, bytes })
            .map_err(|_| "WebSocket client worker is no longer running".to_string())
    }

    pub(crate) fn try_recv(&self) -> Result<StreamingWorkerEvent, mpsc::TryRecvError> {
        self.event_rx.try_recv()
    }

    pub(crate) fn stop(&mut self) {
        let _ = self.command_tx.send(WebSocketClientWorkerCommand::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for WebSocketClientTransportHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

enum WebSocketClientWorkerCommand {
    Send {
        frame_kind: StreamingSendFrameKind,
        bytes: Vec<u8>,
    },
    Stop,
}

fn worker_loop(
    config: WebSocketClientTransportConfig,
    remote_address: SocketAddr,
    command_rx: Receiver<WebSocketClientWorkerCommand>,
    event_tx: Sender<StreamingWorkerEvent>,
    connected: Arc<AtomicBool>,
) {
    let mut stream = None;
    let mut pending_bytes = Vec::new();
    let mut reconnect_delay = WEBSOCKET_RECONNECT_BASE_DELAY;
    let mut next_connect_at = Instant::now();
    let mut last_status = None;

    'worker: loop {
        if stream.is_none() {
            if Instant::now() >= next_connect_at {
                match connect_stream(&config, remote_address) {
                    Ok(open_stream) => {
                        pending_bytes.clear();
                        stream = Some(open_stream);
                        reconnect_delay = WEBSOCKET_RECONNECT_BASE_DELAY;
                        if !emit_status(
                            &event_tx,
                            &mut last_status,
                            WebSocketClientConnectionStatus::Connected { remote_address },
                        ) {
                            return;
                        }
                        connected.store(true, Ordering::Release);
                        continue;
                    }
                    Err(error) => {
                        if !enter_recovery(
                            &event_tx,
                            &connected,
                            &mut last_status,
                            &mut reconnect_delay,
                            &mut next_connect_at,
                            remote_address,
                            error,
                        ) {
                            return;
                        }
                    }
                }
            }

            match command_rx.recv_timeout(reconnect_wait(next_connect_at)) {
                Ok(WebSocketClientWorkerCommand::Send { bytes, .. }) => {
                    if !config.send_enabled {
                        let _ = event_tx.send(StreamingWorkerEvent::Error(
                            "WebSocket client sender is disabled; outgoing bytes were dropped".to_string(),
                        ));
                    } else if event_tx
                        .send(StreamingWorkerEvent::Error(
                            "WebSocket client transport is reconnecting; outgoing bytes were dropped"
                                .to_string(),
                        ))
                        .is_err()
                    {
                        return;
                    }
                    drop(bytes);
                }
                Ok(WebSocketClientWorkerCommand::Stop) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }

            continue;
        }

        if config.receive_enabled {
            let read_status = {
                let Some(active_stream) = stream.as_mut() else {
                    continue;
                };

                match read_available_bytes(active_stream, &mut pending_bytes) {
                    Ok(status) => status,
                    Err(error) => {
                        close_stream(&mut stream, &connected);
                        if !enter_recovery(
                            &event_tx,
                            &connected,
                            &mut last_status,
                            &mut reconnect_delay,
                            &mut next_connect_at,
                            remote_address,
                            format!("WebSocket client receive error: {error}"),
                        ) {
                            return;
                        }
                        continue 'worker;
                    }
                }
            };

            loop {
                let frame = match try_take_frame(&mut pending_bytes, false) {
                    Ok(Some(frame)) => frame,
                    Ok(None) => break,
                    Err(error) => {
                        close_stream(&mut stream, &connected);
                        if !enter_recovery(
                            &event_tx,
                            &connected,
                            &mut last_status,
                            &mut reconnect_delay,
                            &mut next_connect_at,
                            remote_address,
                            format!("WebSocket client frame decode error: {error}"),
                        ) {
                            return;
                        }
                        continue 'worker;
                    }
                };

                match frame {
                    WebSocketIncomingFrame::Text(text) => {
                        if event_tx
                            .send(StreamingWorkerEvent::Bytes(text.into_bytes()))
                            .is_err()
                        {
                            return;
                        }
                    }
                    WebSocketIncomingFrame::Binary(bytes) => {
                        if event_tx.send(StreamingWorkerEvent::Bytes(bytes)).is_err() {
                            return;
                        }
                    }
                    WebSocketIncomingFrame::Ping(payload) => {
                        let Some(active_stream) = stream.as_mut() else {
                            continue 'worker;
                        };

                        if let Err(error) =
                            write_control_frame(active_stream, 0xA, payload.as_slice(), true)
                        {
                            close_stream(&mut stream, &connected);
                            if !enter_recovery(
                                &event_tx,
                                &connected,
                                &mut last_status,
                                &mut reconnect_delay,
                                &mut next_connect_at,
                                remote_address,
                                format!("failed to respond to WebSocket ping: {error}"),
                            ) {
                                return;
                            }
                            continue 'worker;
                        }
                    }
                    WebSocketIncomingFrame::Pong(_) => {}
                    WebSocketIncomingFrame::Close => {
                        close_stream(&mut stream, &connected);
                        if !enter_recovery(
                            &event_tx,
                            &connected,
                            &mut last_status,
                            &mut reconnect_delay,
                            &mut next_connect_at,
                            remote_address,
                            "WebSocket peer closed the connection".to_string(),
                        ) {
                            return;
                        }
                        continue 'worker;
                    }
                }
            }

            if matches!(read_status, WebSocketReadStatus::Closed) {
                close_stream(&mut stream, &connected);
                if !enter_recovery(
                    &event_tx,
                    &connected,
                    &mut last_status,
                    &mut reconnect_delay,
                    &mut next_connect_at,
                    remote_address,
                    "WebSocket peer closed the connection".to_string(),
                ) {
                    return;
                }
                continue;
            }
        }

        match command_rx.recv_timeout(WEBSOCKET_WORKER_POLL_INTERVAL) {
            Ok(WebSocketClientWorkerCommand::Send { frame_kind, bytes }) => {
                let Some(active_stream) = stream.as_mut() else {
                    continue;
                };

                if let Err(error) = send_bytes(active_stream, frame_kind, bytes.as_slice()) {
                    close_stream(&mut stream, &connected);
                    if !enter_recovery(
                        &event_tx,
                        &connected,
                        &mut last_status,
                        &mut reconnect_delay,
                        &mut next_connect_at,
                        remote_address,
                        format!("failed to send WebSocket frame: {error}"),
                    ) {
                        return;
                    }
                }
            }
            Ok(WebSocketClientWorkerCommand::Stop) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    close_stream(&mut stream, &connected);
}

fn connect_stream(config: &WebSocketClientTransportConfig, remote_address: SocketAddr) -> Result<TcpStream, String> {
    let mut stream = TcpStream::connect_timeout(&remote_address, WEBSOCKET_CONNECT_TIMEOUT)
        .map_err(|error| format!("failed to connect WebSocket client socket to {remote_address}: {error}"))?;

    stream
        .set_nodelay(true)
        .map_err(|error| format!("failed to enable TCP_NODELAY for WebSocket client: {error}"))?;
    stream
        .set_read_timeout(Some(WEBSOCKET_HANDSHAKE_TIMEOUT))
        .map_err(|error| format!("failed to set WebSocket handshake read timeout: {error}"))?;
    stream
        .set_write_timeout(Some(WEBSOCKET_HANDSHAKE_TIMEOUT))
        .map_err(|error| format!("failed to set WebSocket handshake write timeout: {error}"))?;

    perform_client_handshake(
        &mut stream,
        config.remote_host.as_str(),
        config.remote_port,
        config.path.as_str(),
    )
    .map_err(|error| format!("failed WebSocket client handshake: {error}"))?;

    stream
        .set_read_timeout(None)
        .map_err(|error| format!("failed to clear WebSocket client read timeout: {error}"))?;
    stream
        .set_write_timeout(None)
        .map_err(|error| format!("failed to clear WebSocket client write timeout: {error}"))?;
    stream
        .set_nonblocking(true)
        .map_err(|error| format!("failed to set WebSocket client stream to non-blocking mode: {error}"))?;

    Ok(stream)
}

fn send_bytes(
    stream: &mut TcpStream,
    frame_kind: StreamingSendFrameKind,
    bytes: &[u8],
) -> Result<(), String> {
    write_data_frame(stream, websocket_frame_kind(frame_kind), bytes, true)
        .map_err(|error| format!("failed to write WebSocket frame: {error}"))
}

fn websocket_frame_kind(frame_kind: StreamingSendFrameKind) -> WebSocketFrameKind {
    match frame_kind {
        StreamingSendFrameKind::Text => WebSocketFrameKind::Text,
        StreamingSendFrameKind::Binary => WebSocketFrameKind::Binary,
    }
}

fn emit_status(
    event_tx: &Sender<StreamingWorkerEvent>,
    last_status: &mut Option<WebSocketClientConnectionStatus>,
    status: WebSocketClientConnectionStatus,
) -> bool {
    if last_status.as_ref() == Some(&status) {
        return true;
    }

    *last_status = Some(status.clone());
    event_tx.send(StreamingWorkerEvent::Status(status)).is_ok()
}

fn enter_recovery(
    event_tx: &Sender<StreamingWorkerEvent>,
    connected: &Arc<AtomicBool>,
    last_status: &mut Option<WebSocketClientConnectionStatus>,
    reconnect_delay: &mut Duration,
    next_connect_at: &mut Instant,
    remote_address: SocketAddr,
    message: String,
) -> bool {
    connected.store(false, Ordering::Release);
    *next_connect_at = Instant::now() + *reconnect_delay;
    *reconnect_delay = (*reconnect_delay * 2).min(WEBSOCKET_RECONNECT_MAX_DELAY);

    emit_status(
        event_tx,
        last_status,
        WebSocketClientConnectionStatus::Recovering {
            remote_address,
            message,
        },
    )
}

fn reconnect_wait(next_connect_at: Instant) -> Duration {
    next_connect_at.saturating_duration_since(Instant::now())
}

fn close_stream(stream: &mut Option<TcpStream>, connected: &Arc<AtomicBool>) {
    connected.store(false, Ordering::Release);
    if let Some(stream) = stream.take() {
        let _ = stream.shutdown(Shutdown::Both);
    }
}

fn resolve_socket_addr(host: &str, port: u16) -> Result<SocketAddr, String> {
    let address = format!("{}:{}", host.trim(), port);
    let mut resolved = address
        .to_socket_addrs()
        .map_err(|error| format!("failed to resolve WebSocket client address '{address}': {error}"))?;

    resolved.next().ok_or_else(|| {
        format!("WebSocket client address '{address}' did not resolve to a socket address")
    })
}