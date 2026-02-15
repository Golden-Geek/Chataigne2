use std::io::{Error, ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use golden_core::engine::{Engine, EngineTime};
use golden_core::node::Node;
use golden_core::ui_sync::{UiAck, UiEditIntent, UiSubscriptionScope};
use serde::Deserialize;

#[derive(Clone)]
pub struct UiServerConfig {
    pub bind_addr: String,
    pub tick_interval: Duration,
}

impl Default for UiServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:7010".to_string(),
            tick_interval: Duration::from_millis(16),
        }
    }
}

#[derive(Clone)]
struct ServerState<T: Node> {
    engine: Arc<Mutex<Engine<T>>>,
}

#[derive(Deserialize)]
struct SnapshotRequest {
    #[serde(default)]
    scope: UiSubscriptionScope,
}

#[derive(Deserialize)]
struct ReplayRequest {
    #[serde(default)]
    scope: UiSubscriptionScope,
    #[serde(default)]
    from: Option<EngineTime>,
}

struct HttpRequest {
    method: String,
    path: String,
    body: Vec<u8>,
}

pub fn run_ui_server<T: Node + 'static>(engine: Arc<Mutex<Engine<T>>>, config: UiServerConfig) -> std::io::Result<()> {
    spawn_runtime_loop(engine.clone(), config.tick_interval);

    let listener = TcpListener::bind(&config.bind_addr)?;
    println!("UI API listening on http://{}", config.bind_addr);

    let state = ServerState { engine };

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                if let Err(err) = handle_connection(&mut stream, &state) {
                    eprintln!("ui server request failed: {err}");
                }
            }
            Err(err) => {
                eprintln!("ui server accept failed: {err}");
            }
        }
    }

    Ok(())
}

fn spawn_runtime_loop<T: Node + 'static>(engine: Arc<Mutex<Engine<T>>>, tick_interval: Duration) {
    thread::spawn(move || {
        let mut last_tick_start = Instant::now();

        loop {
            let tick_start = Instant::now();
            let elapsed = tick_start.saturating_duration_since(last_tick_start);
            last_tick_start = tick_start;

            let mut guard = match engine.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };

            if let Err(err) = guard.run_tick(elapsed) {
                eprintln!("runtime tick failed: {err}");
            }
            drop(guard);

            let spent = tick_start.elapsed();
            if spent < tick_interval {
                thread::sleep(tick_interval - spent);
            }
        }
    });
}

fn handle_connection<T: Node>(stream: &mut TcpStream, state: &ServerState<T>) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;

    let request = match read_http_request(stream) {
        Ok(request) => request,
        Err(err) => {
            write_json_error(stream, "400 Bad Request", &format!("invalid request: {err}"))?;
            return Ok(());
        }
    };

    if request.method.eq_ignore_ascii_case("OPTIONS") {
        write_response(stream, "204 No Content", "text/plain", &[])?;
        return Ok(());
    }

    let route = request.path.as_str();
    match (request.method.as_str(), route) {
        ("GET", "/api/ui/health") => {
            write_json(stream, "200 OK", &serde_json::json!({ "ok": true }))?;
        }
        ("POST", "/api/ui/snapshot") => {
            let payload: SnapshotRequest = if request.body.is_empty() {
                SnapshotRequest { scope: UiSubscriptionScope::WholeGraph }
            } else {
                serde_json::from_slice(&request.body).map_err(|err| Error::new(ErrorKind::InvalidData, err.to_string()))?
            };

            let guard = lock_engine(&state.engine);
            let snapshot = guard.ui_snapshot(payload.scope);
            drop(guard);

            write_json(stream, "200 OK", &snapshot)?;
        }
        ("POST", "/api/ui/replay") => {
            let payload: ReplayRequest = serde_json::from_slice(&request.body).map_err(|err| Error::new(ErrorKind::InvalidData, format!("invalid replay payload: {err}")))?;

            let guard = lock_engine(&state.engine);
            let batch = guard.ui_event_batch(payload.from, payload.scope);
            drop(guard);

            write_json(stream, "200 OK", &batch)?;
        }
        ("POST", "/api/ui/intent") => {
            let intent: UiEditIntent = serde_json::from_slice(&request.body).map_err(|err| Error::new(ErrorKind::InvalidData, format!("invalid intent payload: {err}")))?;

            let mut guard = lock_engine(&state.engine);
            let ack = guard.apply_ui_intent(intent);
            drop(guard);

            write_json(stream, "200 OK", &ack)?;
        }
        ("POST", "/api/ui/intent/batch") => {
            let intents: Vec<UiEditIntent> = serde_json::from_slice(&request.body).map_err(|err| Error::new(ErrorKind::InvalidData, format!("invalid intent batch payload: {err}")))?;

            let mut guard = lock_engine(&state.engine);
            let mut acks = Vec::<UiAck>::with_capacity(intents.len());
            for intent in intents {
                acks.push(guard.apply_ui_intent(intent));
            }
            drop(guard);

            write_json(stream, "200 OK", &acks)?;
        }
        _ => {
            write_json_error(stream, "404 Not Found", "unknown route")?;
        }
    }

    Ok(())
}

fn lock_engine<T: Node>(engine: &Arc<Mutex<Engine<T>>>) -> std::sync::MutexGuard<'_, Engine<T>> {
    match engine.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn read_http_request(stream: &mut TcpStream) -> std::io::Result<HttpRequest> {
    let mut buffer = Vec::<u8>::with_capacity(2048);
    let mut temp = [0u8; 1024];
    let mut header_end = None::<usize>;
    let mut content_length = 0usize;

    loop {
        let read = stream.read(&mut temp)?;
        if read == 0 {
            break;
        }

        buffer.extend_from_slice(&temp[..read]);
        if buffer.len() > 1024 * 1024 {
            return Err(Error::new(ErrorKind::InvalidData, "request exceeds 1MB"));
        }

        if header_end.is_none() {
            if let Some(idx) = find_header_end(&buffer) {
                header_end = Some(idx + 4);
                let header_text = std::str::from_utf8(&buffer[..idx]).map_err(|err| Error::new(ErrorKind::InvalidData, format!("invalid header utf-8: {err}")))?;
                content_length = parse_content_length(header_text)?;
            }
        }

        if let Some(end) = header_end {
            if buffer.len() >= end + content_length {
                break;
            }
        }
    }

    let header_end = header_end.ok_or_else(|| Error::new(ErrorKind::InvalidData, "malformed request: missing header terminator"))?;
    if buffer.len() < header_end + content_length {
        return Err(Error::new(ErrorKind::UnexpectedEof, "request body truncated"));
    }

    let header_text = std::str::from_utf8(&buffer[..header_end - 4]).map_err(|err| Error::new(ErrorKind::InvalidData, format!("invalid header utf-8: {err}")))?;
    let mut lines = header_text.lines();
    let request_line = lines.next().ok_or_else(|| Error::new(ErrorKind::InvalidData, "missing request line"))?;

    let mut parts = request_line.split_whitespace();
    let method = parts.next().ok_or_else(|| Error::new(ErrorKind::InvalidData, "missing method"))?.to_string();
    let target = parts.next().ok_or_else(|| Error::new(ErrorKind::InvalidData, "missing request target"))?;

    let path = target.split('?').next().unwrap_or(target).to_string();
    let body = buffer[header_end..header_end + content_length].to_vec();

    Ok(HttpRequest { method, path, body })
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|chunk| chunk == b"\r\n\r\n")
}

fn parse_content_length(headers: &str) -> std::io::Result<usize> {
    for line in headers.lines() {
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                return value.trim().parse::<usize>().map_err(|err| Error::new(ErrorKind::InvalidData, format!("invalid content-length: {err}")));
            }
        }
    }

    Ok(0)
}

fn write_json<T: serde::Serialize>(stream: &mut TcpStream, status: &str, payload: &T) -> std::io::Result<()> {
    let body = serde_json::to_vec(payload).map_err(|err| Error::new(ErrorKind::InvalidData, format!("failed to serialize json: {err}")))?;
    write_response(stream, status, "application/json; charset=utf-8", &body)
}

fn write_json_error(stream: &mut TcpStream, status: &str, message: &str) -> std::io::Result<()> {
    write_json(stream, status, &serde_json::json!({ "error": message }))
}

fn write_response(stream: &mut TcpStream, status: &str, content_type: &str, body: &[u8]) -> std::io::Result<()> {
    let headers = format!(
        "HTTP/1.1 {status}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
         Access-Control-Allow-Headers: Content-Type\r\n\
         Connection: close\r\n\r\n",
        body.len()
    );

    stream.write_all(headers.as_bytes())?;
    if !body.is_empty() {
        stream.write_all(body)?;
    }
    stream.flush()?;
    Ok(())
}
