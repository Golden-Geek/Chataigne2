use super::*;

golden_engine::define_node_enum!(
    enum AccessTestNode {}
);
impl ProjectLifecycle for AccessTestNode {}

fn request(host: &str, origin: Option<&str>) -> HttpRequest {
    let mut headers = HashMap::from([("host".into(), host.into())]);
    if let Some(origin) = origin {
        headers.insert("origin".into(), origin.into());
    }
    HttpRequest {
        method: "POST".into(),
        path: "/api/ui/intent".into(),
        headers,
        body: Vec::new(),
    }
}

#[test]
fn host_policy_admits_loopback_lan_ipv6_and_native_clients_without_dns_inference() {
    let policy = RequestAccessPolicy::new(&[], &[]).unwrap();
    for (local, host, origin) in [
        ("127.0.0.1:7010", "localhost:7010", "http://localhost:7010"),
        ("127.0.0.1:7010", "127.0.0.1:7010", "http://127.0.0.1:7010"),
        ("192.168.20.44:7041", "192.168.20.44:7041", "http://192.168.20.44:7041"),
        ("[::1]:7010", "[::1]:7010", "http://[::1]:7010"),
        ("[2001:db8::1]:7010", "[2001:db8::1]:7010", "http://[2001:db8::1]:7010"),
    ] {
        assert_eq!(
            policy
                .admit(&request(host, Some(origin)), local.parse().unwrap())
                .unwrap()
                .origin,
            Some(origin.into())
        );
        assert!(
            policy
                .admit(&request(host, None), local.parse().unwrap())
                .unwrap()
                .origin
                .is_none()
        );
    }
}

#[test]
fn rebinding_hosts_wrong_ports_and_unapproved_origins_are_rejected() {
    let policy = RequestAccessPolicy::new(&[], &[]).unwrap();
    let local = "127.0.0.1:7010".parse().unwrap();
    for (host, origin) in [
        ("attacker.test:7010", "http://attacker.test:7010"),
        ("127.0.0.1:7011", "http://127.0.0.1:7011"),
        ("127.0.0.1:7010", "http://attacker.test"),
        ("127.0.0.1:7010", "http://localhost:5173"),
        ("127.0.0.1:7010", "null"),
        ("127.0.0.1:7010", "file://localhost"),
        ("user@127.0.0.1:7010", "http://127.0.0.1:7010"),
        ("127.0.0.1:7010/path", "http://127.0.0.1:7010"),
        ("127.0.0.1:7010", "http://user@127.0.0.1:7010"),
        ("127.0.0.1:7010", "http://127.0.0.1:7010/?query=1"),
        ("127.0.0.1:7010,localhost:7010", "http://127.0.0.1:7010"),
    ] {
        assert!(
            policy.admit(&request(host, Some(origin)), local).is_err(),
            "{host} / {origin}"
        );
    }
    assert!(
        policy
            .admit(&request("localhost:7010", None), "192.168.20.44:7010".parse().unwrap())
            .is_err()
    );
}

#[test]
fn configured_origins_and_dns_authorities_are_exact_bounded_opt_ins() {
    let policy = RequestAccessPolicy::new(&["http://127.0.0.1:5173".into()], &["studio.local:7010".into()]).unwrap();
    let local = "127.0.0.1:7010".parse().unwrap();
    assert!(
        policy
            .admit(&request("studio.local:7010", Some("http://studio.local:7010")), local)
            .is_ok()
    );
    assert!(
        policy
            .admit(&request("127.0.0.1:7010", Some("http://127.0.0.1:5173")), local)
            .is_ok()
    );
    assert!(
        policy
            .admit(&request("127.0.0.1:7010", Some("http://localhost:5173")), local)
            .is_err()
    );
    for invalid in [
        "*",
        "null",
        "http://*.test",
        "http:localhost:5173",
        "http://localhost:5173\0",
        "http://user:pass@localhost:5173",
        "http://localhost:5173/path",
    ] {
        assert!(RequestAccessPolicy::new(&[invalid.into()], &[]).is_err(), "{invalid}");
    }
    assert!(RequestAccessPolicy::new(&[], &["*.local:7010".into()]).is_err());
    assert!(RequestAccessPolicy::new(&vec!["http://localhost:5173".into(); 33], &[]).is_err());
    assert!(RequestAccessPolicy::new(&[], &vec!["localhost:7010".into(); 33]).is_err());
    assert!(RequestAccessPolicy::new(&[format!("http://{}.test", "a".repeat(2_048))], &[]).is_err());
}

#[test]
fn browser_fetch_metadata_cannot_bypass_origin_admission() {
    let policy = RequestAccessPolicy::new(&[], &[]).unwrap();
    for site in ["cross-site", "same-site"] {
        let mut request = request("localhost:7010", None);
        request.headers.insert("sec-fetch-site".into(), site.into());
        assert!(policy.admit(&request, "127.0.0.1:7010".parse().unwrap()).is_err());
        request.path = "/index.html".into();
        assert!(
            policy.admit(&request, "127.0.0.1:7010".parse().unwrap()).is_ok(),
            "ordinary link navigation remains usable"
        );
    }
}

fn exchange(build: impl FnOnce(std::net::SocketAddr) -> String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let runtime =
        ProductionRuntime::<AccessTestNode>::new(Engine::new(Folder::new("root").into()), UiProjectFileSpec::default())
            .unwrap();
    let (cmd_tx, commands) = mpsc::sync_channel(64);
    let state = ServerState {
        sessions: Arc::new(SessionRegistry::new(golden_protocol::UiSessionCapabilities::ALL)),
        request_access: Arc::new(RequestAccessPolicy::new(&["http://127.0.0.1:5173".into()], &[]).unwrap()),
        read_model: runtime.read_model(),
        runtime,
        ws_hub: WsHubHandle {
            cmd_tx,
            readiness: Arc::new(UiSessionReadiness::default()),
        },
        snapshot_encoding: SnapshotEncodingService::spawn().unwrap(),
        frontend_assets: &[],
        preferences: None,
    };
    let mut wire = build(address);
    if wire.starts_with("GET /api/ui/ws ") {
        let origin = wire.lines().find_map(|line| line.strip_prefix("Origin: "));
        let admission = super::super::access::RequestAdmission {
            origin: origin.map(str::to_owned),
            authority: address.to_string(),
        };
        let session = state.sessions.bootstrap(&admission, Default::default(), None).unwrap();
        wire = wire.replacen("\r\n", &format!("\r\nAuthorization: Bearer {}\r\n", session.token), 1);
    }
    let server = thread::spawn(move || {
        let _commands = commands;
        let (mut stream, _) = listener.accept().unwrap();
        handle_connection(&mut stream, &state).unwrap();
    });
    let mut client = TcpStream::connect(address).unwrap();
    client.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    client.write_all(wire.as_bytes()).unwrap();
    client.shutdown(Shutdown::Write).unwrap();
    let mut response = String::new();
    client.read_to_string(&mut response).unwrap();
    server.join().unwrap();
    response
}

#[test]
fn http_origin_failures_precede_privileged_body_parsing_and_have_no_cors_grant() {
    for path in ["/api/ui/intent", "/api/ui/project-load-path", "/api/ui/script-config"] {
        let response = exchange(|address| {
            format!(
                "POST {path} HTTP/1.1\r\nHost: {address}\r\nOrigin: http://attacker.test\r\nContent-Length: 8\r\n\r\ninvalid!"
            )
        });
        assert!(response.starts_with("HTTP/1.1 403 Forbidden"), "{response}");
        assert!(!response.contains("Access-Control-Allow-Origin"));
        assert!(response.contains("browser Origin is not admitted"));
    }
}

#[test]
fn http_served_development_and_headless_access_keep_exact_cors_behavior() {
    for origin in [None, Some("same"), Some("http://127.0.0.1:5173")] {
        let response = exchange(|address| {
            let origin = origin.map(|origin| {
                if origin == "same" {
                    format!("http://{address}")
                } else {
                    origin.into()
                }
            });
            format!(
                "GET /api/ui/health HTTP/1.1\r\nHost: {address}\r\n{}\r\n",
                origin.map_or(String::new(), |origin| format!("Origin: {origin}\r\n"))
            )
        });
        assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
        assert!(!response.contains("Access-Control-Allow-Origin: *"));
        assert_eq!(response.contains("Access-Control-Allow-Origin:"), origin.is_some());
        assert!(response.contains("Vary: Origin"));
        if origin == Some("http://127.0.0.1:5173") {
            assert!(response.contains("Access-Control-Allow-Origin: http://127.0.0.1:5173\r\n"));
        }
    }
}

#[test]
fn http_preflight_rejects_unknown_origin_method_or_headers() {
    for (origin, method, headers, expected) in [
        (
            "http://127.0.0.1:5173",
            "POST",
            "Content-Type, X-GC-UI-Client-Instance",
            "204 No Content",
        ),
        ("http://attacker.test", "POST", "Content-Type", "403 Forbidden"),
        ("http://127.0.0.1:5173", "DELETE", "Content-Type", "403 Forbidden"),
        ("http://127.0.0.1:5173", "POST", "X-Unreviewed", "403 Forbidden"),
    ] {
        let response = exchange(|address| {
            format!(
                "OPTIONS /api/ui/intent HTTP/1.1\r\nHost: {address}\r\nOrigin: {origin}\r\nAccess-Control-Request-Method: {method}\r\nAccess-Control-Request-Headers: {headers}\r\n\r\n"
            )
        });
        assert!(response.starts_with(&format!("HTTP/1.1 {expected}")), "{response}");
    }
}

#[test]
fn websocket_origin_admission_is_enforced_before_the_upgrade() {
    for (origin, expected) in [
        (None, "101 Switching Protocols"),
        (Some("same"), "101 Switching Protocols"),
        (Some("http://127.0.0.1:5173"), "101 Switching Protocols"),
        (Some("http://attacker.test"), "403 Forbidden"),
    ] {
        let response = exchange(|address| {
            let origin = origin.map(|origin| {
                if origin == "same" {
                    format!("http://{address}")
                } else {
                    origin.into()
                }
            });
            format!(
                "GET /api/ui/ws HTTP/1.1\r\nHost: {address}\r\n{}Connection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n",
                origin.map_or(String::new(), |origin| format!("Origin: {origin}\r\n"))
            )
        });
        assert!(response.starts_with(&format!("HTTP/1.1 {expected}")), "{response}");
        assert!(!response.contains("Access-Control-Allow-Origin"));
    }
}

#[test]
fn socket_requests_cannot_use_a_rebound_or_ambiguous_host() {
    for headers in [
        "Host: attacker.test:{port}\r\nOrigin: http://attacker.test:{port}\r\n",
        "Host: 127.0.0.1:{port}\r\nHost: attacker.test:{port}\r\n",
    ] {
        let response = exchange(|address| {
            format!(
                "GET /api/ui/health HTTP/1.1\r\n{}\r\n",
                headers.replace("{port}", &address.port().to_string())
            )
        });
        assert!(
            response.starts_with("HTTP/1.1 403 Forbidden") || response.starts_with("HTTP/1.1 400 Bad Request"),
            "{response}"
        );
        assert!(!response.contains("Access-Control-Allow-Origin"));
    }
}
