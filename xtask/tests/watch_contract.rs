use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener};
use std::thread;
use std::time::Duration;
use xtask::readiness;
use xtask::{ParseOutcome, WatchConfig};

#[test]
fn watch_defaults_match_the_checked_in_development_contract() {
    let ParseOutcome::Watch(config) = WatchConfig::parse(["watch"]).expect("watch should parse") else {
        panic!("watch should not return help");
    };

    assert_eq!(config.frontend_port, 5173);
    assert_eq!(config.backend_port, 7010);
    assert_eq!(config.frontend_timeout, Duration::from_secs(60));
    assert_eq!(config.backend_timeout, Duration::from_secs(300));
    assert_eq!(config.engine_timeout, Duration::from_secs(30));
    assert_eq!(config.poll_interval, Duration::from_millis(200));
    assert!(!config.headless);
    assert!(config.app_args.is_empty());
}

#[test]
fn watch_parses_overrides_and_forwards_explicit_app_arguments() {
    let ParseOutcome::Watch(config) = WatchConfig::parse([
        "watch",
        "--frontend-port",
        "15173",
        "--backend-port",
        "17010",
        "--frontend-timeout-secs",
        "5",
        "--backend-timeout-secs",
        "6",
        "--engine-timeout-secs",
        "7",
        "--poll-ms",
        "25",
        "--headless",
        "--",
        "--no-remote",
    ])
    .expect("watch overrides should parse") else {
        panic!("watch should not return help");
    };

    assert_eq!(config.frontend_port, 15173);
    assert_eq!(config.backend_port, 17010);
    assert_eq!(config.frontend_timeout, Duration::from_secs(5));
    assert_eq!(config.backend_timeout, Duration::from_secs(6));
    assert_eq!(config.engine_timeout, Duration::from_secs(7));
    assert_eq!(config.poll_interval, Duration::from_millis(25));
    assert!(config.headless);
    assert_eq!(config.app_args, ["--no-remote"]);
}

#[test]
fn watch_rejects_ambiguous_or_unbounded_configuration() {
    assert!(WatchConfig::parse(["watch", "--frontend-port", "7010"]).is_err());
    assert!(WatchConfig::parse(["watch", "--poll-ms", "0"]).is_err());
    assert!(WatchConfig::parse(["watch", "--backend-timeout-secs", "0"]).is_err());
}

#[test]
fn readiness_probes_distinguish_frontend_backend_and_engine_contracts() {
    let frontend = serve_once("200 OK", "text/html", "<main>Chataigne</main>");
    readiness::probe_frontend(frontend).expect("frontend root should be ready");

    let backend = serve_once("200 OK", "application/json", readiness_json(0));
    readiness::probe_backend_health(backend).expect("backend health should be ready");

    let engine = serve_once("200 OK", "application/json", readiness_json(0));
    readiness::probe_engine_read_model(engine).expect("engine read model should be ready");

    let session = serve_once("200 OK", "application/json", readiness_json(1));
    let session = readiness::probe_active_ui_session(session).expect("session should be ready");
    assert_eq!(session.active_subscribed_websocket_clients, 1);
    assert_eq!(session.read_model_revision["tick"], 7);
}

#[test]
fn session_probe_rejects_a_listener_without_a_subscribed_ui_client() {
    let port = serve_once("200 OK", "application/json", readiness_json(0));
    let error = readiness::probe_active_ui_session(port).expect_err("subscription is required");
    assert!(error.contains("no subscribed UI WebSocket session"));
}

fn readiness_json(active_subscribed_clients: u64) -> &'static str {
    match active_subscribed_clients {
        0 => {
            r#"{"version":1,"backend_ready":true,"engine_read_model_ready":true,"active_websocket_clients":1,"active_subscribed_websocket_clients":0,"read_model_revision":{"tick":7,"micro":0,"seq":3}}"#
        }
        1 => {
            r#"{"version":1,"backend_ready":true,"engine_read_model_ready":true,"active_websocket_clients":1,"active_subscribed_websocket_clients":1,"read_model_revision":{"tick":7,"micro":0,"seq":3}}"#
        }
        other => panic!("unsupported readiness fixture count {other}"),
    }
}

#[test]
fn occupied_port_preflight_names_the_port_and_override() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("test listener should bind");
    let port = listener.local_addr().expect("listener should have address").port();

    let error = readiness::ensure_port_available("frontend", port)
        .expect_err("an occupied port must fail before children start");

    assert!(error.contains(&format!("frontend port {port}")));
    assert!(error.contains("--frontend-port/--backend-port"));
}

fn serve_once(status: &'static str, content_type: &'static str, body: &'static str) -> u16 {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("test listener should bind");
    let port = listener.local_addr().expect("listener should have address").port();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("probe should connect");
        read_request(&mut stream);
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream
            .write_all(response.as_bytes())
            .expect("test response should write");
        stream.flush().expect("test response should flush");
        stream
            .shutdown(Shutdown::Write)
            .expect("test response should close cleanly");
    });
    port
}

fn read_request(stream: &mut impl Read) {
    let mut request = Vec::new();
    let mut chunk = [0_u8; 1024];
    loop {
        let read = stream.read(&mut chunk).expect("probe request should read");
        assert_ne!(read, 0, "probe closed before completing its request");
        request.extend_from_slice(&chunk[..read]);
        let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };
        let headers = std::str::from_utf8(&request[..header_end]).expect("request headers are UTF-8");
        let content_length = headers
            .lines()
            .find_map(|line| {
                line.strip_prefix("Content-Length: ")
                    .and_then(|value| value.parse::<usize>().ok())
            })
            .unwrap_or(0);
        if request.len() >= header_end + 4 + content_length {
            return;
        }
    }
}
