use std::{
    net::{SocketAddr, ToSocketAddrs},
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use futures_util::{SinkExt, StreamExt};
use tokio::runtime::{Builder, Runtime};
use tokio::time::timeout;
use tokio_tungstenite::{client_async_with_config, tungstenite::protocol::Message, WebSocketStream};

use golden_io::{
    pending_channel, PendingDrain, PendingReceiver, PendingSender, ReconnectBackoff,
};
use crate::app::module::common::streaming::commands::StreamingSendFrameKind;
use crate::app::module::common::streaming::websocket::{
    websocket_client_url, websocket_config, websocket_message,
};

const WEBSOCKET_CONNECT_TIMEOUT: Duration = Duration::from_millis(500);
const WEBSOCKET_RECONNECT_BASE_DELAY: Duration = Duration::from_millis(250);
const WEBSOCKET_RECONNECT_MAX_DELAY: Duration = Duration::from_secs(5);
const WEBSOCKET_WORKER_POLL_INTERVAL: Duration = Duration::from_millis(5);
const WEBSOCKET_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(3);
const WEBSOCKET_CLIENT_EVENT_DRAIN_BUDGET: NonZeroUsize =
    NonZeroUsize::new(1_024).expect("WebSocket client event drain budget must be nonzero");

type ClientWebSocketStream = WebSocketStream<tokio::net::TcpStream>;

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
    Bytes {
        frame_kind: StreamingSendFrameKind,
        bytes: Vec<u8>,
    },
    Error(String),
    Status(WebSocketClientConnectionStatus),
}

pub(crate) struct WebSocketClientTransportHandle {
    command_tx: Sender<WebSocketClientWorkerCommand>,
    event_rx: PendingReceiver<StreamingWorkerEvent>,
    connected: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl WebSocketClientTransportHandle {
    pub(crate) fn spawn(config: WebSocketClientTransportConfig) -> Result<Self, String> {
        let remote_address = resolve_socket_addr(config.remote_host.as_str(), config.remote_port)?;
        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = pending_channel();
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

    pub(crate) fn drain_events(&self, events: &mut Vec<StreamingWorkerEvent>) -> PendingDrain {
        self.event_rx
            .drain_into(events, WEBSOCKET_CLIENT_EVENT_DRAIN_BUDGET)
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.event_rx.has_pending()
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
    config: WebSocketClientTransportConfig,
    remote_address: SocketAddr,
    command_rx: Receiver<WebSocketClientWorkerCommand>,
    event_tx: PendingSender<StreamingWorkerEvent>,
    connected: Arc<AtomicBool>,
) {
    let runtime = match Builder::new_current_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            let _ = event_tx.send(StreamingWorkerEvent::Error(format!(
                "failed to build WebSocket client runtime: {error}"
            )));
            return;
        }
    };

    let mut stream = None;
    let mut reconnect_delay = ReconnectBackoff::new(
        WEBSOCKET_RECONNECT_BASE_DELAY,
        WEBSOCKET_RECONNECT_MAX_DELAY,
    );
    let mut next_connect_at = Instant::now();
    let mut last_status = None;

    'worker: loop {
        if stream.is_none() {
            if Instant::now() >= next_connect_at {
                match connect_stream(&runtime, &config, remote_address) {
                    Ok(open_stream) => {
                        stream = Some(open_stream);
                        reconnect_delay.reset();
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
            let receive_result = {
                let Some(active_stream) = stream.as_mut() else {
                    continue;
                };

                receive_messages(&runtime, active_stream, &event_tx)
            };

            match receive_result {
                Ok(ReceiveOutcome::Open) => {}
                Ok(ReceiveOutcome::Closed) => {
                    close_stream(&runtime, &mut stream, &connected);
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
                Err(error) => {
                    close_stream(&runtime, &mut stream, &connected);
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
                    continue 'worker;
                }
            }
        } else {
            match command_rx.recv_timeout(WEBSOCKET_WORKER_POLL_INTERVAL) {
                Ok(WebSocketClientWorkerCommand::Send { frame_kind, bytes }) => {
                    let Some(active_stream) = stream.as_mut() else {
                        continue 'worker;
                    };

                    if let Err(error) = send_bytes(&runtime, active_stream, frame_kind, bytes.as_slice()) {
                        close_stream(&runtime, &mut stream, &connected);
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

            continue;
        }

        loop {
            match command_rx.try_recv() {
                Ok(WebSocketClientWorkerCommand::Send { frame_kind, bytes }) => {
                    let Some(active_stream) = stream.as_mut() else {
                        continue 'worker;
                    };

                    if let Err(error) = send_bytes(&runtime, active_stream, frame_kind, bytes.as_slice()) {
                        close_stream(&runtime, &mut stream, &connected);
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
                        continue 'worker;
                    }
                }
                Ok(WebSocketClientWorkerCommand::Stop) => break 'worker,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => break 'worker,
            }
        }
    }

    close_stream(&runtime, &mut stream, &connected);
}

fn receive_messages(
    runtime: &Runtime,
    stream: &mut ClientWebSocketStream,
    event_tx: &PendingSender<StreamingWorkerEvent>,
) -> Result<ReceiveOutcome, String> {
    let mut poll_interval = WEBSOCKET_WORKER_POLL_INTERVAL;

    loop {
        match poll_next_message(runtime, stream, poll_interval)? {
            WebSocketPollOutcome::Idle => return Ok(ReceiveOutcome::Open),
            WebSocketPollOutcome::Closed => return Ok(ReceiveOutcome::Closed),
            WebSocketPollOutcome::Message(message) => match message {
                Message::Text(text) => {
                    if event_tx
                        .send(StreamingWorkerEvent::Bytes {
                            frame_kind: StreamingSendFrameKind::Text,
                            bytes: text.to_string().into_bytes(),
                        })
                        .is_err()
                    {
                        return Err("WebSocket client event channel disconnected".to_string());
                    }
                }
                Message::Binary(bytes) => {
                    if event_tx
                        .send(StreamingWorkerEvent::Bytes {
                            frame_kind: StreamingSendFrameKind::Binary,
                            bytes: bytes.to_vec(),
                        })
                        .is_err()
                    {
                        return Err("WebSocket client event channel disconnected".to_string());
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
    stream: &mut ClientWebSocketStream,
    poll_interval: Duration,
) -> Result<WebSocketPollOutcome, String> {
    match runtime.block_on(async { timeout(poll_interval, stream.next()).await }) {
        Err(_) => Ok(WebSocketPollOutcome::Idle),
        Ok(None) => Ok(WebSocketPollOutcome::Closed),
        Ok(Some(Ok(message))) => Ok(WebSocketPollOutcome::Message(message)),
        Ok(Some(Err(error))) => Err(format!("WebSocket client receive error: {error}")),
    }
}

fn flush_stream(runtime: &Runtime, stream: &mut ClientWebSocketStream) -> Result<(), String> {
    runtime
        .block_on(async { stream.flush().await })
        .map_err(|error| error.to_string())
}

fn connect_stream(
    runtime: &Runtime,
    config: &WebSocketClientTransportConfig,
    remote_address: SocketAddr,
) -> Result<ClientWebSocketStream, String> {
    let request = websocket_client_url(
        config.remote_host.as_str(),
        config.remote_port,
        config.path.as_str(),
    );

    runtime.block_on(async {
        let stream = timeout(WEBSOCKET_CONNECT_TIMEOUT, tokio::net::TcpStream::connect(remote_address))
            .await
            .map_err(|_| format!("timed out connecting WebSocket client socket to {remote_address}"))?
            .map_err(|error| {
                format!("failed to connect WebSocket client socket to {remote_address}: {error}")
            })?;

        stream
            .set_nodelay(true)
            .map_err(|error| format!("failed to enable TCP_NODELAY for WebSocket client: {error}"))?;

        let (stream, _response) = timeout(
            WEBSOCKET_HANDSHAKE_TIMEOUT,
            client_async_with_config(request, stream, Some(websocket_config())),
        )
        .await
        .map_err(|_| format!("timed out performing WebSocket handshake with {remote_address}"))?
        .map_err(|error| format!("failed WebSocket client handshake: {error}"))?;

        Ok(stream)
    })
}

fn send_bytes(
    runtime: &Runtime,
    stream: &mut ClientWebSocketStream,
    frame_kind: StreamingSendFrameKind,
    bytes: &[u8],
) -> Result<(), String> {
    let message = websocket_message(frame_kind, bytes)?;
    runtime
        .block_on(async { stream.send(message).await })
        .map_err(|error| format!("failed to write WebSocket frame: {error}"))
}

fn emit_status(
    event_tx: &PendingSender<StreamingWorkerEvent>,
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
    event_tx: &PendingSender<StreamingWorkerEvent>,
    connected: &Arc<AtomicBool>,
    last_status: &mut Option<WebSocketClientConnectionStatus>,
    reconnect_delay: &mut ReconnectBackoff,
    next_connect_at: &mut Instant,
    remote_address: SocketAddr,
    message: String,
) -> bool {
    connected.store(false, Ordering::Release);
    *next_connect_at = reconnect_delay.schedule(Instant::now());

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

fn close_stream(
    runtime: &Runtime,
    stream: &mut Option<ClientWebSocketStream>,
    connected: &Arc<AtomicBool>,
) {
    connected.store(false, Ordering::Release);
    if let Some(active_stream) = stream.as_mut() {
        let _ = runtime.block_on(active_stream.close(None));
    }
    stream.take();
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
