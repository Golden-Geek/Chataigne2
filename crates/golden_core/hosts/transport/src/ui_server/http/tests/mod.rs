use super::{request::parse_head, *};

#[test]
fn parser_preserves_valid_request_targets_and_combines_list_headers() {
    let bytes = b"POST /api/ui/snapshot?view=whole HTTP/1.1\r\nHost: localhost:7010\r\nConnection: keep-alive\r\nConnection: Upgrade\r\nContent-Length: 2\r\n\r\n";
    let (request, offset, length) = parse_head(bytes).unwrap();
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/api/ui/snapshot");
    assert_eq!(request.headers["host"], "localhost:7010");
    assert!(header_contains_token(&request, "connection", "upgrade"));
    assert_eq!(offset, bytes.len());
    assert_eq!(length, 2);
}

#[test]
fn parser_rejects_duplicate_authority_origin_credentials_and_framing() {
    for name in [
        "Host",
        "Origin",
        "Content-Length",
        "Authorization",
        "Sec-WebSocket-Key",
        "Sec-WebSocket-Version",
    ] {
        let wire = format!(
            "GET /api/ui/health HTTP/1.1\r\n{name}: 1\r\n{}: 1\r\n\r\n",
            name.to_lowercase()
        );
        assert!(parse_head(wire.as_bytes()).is_err(), "duplicate {name}");
    }
}

#[test]
fn parser_rejects_ambiguous_or_unsupported_http_framing_and_targets() {
    for wire in [
        "GET / HTTP/1.1\r\nHost : localhost\r\n\r\n",
        "GET / HTTP/1.1\r\nTransfer-Encoding: chunked\r\nContent-Length: 0\r\n\r\n",
        "GET / HTTP/1.1\r\nContent-Length: +1\r\n\r\n",
        "GET / HTTP/1.1\r\nContent-Length: 1,1\r\n\r\n",
        "GET / HTTP/1.1\r\nContent-Length: 184467440737095516160\r\n\r\n",
        "GET http://localhost:7010/api/ui/health HTTP/1.1\r\n\r\n",
        "GET //other.test/api/ui/health HTTP/1.1\r\n\r\n",
        "GET /api/ui/health#fragment HTTP/1.1\r\n\r\n",
        "GET / HTTP/1.0\r\n\r\n",
        "GET / HTTP/1.1 trailing\r\n\r\n",
        "GET / HTTP/1.1\r\nHost: local\r\n folded: header\r\n\r\n",
    ] {
        assert!(parse_head(wire.as_bytes()).is_err(), "unexpectedly accepted {wire:?}");
    }
}

#[test]
fn parser_enforces_header_count_header_bytes_and_declared_body_limits() {
    let mut wire = "GET / HTTP/1.1\r\n".to_string();
    for index in 0..129 {
        wire.push_str(&format!("X-{index}: value\r\n"));
    }
    wire.push_str("\r\n");
    assert!(parse_head(wire.as_bytes()).is_err());
    let header = format!("GET / HTTP/1.1\r\nHost: {}\r\n\r\n", "a".repeat(HTTP_MAX_HEADER_BYTES));
    assert!(parse_head(header.as_bytes()).is_err());
    let body = format!("POST /api/ui/intent HTTP/1.1\r\nContent-Length: {HTTP_MAX_REQUEST_BYTES}\r\n\r\n");
    assert!(parse_head(body.as_bytes()).is_err());
}

#[test]
fn socket_reader_preserves_body_and_rejects_a_truncated_declared_body() {
    for truncated in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let body = if truncated { "{" } else { "{}" };
        write!(
            client,
            "POST /api/ui/snapshot HTTP/1.1\r\nHost: localhost:7010\r\nContent-Length: 2\r\n\r\n{body}"
        )
        .unwrap();
        client.shutdown(Shutdown::Write).unwrap();
        let (mut server, _) = listener.accept().unwrap();
        server.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        let parsed = read_http_request(&mut server);
        if truncated {
            assert_eq!(parsed.err().unwrap().kind(), ErrorKind::UnexpectedEof);
        } else {
            assert_eq!(parsed.unwrap().body, b"{}");
        }
    }
}
