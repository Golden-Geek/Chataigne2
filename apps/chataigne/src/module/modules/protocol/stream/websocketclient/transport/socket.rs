use super::WebSocketClientTransportConfig;
use crate::app::module::common::streaming::websocket::socket::{BudgetSocket, WireSocket};
use crate::app::module::common::streaming::websocket::{websocket_client_url, websocket_config};
use golden_io::WorkerCancellation;
use std::{
    net::{TcpStream, ToSocketAddrs},
    time::{Duration, Instant},
};
use tokio_tungstenite::tungstenite::{client::client_with_config, handshake::HandshakeError};

const POLL: Duration = Duration::from_millis(5);

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
        let limits = websocket_config();
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
