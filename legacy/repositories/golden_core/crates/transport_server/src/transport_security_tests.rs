use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;

use super::{
    BrowserRequestRejection, TransportMetrics, UiTransportSecurityConfig, validate_browser_request, validate_json_shape,
};

fn local_addr() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 7010)
}

fn headers(host: &str, origin: Option<&str>) -> HashMap<String, String> {
    let mut headers = HashMap::from([("host".to_string(), host.to_string())]);
    if let Some(origin) = origin {
        headers.insert("origin".to_string(), origin.to_string());
    }
    headers
}

#[test]
fn native_request_without_origin_remains_allowed() {
    assert_eq!(
        validate_browser_request(
            &headers("127.0.0.1:7010", None),
            local_addr(),
            &UiTransportSecurityConfig::default(),
        ),
        Ok(None)
    );
}

#[test]
fn same_origin_and_explicit_origin_are_allowed() {
    let mut security = UiTransportSecurityConfig::default();
    security.allowed_origins.push("https://controller.example".to_string());
    assert_eq!(
        validate_browser_request(
            &headers("127.0.0.1:7010", Some("http://127.0.0.1:7010")),
            local_addr(),
            &security,
        ),
        Ok(Some("http://127.0.0.1:7010".to_string()))
    );
    assert!(
        validate_browser_request(
            &headers("127.0.0.1:7010", Some("https://controller.example")),
            local_addr(),
            &security,
        )
        .is_ok()
    );
}

#[test]
fn configured_dev_frontend_can_reach_localhost_runtime() {
    let mut security = UiTransportSecurityConfig::default();
    security.allowed_origins.push("http://127.0.0.1:5173".to_string());

    assert_eq!(
        validate_browser_request(
            &headers("localhost:7010", Some("http://127.0.0.1:5173")),
            local_addr(),
            &security,
        ),
        Ok(Some("http://127.0.0.1:5173".to_string()))
    );
}

#[test]
fn foreign_origin_and_rebound_host_are_rejected() {
    assert!(matches!(
        validate_browser_request(
            &headers("127.0.0.1:7010", Some("https://evil.example")),
            local_addr(),
            &UiTransportSecurityConfig::default(),
        ),
        Err(BrowserRequestRejection::Origin(_))
    ));
    assert!(matches!(
        validate_browser_request(
            &headers("rebound.example:7010", None),
            local_addr(),
            &UiTransportSecurityConfig::default(),
        ),
        Err(BrowserRequestRejection::Host(_))
    ));
}

#[test]
fn connection_permits_enforce_and_release_the_limit() {
    let metrics = Arc::new(TransportMetrics::default());
    let first = metrics.try_acquire_connection(1).expect("first permit should fit");
    assert!(metrics.try_acquire_connection(1).is_none());
    assert_eq!(metrics.snapshot().rejected_connections, 1);
    drop(first);
    assert!(metrics.try_acquire_connection(1).is_some());
}

#[test]
fn json_shape_limits_reject_depth_strings_and_unbalanced_payloads() {
    assert!(validate_json_shape(br#"{"ok":[1,2,3]}"#, 4, 16).is_ok());
    assert_eq!(
        validate_json_shape(br#"[[[[]]]]"#, 3, 16),
        Err("JSON nesting exceeds the configured depth limit")
    );
    assert_eq!(
        validate_json_shape(br#"{"value":"too long"}"#, 4, 4),
        Err("JSON string exceeds the configured size limit")
    );
    assert_eq!(
        validate_json_shape(br#"{"value":[]"#, 4, 16),
        Err("JSON payload is incomplete")
    );
}
