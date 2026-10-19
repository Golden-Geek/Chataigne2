pub(crate) mod socket;

use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;

pub(crate) const WEBSOCKET_MAX_PAYLOAD_BYTES: usize = 1024 * 1024;

pub(crate) fn normalize_websocket_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        "/".to_string()
    } else if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

pub(crate) fn websocket_client_url(remote_host: &str, remote_port: u16, path: &str) -> String {
    let remote_host = remote_host.trim();
    let authority_host = if remote_host.contains(':') && !remote_host.starts_with('[') && !remote_host.ends_with(']') {
        format!("[{remote_host}]")
    } else {
        remote_host.to_string()
    };

    format!("ws://{authority_host}:{remote_port}{}", normalize_websocket_path(path))
}

pub(crate) fn websocket_config() -> WebSocketConfig {
    WebSocketConfig::default()
        .read_buffer_size(8192)
        .write_buffer_size(0)
        .max_write_buffer_size(2 * WEBSOCKET_MAX_PAYLOAD_BYTES + 1024)
        .max_message_size(Some(WEBSOCKET_MAX_PAYLOAD_BYTES))
        .max_frame_size(Some(WEBSOCKET_MAX_PAYLOAD_BYTES))
}
