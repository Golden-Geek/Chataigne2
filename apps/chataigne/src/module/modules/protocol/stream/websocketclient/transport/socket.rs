use super::WebSocketClientTransportConfig;
use crate::app::module::common::streaming::websocket::{websocket_client_url, websocket_config};
use golden_io::WorkerCancellation;
use std::{
    io::{self, Read, Write},
    net::{SocketAddr, TcpStream, ToSocketAddrs},
    time::{Duration, Instant},
};
use tokio_tungstenite::tungstenite::{client::client_with_config, handshake::HandshakeError, WebSocket};

pub(super) type WireSocket = WebSocket<BudgetSocket>;
const IO_BYTES: usize = 64 * 1024;
const IO_CALLS: usize = 4;
const POLL: Duration = Duration::from_millis(5);

/// Bound even protocol-internal fragment/control loops and partial flushes.
pub(super) struct BudgetSocket {
    pub stream: TcpStream,
    read_bytes: usize,
    write_bytes: usize,
    reads: usize,
    writes: usize,
}
impl BudgetSocket {
    pub fn new(stream: TcpStream) -> Self {
        let mut socket = Self {
            stream,
            read_bytes: 0,
            write_bytes: 0,
            reads: 0,
            writes: 0,
        };
        socket.reset();
        socket
    }
    pub fn reset(&mut self) {
        self.read_bytes = IO_BYTES;
        self.write_bytes = IO_BYTES;
        self.reads = 0;
        self.writes = 0;
    }
    pub fn remote(&self) -> io::Result<SocketAddr> {
        self.stream.peer_addr()
    }
}
impl Read for BudgetSocket {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.read_bytes == 0 || self.reads >= IO_CALLS {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        self.reads += 1;
        let size = buffer.len().min(self.read_bytes);
        let read = self.stream.read(&mut buffer[..size])?;
        self.read_bytes -= read;
        Ok(read)
    }
}
impl Write for BudgetSocket {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.write_bytes == 0 || self.writes >= IO_CALLS {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        self.writes += 1;
        let size = buffer.len().min(self.write_bytes);
        let written = self.stream.write(&buffer[..size])?;
        self.write_bytes -= written;
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

pub(super) fn connect(
    config: &WebSocketClientTransportConfig,
    cancel: &WorkerCancellation,
    stopped: &dyn Fn() -> bool,
) -> Result<WireSocket, String> {
    let addresses = (config.remote_host.trim(), config.remote_port)
        .to_socket_addrs()
        .map_err(|_| "WebSocket endpoint resolution failed".to_string())?;
    for address in addresses.take(8) {
        if cancel.is_cancelled() || stopped() {
            return Err("WebSocket setup cancelled".into());
        }
        let Ok(stream) = TcpStream::connect_timeout(&address, Duration::from_millis(100)) else {
            continue;
        };
        stream.set_nonblocking(true).map_err(|error| error.to_string())?;
        stream.set_nodelay(true).map_err(|error| error.to_string())?;
        let url = websocket_client_url(&config.remote_host, config.remote_port, &config.path);
        let limits = websocket_config()
            .read_buffer_size(8192)
            .write_buffer_size(0)
            .max_write_buffer_size(2 * 1024 * 1024 + 1024);
        let mut result = client_with_config(url, BudgetSocket::new(stream), Some(limits));
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if cancel.is_cancelled() || stopped() {
                return Err("WebSocket handshake cancelled".into());
            }
            match result {
                Ok((socket, _response)) => return Ok(socket),
                Err(HandshakeError::Failure(error)) => return Err(format!("WebSocket handshake failed: {error}")),
                Err(HandshakeError::Interrupted(mut handshake)) => {
                    if Instant::now() >= deadline {
                        return Err("WebSocket handshake deadline exceeded".into());
                    }
                    if cancel.wait(POLL) || stopped() {
                        return Err("WebSocket handshake cancelled".into());
                    }
                    handshake.get_mut().get_mut().reset();
                    result = handshake.handshake();
                }
            }
        }
    }
    Err("WebSocket endpoint has no reachable address".into())
}
