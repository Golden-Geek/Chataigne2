use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream, ToSocketAddrs},
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle},
    time::Duration,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TcpStreamingTransportConfig {
    pub remote_host: String,
    pub remote_port: u16,
    pub receive_enabled: bool,
    pub send_enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum StreamingWorkerEvent {
    Bytes(Vec<u8>),
    Error(String),
    Stopped(String),
}

pub(crate) struct TcpStreamingTransportHandle {
    command_tx: Sender<TcpStreamingWorkerCommand>,
    event_rx: Receiver<StreamingWorkerEvent>,
    worker: Option<JoinHandle<()>>,
}

impl TcpStreamingTransportHandle {
    pub(crate) fn spawn(config: TcpStreamingTransportConfig) -> Result<Self, String> {
        let remote_address = resolve_socket_addr(config.remote_host.as_str(), config.remote_port)?;
        let stream = TcpStream::connect_timeout(&remote_address, Duration::from_millis(500))
            .map_err(|error| format!("failed to connect TCP stream to {remote_address}: {error}"))?;
        stream
            .set_nonblocking(true)
            .map_err(|error| format!("failed to set TCP stream to non-blocking mode: {error}"))?;

        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();

        let worker = thread::Builder::new()
            .name(format!("streaming-tcp-{}", remote_address.port()))
            .spawn(move || worker_loop(stream, config, command_rx, event_tx))
            .map_err(|error| format!("failed to start TCP worker thread: {error}"))?;

        Ok(Self {
            command_tx,
            event_rx,
            worker: Some(worker),
        })
    }

    pub(crate) fn send(&self, bytes: Vec<u8>) -> Result<(), String> {
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
    mut stream: TcpStream,
    config: TcpStreamingTransportConfig,
    command_rx: Receiver<TcpStreamingWorkerCommand>,
    event_tx: Sender<StreamingWorkerEvent>,
) {
    let mut buffer = [0u8; 8192];

    loop {
        if drain_commands(&command_rx, &event_tx, &mut stream, config.send_enabled) {
            break;
        }

        if config.receive_enabled {
            match stream.read(&mut buffer) {
                Ok(0) => {
                    let _ = event_tx.send(StreamingWorkerEvent::Stopped(
                        "TCP peer closed the connection".to_string(),
                    ));
                    break;
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
                    let _ = event_tx.send(StreamingWorkerEvent::Stopped(format!(
                        "TCP stream receive error: {error}"
                    )));
                    break;
                }
            }
        }

        match command_rx.recv_timeout(Duration::from_millis(5)) {
            Ok(TcpStreamingWorkerCommand::Send(bytes)) => {
                write_bytes(&event_tx, &mut stream, config.send_enabled, bytes);
            }
            Ok(TcpStreamingWorkerCommand::Stop) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn drain_commands(
    command_rx: &Receiver<TcpStreamingWorkerCommand>,
    event_tx: &Sender<StreamingWorkerEvent>,
    stream: &mut TcpStream,
    send_enabled: bool,
) -> bool {
    loop {
        match command_rx.try_recv() {
            Ok(TcpStreamingWorkerCommand::Send(bytes)) => write_bytes(event_tx, stream, send_enabled, bytes),
            Ok(TcpStreamingWorkerCommand::Stop) => return true,
            Err(mpsc::TryRecvError::Empty) => return false,
            Err(mpsc::TryRecvError::Disconnected) => return true,
        }
    }
}

fn write_bytes(
    event_tx: &Sender<StreamingWorkerEvent>,
    stream: &mut TcpStream,
    send_enabled: bool,
    bytes: Vec<u8>,
) {
    if !send_enabled {
        let _ = event_tx.send(StreamingWorkerEvent::Error(
            "TCP sender is disabled; outgoing bytes were dropped".to_string(),
        ));
        return;
    }

    if let Err(error) = stream.write_all(bytes.as_slice()) {
        let _ = event_tx.send(StreamingWorkerEvent::Stopped(format!(
            "failed to write TCP stream: {error}"
        )));
    }
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
