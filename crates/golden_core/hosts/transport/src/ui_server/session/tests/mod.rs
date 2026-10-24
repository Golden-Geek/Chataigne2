use super::*;

pub(in crate::ui_server) fn authoring_grant() -> SessionGrant {
    SessionGrant {
        epoch: Instant::now(),
        expires_at_ms: AtomicU64::new(SESSION_LIFETIME_MS),
        active: AtomicBool::new(true),
        capabilities: UiSessionCapabilities::ALL,
    }
}

fn binding() -> RequestAdmission {
    RequestAdmission {
        authority: "localhost:7010".into(),
        origin: Some("http://localhost:7010".into()),
    }
}
fn request(token: &str) -> HttpRequest {
    HttpRequest {
        method: "GET".into(),
        path: "/api/ui/ws".into(),
        headers: HashMap::from([("authorization".into(), format!("Bearer {token}"))]),
        body: Vec::new(),
    }
}

#[test]
fn credentials_are_bound_scoped_and_redacted() {
    let registry = SessionRegistry::new(UiSessionCapabilities::ALL);
    let response = registry
        .bootstrap(
            &binding(),
            UiSessionRequest {
                capabilities: Some(UiSessionCapabilities {
                    observe: true,
                    ..Default::default()
                }),
            },
            None,
        )
        .unwrap();
    assert_eq!(response.token.len(), 43);
    assert!(!format!("{response:?}").contains(&response.token));
    assert!(!format!("{registry:?}").contains(&response.token));
    let grant = registry.authenticate(&binding(), &request(&response.token)).unwrap();
    assert!(grant.require(Capability::Observe).is_ok());
    for privilege in [Capability::Author, Capability::ProjectFiles, Capability::Scripts] {
        assert_eq!(grant.require(privilege).unwrap_err().status, "403 Forbidden");
    }
    let mut other = binding();
    other.origin = None;
    assert!(registry.authenticate(&other, &request(&response.token)).is_err());
    other = binding();
    other.authority = "127.0.0.1:7010".into();
    assert!(registry.authenticate(&other, &request(&response.token)).is_err());
}

#[test]
fn bounded_sessions_renew_in_place_and_recover_after_expiry() {
    let registry = SessionRegistry::new(UiSessionCapabilities::ALL);
    let first = registry.bootstrap_at(&binding(), Default::default(), None, 0).unwrap();
    let grant = registry.authenticate(&binding(), &request(&first.token)).unwrap();
    for _ in 1..MAX_SESSIONS {
        registry.bootstrap_at(&binding(), Default::default(), None, 0).unwrap();
    }
    assert_eq!(
        registry
            .bootstrap_at(&binding(), Default::default(), None, 0)
            .unwrap_err()
            .status,
        "503 Service Unavailable"
    );
    let renewed = registry
        .bootstrap_at(&binding(), Default::default(), Some(&first.token), 100)
        .unwrap();
    assert_eq!(renewed.token, first.token);
    assert_eq!(registry.records.lock().unwrap().len(), MAX_SESSIONS);
    assert!(grant.check_at(SESSION_LIFETIME_MS).is_ok());
    assert!(grant.check_at(SESSION_LIFETIME_MS + 100).is_err());
    registry
        .bootstrap_at(&binding(), Default::default(), None, SESSION_LIFETIME_MS + 100)
        .unwrap();
    assert_eq!(registry.records.lock().unwrap().len(), 1);
    assert!(!grant.active.load(Ordering::Acquire));
}

#[test]
fn revocation_registry_teardown_and_new_host_invalidate_credentials() {
    let registry = SessionRegistry::new(UiSessionCapabilities::ALL);
    let response = registry.bootstrap(&binding(), Default::default(), None).unwrap();
    let grant = registry.authenticate(&binding(), &request(&response.token)).unwrap();
    registry.revoke(&request(&response.token)).unwrap();
    assert!(grant.check().is_err());
    assert!(registry.authenticate(&binding(), &request(&response.token)).is_err());
    let second = registry.bootstrap(&binding(), Default::default(), None).unwrap();
    let active = registry.authenticate(&binding(), &request(&second.token)).unwrap();
    drop(registry);
    assert!(active.check().is_err());
    assert!(
        SessionRegistry::new(UiSessionCapabilities::ALL)
            .authenticate(&binding(), &request(&second.token))
            .is_err()
    );
}

#[test]
fn host_grant_cannot_be_escalated_by_bootstrap_or_renewal() {
    let readonly = UiSessionCapabilities {
        observe: true,
        ..Default::default()
    };
    let registry = SessionRegistry::new(readonly);
    assert!(
        registry
            .bootstrap(
                &binding(),
                UiSessionRequest {
                    capabilities: Some(UiSessionCapabilities::ALL)
                },
                None
            )
            .is_err()
    );
    let response = registry.bootstrap(&binding(), Default::default(), None).unwrap();
    assert_eq!(response.capabilities, readonly);
    assert!(
        registry
            .bootstrap(
                &binding(),
                UiSessionRequest {
                    capabilities: Some(UiSessionCapabilities::ALL)
                },
                Some(&response.token)
            )
            .is_err()
    );
}

#[test]
fn websocket_credentials_accept_browser_subprotocols_and_reject_ambiguity() {
    let registry = SessionRegistry::new(UiSessionCapabilities::ALL);
    let response = registry.bootstrap(&binding(), Default::default(), None).unwrap();
    let mut request = request(&response.token);
    request.headers.remove("authorization");
    request.headers.insert(
        "sec-websocket-protocol".into(),
        format!("{SESSION_PROTOCOL}, {TOKEN_PROTOCOL_PREFIX}{}", response.token),
    );
    assert!(registry.authenticate(&binding(), &request).is_ok());
    request
        .headers
        .insert("authorization".into(), format!("Bearer {}", "a".repeat(43)));
    assert!(request_token(&request).is_err());
    request.headers.remove("authorization");
    request
        .headers
        .get_mut("sec-websocket-protocol")
        .unwrap()
        .push_str(&format!(", {TOKEN_PROTOCOL_PREFIX}{}", response.token));
    assert!(request_token(&request).is_err());
    request.path = "/api/ui/intent".into();
    assert!(request_token(&request).unwrap().is_none());
}

#[test]
fn underprivileged_websocket_operations_never_reach_the_hub() {
    use crate::ui_server::{
        UiEditIntent, UiSessionReadiness, WsClientMessage, WsHubHandle, WsOutbound, WsOutboundQueue, WsServerMessage,
        handle_ws_client_message,
    };
    let registry = SessionRegistry::new(UiSessionCapabilities {
        observe: true,
        ..Default::default()
    });
    let response = registry.bootstrap(&binding(), Default::default(), None).unwrap();
    let grant = registry.authenticate(&binding(), &request(&response.token)).unwrap();
    let (cmd_tx, receiver) = std::sync::mpsc::sync_channel(1);
    let hub = WsHubHandle {
        cmd_tx,
        readiness: Default::default(),
    };
    let outbound = Arc::new(WsOutboundQueue::new(4));
    let _readiness: Arc<UiSessionReadiness> = hub.readiness.clone();
    let message = WsClientMessage::Intent {
        request_id: "denied".into(),
        intent: Box::new(UiEditIntent::ReevaluateGraph),
        include_self_events: true,
    };
    assert!(handle_ws_client_message(message, 1, &hub, &outbound, &grant));
    assert!(receiver.try_recv().is_err());
    assert!(
        matches!(outbound.pop(), Some(WsOutbound::Message(WsServerMessage::Error { request_id: Some(id), .. })) if id == "denied")
    );
}
