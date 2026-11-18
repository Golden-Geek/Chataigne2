use super::{Settings, commands, preferences};
use crate::ui_server::{
    HttpRequest, ServerState, build_websocket_request, create_response, write_ws_handshake_response,
};
use golden_engine::app::ProjectLifecycle;
use golden_protocol::UiSubscriptionScope;
use std::{
    io::{self, Write},
    net::TcpStream,
    time::Duration,
};
use tokio_tungstenite::tungstenite::{
    self, Message, WebSocket,
    protocol::{Role, WebSocketConfig},
};

pub(super) const MAX_CLIENTS: usize = 8;
const MAX_MESSAGE: usize = 16 * 1024;
const IDLE_CHECK: Duration = Duration::from_millis(150);

pub(super) fn serve<T: ProjectLifecycle>(
    stream: &mut TcpStream,
    request: &HttpRequest,
    state: &ServerState<T>,
    settings: Settings,
) -> io::Result<()> {
    let request = build_websocket_request(request)?;
    match create_response(&request) {
        Ok(response) => write_ws_handshake_response(&mut *stream, &response).map_err(io::Error::other)?,
        Err(tungstenite::Error::Http(response)) => {
            write_ws_handshake_response(&mut *stream, &response).map_err(io::Error::other)?;
            return stream.flush();
        }
        Err(error) => return Err(io::Error::other(error)),
    }
    stream.flush()?;
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(IDLE_CHECK))?;
    let config = WebSocketConfig::default()
        .read_buffer_size(MAX_MESSAGE)
        .write_buffer_size(0)
        .max_message_size(Some(MAX_MESSAGE))
        .max_frame_size(Some(MAX_MESSAGE));
    let mut socket = WebSocket::from_raw_socket(stream, Role::Server, Some(config));
    loop {
        let snapshot = state.read_model.capture_snapshot(UiSubscriptionScope::WholeGraph);
        if preferences::enabled_settings(&snapshot) != Some(settings) {
            let _ = socket.close(None);
            return Ok(());
        }
        drop(snapshot);
        match socket.read() {
            Ok(Message::Text(text)) => {
                let reply = commands::handle(state, settings, text.as_str());
                socket
                    .send(Message::text(reply.to_string()))
                    .map_err(io::Error::other)?;
            }
            Ok(Message::Ping(_)) => socket.flush().map_err(io::Error::other)?,
            Ok(Message::Close(_)) => {
                let _ = socket.flush();
                return Ok(());
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(error))
                if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => {}
            Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => return Ok(()),
            Err(error) => return Err(io::Error::other(error)),
        }
    }
}
