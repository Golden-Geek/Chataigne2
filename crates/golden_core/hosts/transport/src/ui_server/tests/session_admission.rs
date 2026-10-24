use super::*;
use golden_protocol::{UiSessionCapabilities, UiSessionResponse};
use tokio_tungstenite::tungstenite::{self, client::IntoClientRequest};

golden_engine::define_node_enum!(
    enum SessionTestNode {}
);
impl ProjectLifecycle for SessionTestNode {}

struct TestHost {
    listener: TcpListener,
    state: ServerState<SessionTestNode>,
    commands: Receiver<WsHubCommand>,
}

impl TestHost {
    fn new() -> Self {
        let runtime =
            ProductionRuntime::new(Engine::new(Folder::new("root").into()), UiProjectFileSpec::default()).unwrap();
        let (cmd_tx, commands) = mpsc::sync_channel(64);
        Self {
            listener: TcpListener::bind("127.0.0.1:0").unwrap(),
            commands,
            state: ServerState {
                frontend_security: Arc::new(FrontendSecurity::default()),
                sessions: Arc::new(SessionRegistry::new(UiSessionCapabilities::ALL)),
                request_access: Arc::new(RequestAccessPolicy::new(&["http://127.0.0.1:5173".into()], &[]).unwrap()),
                read_model: runtime.read_model(),
                runtime,
                ws_hub: WsHubHandle {
                    cmd_tx,
                    readiness: Default::default(),
                },
                snapshot_encoding: SnapshotEncodingService::spawn().unwrap(),
                frontend_assets: &[],
                preferences: None,
            },
        }
    }

    fn accept(&self) -> thread::JoinHandle<()> {
        let listener = self.listener.try_clone().unwrap();
        let state = self.state.clone();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            handle_connection(&mut stream, &state).unwrap();
        })
    }

    fn post(
        &self,
        path: &str,
        token: Option<&str>,
        origin: Option<&str>,
        body: serde_json::Value,
    ) -> (String, serde_json::Value) {
        let worker = self.accept();
        let mut client = TcpStream::connect(self.listener.local_addr().unwrap()).unwrap();
        client.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        let authorization = token.map_or(String::new(), |token| format!("Authorization: Bearer {token}\r\n"));
        let origin = origin.map_or(String::new(), |origin| format!("Origin: {origin}\r\n"));
        let body = body.to_string();
        write!(client, "POST {path} HTTP/1.1\r\nHost: {}\r\n{authorization}{origin}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", self.listener.local_addr().unwrap(), body.len()).unwrap();
        let mut response = String::new();
        client.read_to_string(&mut response).unwrap();
        worker.join().unwrap();
        let (headers, body) = response.split_once("\r\n\r\n").unwrap();
        (
            headers.into(),
            if body.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::from_str(body).unwrap()
            },
        )
    }

    fn bootstrap(&self, capabilities: Option<UiSessionCapabilities>, origin: Option<&str>) -> UiSessionResponse {
        let (headers, body) = self.post("/api/ui/session", None, origin, json!({ "capabilities": capabilities }));
        assert!(headers.starts_with("HTTP/1.1 200"), "{headers}: {body}");
        assert!(headers.contains("Cache-Control: no-store"));
        serde_json::from_value(body).unwrap()
    }

    fn websocket(&self, session: &UiSessionResponse) -> (tungstenite::WebSocket<TcpStream>, thread::JoinHandle<()>) {
        let worker = self.accept();
        let address = self.listener.local_addr().unwrap();
        let mut request = format!("ws://{address}/api/ui/ws").into_client_request().unwrap();
        request.headers_mut().insert(
            "Sec-WebSocket-Protocol",
            format!("golden-ui, gc-session.{}", session.token).parse().unwrap(),
        );
        let stream = TcpStream::connect(address).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        let (socket, response) = tungstenite::client(request, stream).unwrap();
        assert_eq!(response.headers()["sec-websocket-protocol"], "golden-ui");
        assert!(!format!("{:?}", response.headers()).contains(&session.token));
        assert!(matches!(
            self.commands.recv_timeout(Duration::from_secs(3)),
            Ok(WsHubCommand::RegisterClient { .. })
        ));
        (socket, worker)
    }
}

#[test]
fn http_missing_and_underprivileged_sessions_fail_before_privileged_dispatch() {
    let host = TestHost::new();
    let readonly = host.bootstrap(
        Some(UiSessionCapabilities {
            observe: true,
            ..Default::default()
        }),
        None,
    );
    for path in [
        "/api/ui/intent",
        "/api/ui/intent/batch",
        "/api/ui/project-new",
        "/api/ui/project-load",
        "/api/ui/project-save",
        "/api/ui/project-upload-load",
        "/api/ui/script-state",
        "/api/ui/script-config",
        "/api/ui/script-reload",
    ] {
        let before = host.state.read_model.current_revision();
        let (headers, _) = host.post(path, None, None, json!("invalid payload"));
        assert!(headers.starts_with("HTTP/1.1 401"), "{path}: {headers}");
        assert!(headers.contains("WWW-Authenticate: Bearer"));
        let (headers, _) = host.post(path, Some(&readonly.token), None, json!("invalid payload"));
        assert!(headers.starts_with("HTTP/1.1 403"), "{path}: {headers}");
        assert_eq!(host.state.read_model.current_revision(), before);
    }
    let (headers, _) = host.post(
        "/api/ui/snapshot",
        Some(&readonly.token),
        None,
        json!({ "scope": "wholeGraph", "cancel_active_edit_session": true }),
    );
    assert!(headers.starts_with("HTTP/1.1 403"));
    let (headers, body) = host.post(
        "/api/ui/snapshot",
        Some(&readonly.token),
        None,
        json!({ "scope": "wholeGraph" }),
    );
    assert!(headers.starts_with("HTTP/1.1 200"), "{body}");
}

#[test]
fn http_denied_save_cannot_create_a_file() {
    let host = TestHost::new();
    let session = host.bootstrap(
        Some(UiSessionCapabilities {
            observe: true,
            ..Default::default()
        }),
        None,
    );
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let path = std::env::temp_dir().join(format!("golden-denied-{}-{nonce}.json", std::process::id()));
    assert!(!path.exists());
    let (headers, _) = host.post(
        "/api/ui/project-save",
        Some(&session.token),
        None,
        json!({ "path": path.to_string_lossy() }),
    );
    assert!(headers.starts_with("HTTP/1.1 403"));
    assert!(!path.exists());
}

#[test]
fn http_sessions_renew_revoke_and_reject_another_origin_or_host_epoch() {
    let mut host = TestHost::new();
    let origin = "http://127.0.0.1:5173";
    let session = host.bootstrap(None, Some(origin));
    let (headers, _) = host.post("/api/ui/snapshot", Some(&session.token), None, json!({}));
    assert!(headers.starts_with("HTTP/1.1 401"));
    let (headers, renewed) = host.post("/api/ui/session", Some(&session.token), Some(origin), json!({}));
    assert!(headers.starts_with("HTTP/1.1 200"));
    assert_eq!(renewed["token"], session.token);
    let (headers, _) = host.post("/api/ui/session/revoke", Some(&session.token), Some(origin), json!({}));
    assert!(headers.starts_with("HTTP/1.1 204"));
    let (headers, _) = host.post("/api/ui/snapshot", Some(&session.token), Some(origin), json!({}));
    assert!(headers.starts_with("HTTP/1.1 401"));
    let session = host.bootstrap(None, None);
    host.state.sessions = Arc::new(SessionRegistry::new(UiSessionCapabilities::ALL));
    let (headers, _) = host.post("/api/ui/snapshot", Some(&session.token), None, json!({}));
    assert!(headers.starts_with("HTTP/1.1 401"));
}

#[test]
fn websocket_readonly_session_rejects_mutation_before_hub_admission() {
    let host = TestHost::new();
    let session = host.bootstrap(
        Some(UiSessionCapabilities {
            observe: true,
            ..Default::default()
        }),
        None,
    );
    let (mut socket, worker) = host.websocket(&session);
    socket
        .send(Message::text(
            json!({ "kind": "intent", "request_id": "denied", "intent": { "kind": "reevaluateGraph" } }).to_string(),
        ))
        .unwrap();
    let error: serde_json::Value = serde_json::from_str(socket.read().unwrap().to_text().unwrap()).unwrap();
    assert_eq!(error["kind"], "error");
    assert_eq!(error["request_id"], "denied");
    assert!(host.commands.try_recv().is_err());
    socket.close(None).unwrap();
    drop(socket);
    worker.join().unwrap();
}

#[test]
fn revocation_closes_an_existing_websocket_with_a_policy_reason() {
    let host = TestHost::new();
    let session = host.bootstrap(None, None);
    let (mut socket, worker) = host.websocket(&session);
    let (headers, _) = host.post("/api/ui/session/revoke", Some(&session.token), None, json!({}));
    assert!(headers.starts_with("HTTP/1.1 204"));
    let Message::Close(Some(close)) = socket.read().unwrap() else {
        panic!("revocation must close the socket");
    };
    assert_eq!(close.code, tungstenite::protocol::frame::coding::CloseCode::Policy);
    drop(socket);
    worker.join().unwrap();
}

#[test]
fn websocket_missing_or_revoked_capability_is_rejected_before_upgrade() {
    let host = TestHost::new();
    let session = host.bootstrap(None, None);
    host.post("/api/ui/session/revoke", Some(&session.token), None, json!({}));
    for token in [None, Some(session.token.as_str())] {
        let worker = host.accept();
        let address = host.listener.local_addr().unwrap();
        let mut request = format!("ws://{address}/api/ui/ws").into_client_request().unwrap();
        if let Some(token) = token {
            request
                .headers_mut()
                .insert("Authorization", format!("Bearer {token}").parse().unwrap());
        }
        let stream = TcpStream::connect(address).unwrap();
        let error = tungstenite::client(request, stream).unwrap_err();
        assert!(
            matches!(error, tungstenite::HandshakeError::Failure(tungstenite::Error::Http(response)) if response.status() == 401)
        );
        worker.join().unwrap();
        assert!(host.commands.try_recv().is_err());
    }
}
