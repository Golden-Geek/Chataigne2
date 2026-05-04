use std::{
    io::{Read, Write},
    net::{Shutdown, SocketAddr, TcpStream, ToSocketAddrs},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const TCP_CONNECT_TIMEOUT: Duration = Duration::from_millis(500);
const TCP_RECONNECT_BASE_DELAY: Duration = Duration::from_millis(250);
const TCP_RECONNECT_MAX_DELAY: Duration = Duration::from_secs(5);
const TCP_WORKER_POLL_INTERVAL: Duration = Duration::from_millis(5);
const TCP_HEALTH_CHECK_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TcpStreamingTransportConfig {
    pub remote_host: String,
    pub remote_port: u16,
    pub receive_enabled: bool,
    pub send_enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TcpStreamingConnectionStatus {
    Connected { remote_address: SocketAddr },
    Recovering { remote_address: SocketAddr, message: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum StreamingWorkerEvent {
    Bytes(Vec<u8>),
    Error(String),
    Status(TcpStreamingConnectionStatus),
}

pub(crate) struct TcpStreamingTransportHandle {
    command_tx: Sender<TcpStreamingWorkerCommand>,
    event_rx: Receiver<StreamingWorkerEvent>,
    connected: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl TcpStreamingTransportHandle {
    pub(crate) fn spawn(config: TcpStreamingTransportConfig) -> Result<Self, String> {
        let remote_address = resolve_socket_addr(config.remote_host.as_str(), config.remote_port)?;

        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let connected = Arc::new(AtomicBool::new(false));
        let worker_connected = Arc::clone(&connected);

        let worker = thread::Builder::new()
            .name(format!("streaming-tcp-{}", remote_address.port()))
            .spawn(move || worker_loop(config, remote_address, command_rx, event_tx, worker_connected))
            .map_err(|error| format!("failed to start TCP worker thread: {error}"))?;

        Ok(Self {
            command_tx,
            event_rx,
            connected,
            worker: Some(worker),
        })
    }

    pub(crate) fn send(&self, bytes: Vec<u8>) -> Result<(), String> {
        if !self.connected.load(Ordering::Acquire) {
            return Err("TCP transport is not connected".to_string());
        }

        self.command_tx
            .send(TcpStreamingWorkerCommand::Send(bytes))
            .map_err(|_| "TCP worker is no longer running".to_string())
    }

    pub(crate) fn try_recv(&self) -> Result<StreamingWorkerEvent, mpsc::TryRecvError> {
        self.event_rx.try_recv()
    }

    pub(crate) fn stop(&mut self) {
        let _ = self.command_tx.send(TcpStreamingWorkerCommand::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for TcpStreamingTransportHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

enum TcpStreamingWorkerCommand {
    Send(Vec<u8>),
    Stop,
}

fn worker_loop(
    config: TcpStreamingTransportConfig,
    remote_address: SocketAddr,
    command_rx: Receiver<TcpStreamingWorkerCommand>,
    event_tx: Sender<StreamingWorkerEvent>,
    connected: Arc<AtomicBool>,
) {
    let mut buffer = [0u8; 8192];
    let mut stream = None;
    let mut reconnect_delay = TCP_RECONNECT_BASE_DELAY;
    let mut next_connect_at = Instant::now();
    let mut last_status = None;
    let mut last_health_check = Instant::now();

    loop {
        if stream.is_none() {
            if Instant::now() >= next_connect_at {
                match connect_stream(remote_address) {
                    Ok(open_stream) => {
                        stream = Some(open_stream);
                        reconnect_delay = TCP_RECONNECT_BASE_DELAY;
                        last_health_check = Instant::now();
                        if !emit_status(
                            &event_tx,
                            &connected,
                            &mut last_status,
                            TcpStreamingConnectionStatus::Connected { remote_address },
                        ) {
                            return;
                        }
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
                Ok(TcpStreamingWorkerCommand::Send(bytes)) => {
                    if !config.send_enabled {
                        let _ = event_tx.send(StreamingWorkerEvent::Error(
                            "TCP sender is disabled; outgoing bytes were dropped".to_string(),
                        ));
                    } else if event_tx
                        .send(StreamingWorkerEvent::Error(
                            "TCP transport is reconnecting; outgoing bytes were dropped".to_string(),
                        ))
                        .is_err()
                    {
                        return;
                    }
                    drop(bytes);
                }
                Ok(TcpStreamingWorkerCommand::Stop) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }

            continue;
        }

        if drain_commands(&command_rx, &event_tx, stream.as_mut(), config.send_enabled) {
            break;
        }

        if let Some(active_stream) = stream.as_mut() {
            if config.receive_enabled {
                match active_stream.read(&mut buffer) {
                    Ok(0) => {
                        close_stream(&mut stream);
                        if !enter_recovery(
                            &event_tx,
                            &connected,
                            &mut last_status,
                            &mut reconnect_delay,
                            &mut next_connect_at,
                            remote_address,
                            "TCP peer closed the connection".to_string(),
                        ) {
                            return;
                        }
                        continue;
                    }
                    Ok(length) => {
                        if event_tx
                            .send(StreamingWorkerEvent::Bytes(buffer[..length].to_vec()))
                            .is_err()
                        {
                            return;
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(error) => {
                        close_stream(&mut stream);
                        if !enter_recovery(
                            &event_tx,
                            &connected,
                            &mut last_status,
                            &mut reconnect_delay,
                            &mut next_connect_at,
                            remote_address,
                            format!("TCP stream receive error: {error}"),
                        ) {
                            return;
                        }
                        continue;
                    }
                }
            }

            if last_health_check.elapsed() >= TCP_HEALTH_CHECK_INTERVAL {
                if let Err(error) = poll_stream_health(active_stream) {
                    close_stream(&mut stream);
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
                    continue;
                }
                last_health_check = Instant::now();
            }
        }

        match command_rx.recv_timeout(TCP_WORKER_POLL_INTERVAL) {
            Ok(TcpStreamingWorkerCommand::Send(bytes)) => {
                if let Err(error) = write_bytes(&event_tx, stream.as_mut(), config.send_enabled, bytes) {
                    close_stream(&mut stream);
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
            Ok(TcpStreamingWorkerCommand::Stop) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    connected.store(false, Ordering::Release);
    close_stream(&mut stream);
}

fn drain_commands(
    command_rx: &Receiver<TcpStreamingWorkerCommand>,
    event_tx: &Sender<StreamingWorkerEvent>,
    mut stream: Option<&mut TcpStream>,
    send_enabled: bool,
) -> bool {
    loop {
        match command_rx.try_recv() {
            Ok(TcpStreamingWorkerCommand::Send(bytes)) => {
                if write_bytes(event_tx, stream.as_deref_mut(), send_enabled, bytes).is_err() {
                    return false;
                }
            }
            Ok(TcpStreamingWorkerCommand::Stop) => return true,
            Err(mpsc::TryRecvError::Empty) => return false,
            Err(mpsc::TryRecvError::Disconnected) => return true,
        }
    }
}

fn write_bytes(
    event_tx: &Sender<StreamingWorkerEvent>,
    stream: Option<&mut TcpStream>,
    send_enabled: bool,
    bytes: Vec<u8>,
) -> Result<(), String> {
    if !send_enabled {
        let _ = event_tx.send(StreamingWorkerEvent::Error(
            "TCP sender is disabled; outgoing bytes were dropped".to_string(),
        ));
        return Ok(());
    }

    let Some(stream) = stream else {
        let _ = event_tx.send(StreamingWorkerEvent::Error(
            "TCP transport is reconnecting; outgoing bytes were dropped".to_string(),
        ));
        return Ok(());
    };

    if let Err(error) = stream.write_all(bytes.as_slice()) {
        return Err(format!("failed to write TCP stream: {error}"));
    }

    Ok(())
}

fn connect_stream(remote_address: SocketAddr) -> Result<TcpStream, String> {
    let stream = TcpStream::connect_timeout(&remote_address, TCP_CONNECT_TIMEOUT)
        .map_err(|error| format!("failed to connect TCP stream to {remote_address}: {error}"))?;
    stream
        .set_nonblocking(true)
        .map_err(|error| format!("failed to set TCP stream to non-blocking mode: {error}"))?;

    Ok(stream)
}

fn poll_stream_health(stream: &TcpStream) -> Result<(), String> {
    if let Some(error) = stream
        .take_error()
        .map_err(|error| format!("failed to read TCP socket state: {error}"))?
    {
        return Err(format!("TCP stream error: {error}"));
    }

    let mut probe = [0u8; 1];
    match stream.peek(&mut probe) {
        Ok(0) => Err("TCP peer closed the connection".to_string()),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(()),
        Err(error) if is_disconnect_error(&error) => Err(format!("TCP stream health check failed: {error}")),
        Err(error) => Err(format!("TCP stream health check failed: {error}")),
    }
}

fn emit_status(
    event_tx: &Sender<StreamingWorkerEvent>,
    connected: &Arc<AtomicBool>,
    last_status: &mut Option<TcpStreamingConnectionStatus>,
    next_status: TcpStreamingConnectionStatus,
) -> bool {
    connected.store(
        matches!(next_status, TcpStreamingConnectionStatus::Connected { .. }),
        Ordering::Release,
    );

    if last_status.as_ref() == Some(&next_status) {
        return true;
    }

    *last_status = Some(next_status.clone());
    event_tx.send(StreamingWorkerEvent::Status(next_status)).is_ok()
}

fn enter_recovery(
    event_tx: &Sender<StreamingWorkerEvent>,
    connected: &Arc<AtomicBool>,
    last_status: &mut Option<TcpStreamingConnectionStatus>,
    reconnect_delay: &mut Duration,
    next_connect_at: &mut Instant,
    remote_address: SocketAddr,
    message: String,
) -> bool {
    *next_connect_at = Instant::now() + *reconnect_delay;
    *reconnect_delay = reconnect_delay.saturating_mul(2).min(TCP_RECONNECT_MAX_DELAY);

    emit_status(
        event_tx,
        connected,
        last_status,
        TcpStreamingConnectionStatus::Recovering {
            remote_address,
            message,
        },
    )
}

fn reconnect_wait(next_connect_at: Instant) -> Duration {
    let wait = next_connect_at.saturating_duration_since(Instant::now());
    if wait.is_zero() {
        TCP_WORKER_POLL_INTERVAL
    } else {
        wait.min(TCP_WORKER_POLL_INTERVAL)
    }
}

fn close_stream(stream: &mut Option<TcpStream>) {
    if let Some(active_stream) = stream.take() {
        let _ = active_stream.shutdown(Shutdown::Both);
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
        .map_err(|error| format!("failed to resolve TCP address '{address}': {error}"))?;

    resolved
        .next()
        .ok_or_else(|| format!("TCP address '{address}' did not resolve to a socket address"))
}
