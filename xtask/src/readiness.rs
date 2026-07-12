use serde_json::Value;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq)]
pub struct UiReadiness {
    pub active_websocket_clients: u64,
    pub active_subscribed_websocket_clients: u64,
    pub read_model_revision: Value,
}

const PROBE_CONNECT_TIMEOUT: Duration = Duration::from_millis(750);
const PROBE_RESPONSE_TIMEOUT: Duration = Duration::from_secs(5);

pub fn ensure_port_available(component: &str, port: u16) -> Result<(), String> {
    let address = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
    match TcpListener::bind(address) {
        Ok(listener) => {
            drop(listener);
            Ok(())
        }
        Err(error) => Err(format!(
            "{component} port {port} is unavailable at 127.0.0.1:{port}: {error}. {}",
            occupied_port_help(port)
        )),
    }
}

pub fn probe_frontend(port: u16) -> Result<(), String> {
    let response = request(port, "GET", "/", None)?;
    if (200..400).contains(&response.status) {
        Ok(())
    } else {
        Err(format!("frontend returned HTTP {} from /", response.status))
    }
}

pub fn probe_backend_health(port: u16) -> Result<(), String> {
    let payload = request_ui_readiness(port)?;
    if payload.get("backend_ready").and_then(Value::as_bool) == Some(true) {
        Ok(())
    } else {
        Err("UI readiness response did not contain backend_ready=true".to_string())
    }
}

pub fn probe_engine_read_model(port: u16) -> Result<(), String> {
    let payload = request_ui_readiness(port)?;
    if payload.get("engine_read_model_ready").and_then(Value::as_bool) == Some(true) {
        Ok(())
    } else {
        Err("UI readiness response did not contain engine_read_model_ready=true".to_string())
    }
}

pub fn probe_active_ui_session(port: u16) -> Result<UiReadiness, String> {
    let payload = request_ui_readiness(port)?;
    if payload.get("backend_ready").and_then(Value::as_bool) != Some(true) {
        return Err("backend is not ready".to_string());
    }
    if payload.get("engine_read_model_ready").and_then(Value::as_bool) != Some(true) {
        return Err("engine read model is not ready".to_string());
    }

    let active_websocket_clients = payload
        .get("active_websocket_clients")
        .and_then(Value::as_u64)
        .ok_or_else(|| "UI readiness response omitted active_websocket_clients".to_string())?;
    let active_subscribed_websocket_clients = payload
        .get("active_subscribed_websocket_clients")
        .and_then(Value::as_u64)
        .ok_or_else(|| "UI readiness response omitted active_subscribed_websocket_clients".to_string())?;
    if active_subscribed_websocket_clients == 0 {
        return Err("no subscribed UI WebSocket session is active".to_string());
    }
    if active_subscribed_websocket_clients > active_websocket_clients {
        return Err("subscribed UI client count exceeds connected client count".to_string());
    }
    let read_model_revision = payload
        .get("read_model_revision")
        .cloned()
        .ok_or_else(|| "UI readiness response omitted read_model_revision".to_string())?;

    Ok(UiReadiness {
        active_websocket_clients,
        active_subscribed_websocket_clients,
        read_model_revision,
    })
}

fn request_ui_readiness(port: u16) -> Result<Value, String> {
    let response = request(port, "GET", "/api/ui/health", None)?;
    require_ok_status("UI readiness", &response)?;
    let payload: Value = serde_json::from_slice(&response.body)
        .map_err(|error| format!("UI readiness returned invalid JSON: {error}"))?;
    if payload.get("version").and_then(Value::as_u64) != Some(1) {
        return Err("UI readiness response has an unsupported version".to_string());
    }
    Ok(payload)
}

fn require_ok_status(component: &str, response: &HttpResponse) -> Result<(), String> {
    if response.status == 200 {
        Ok(())
    } else {
        Err(format!("{component} returned HTTP {}", response.status))
    }
}

struct HttpResponse {
    status: u16,
    body: Vec<u8>,
}

fn request(port: u16, method: &str, path: &str, body: Option<&[u8]>) -> Result<HttpResponse, String> {
    let address = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port));
    let mut stream = TcpStream::connect_timeout(&address, PROBE_CONNECT_TIMEOUT)
        .map_err(|error| format!("127.0.0.1:{port} is not reachable: {error}"))?;
    stream
        .set_read_timeout(Some(PROBE_RESPONSE_TIMEOUT))
        .map_err(|error| format!("failed to configure readiness read timeout: {error}"))?;
    stream
        .set_write_timeout(Some(PROBE_RESPONSE_TIMEOUT))
        .map_err(|error| format!("failed to configure readiness write timeout: {error}"))?;

    let body = body.unwrap_or_default();
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    stream
        .write_all(request.as_bytes())
        .and_then(|()| stream.write_all(body))
        .map_err(|error| format!("failed to write readiness request: {error}"))?;

    let mut bytes = Vec::new();
    match stream.read_to_end(&mut bytes) {
        Ok(_) => {}
        Err(error) if !bytes.is_empty() => {
            // A complete response can arrive before a keep-alive peer closes. Parsing below
            // validates that the buffered response is usable.
            let _ = error;
        }
        Err(error) => return Err(format!("failed to read readiness response: {error}")),
    }
    parse_response(&bytes)
}

fn parse_response(bytes: &[u8]) -> Result<HttpResponse, String> {
    let header_end = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| "readiness endpoint returned an incomplete HTTP response".to_string())?;
    let headers = std::str::from_utf8(&bytes[..header_end])
        .map_err(|error| format!("readiness endpoint returned invalid HTTP headers: {error}"))?;
    let status = headers
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| "readiness endpoint returned an invalid HTTP status line".to_string())?;
    Ok(HttpResponse {
        status,
        body: bytes[header_end + 4..].to_vec(),
    })
}

fn occupied_port_help(port: u16) -> String {
    if cfg!(windows) {
        format!(
            "Stop the listener shown by 'Get-NetTCPConnection -LocalPort {port}', or select another port with --frontend-port/--backend-port."
        )
    } else {
        format!(
            "Stop the listener shown by 'lsof -nP -iTCP:{port} -sTCP:LISTEN', or select another port with --frontend-port/--backend-port."
        )
    }
}
