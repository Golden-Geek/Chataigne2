use std::{
    collections::BTreeSet,
    net::{IpAddr, Ipv4Addr},
    sync::{Arc, mpsc},
};

use golden_protocol::{
    ClientId, ControlRequest, ObservationInterest, ObservationMessage, PreviewChange, PreviewDelta, PreviewKey,
    ProtocolValue, ScopeId, ServerMessage, ViewId,
};

use super::*;

fn preview(sequence: u32, scope: &str) -> PreviewDelta {
    PreviewDelta {
        sequence,
        changes: vec![PreviewChange {
            key: PreviewKey {
                scope: ScopeId(scope.into()),
                entity: format!("entity-{sequence}"),
                field: "value".into(),
            },
            value: ProtocolValue::Integer(sequence as i32),
        }],
    }
}

#[test]
fn fifty_slow_clients_remain_bounded_and_do_not_block_a_healthy_client() {
    let started = std::time::Instant::now();
    let metrics = Arc::new(TransportMetrics::default());
    let slow = (0..50)
        .map(|_| Arc::new(ClientOutboundQueue::new(4, 8, Arc::clone(&metrics)).unwrap()))
        .collect::<Vec<_>>();
    let healthy = ClientOutboundQueue::new(4, 8, Arc::clone(&metrics)).unwrap();
    for sequence in 0..1_000 {
        for client in &slow {
            client.enqueue_preview(preview(sequence, &format!("scope-{sequence}")));
        }
        healthy.enqueue_preview(preview(sequence, "visible"));
        assert!(healthy.drain(2).len() <= 2);
    }
    for client in slow {
        assert!(client.queued_len() <= 9);
        let frames = client.drain(8);
        assert!(frames.len() <= 8);
        assert!(frames.iter().any(|frame| matches!(
            frame,
            OutboundFrame::Message(ServerMessage::Observation(ObservationMessage::ResyncRequired { .. }))
        )));
    }
    assert!(metrics.snapshot().preview_drops > 0);
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
}

#[test]
fn reliable_control_messages_backpressure_instead_of_dropping() {
    let metrics = Arc::new(TransportMetrics::default());
    let queue = ClientOutboundQueue::new(1, 1, Arc::clone(&metrics)).unwrap();
    queue
        .enqueue_reliable(ServerMessage::Control(golden_protocol::ControlResponse::Accepted {
            request_id: 1,
        }))
        .unwrap();
    assert_eq!(
        queue
            .enqueue_reliable(ServerMessage::Control(golden_protocol::ControlResponse::Accepted {
                request_id: 2,
            }))
            .unwrap_err(),
        QueueError::ReliableBackpressure
    );
    assert_eq!(metrics.snapshot().reliable_backpressure, 1);
}

#[test]
fn interests_replace_per_client_view_and_clear_on_disconnect() {
    let client = ClientId("client".into());
    let first = ViewId("first".into());
    let second = ViewId("second".into());
    let mut registry = ObservationRegistry::default();
    for view in [&first, &second] {
        registry.replace(ObservationInterest {
            client: client.clone(),
            view: view.clone(),
            scopes: vec![ScopeId("runtime".into())],
        });
    }
    registry.replace(ObservationInterest {
        client: client.clone(),
        view: first.clone(),
        scopes: vec![ScopeId("inspector".into())],
    });
    assert_eq!(
        registry.scopes_for(&client, &first),
        Some(&BTreeSet::from([ScopeId("inspector".into())]))
    );
    assert_eq!(registry.clear_client(&client), 2);
}

#[test]
fn control_handles_use_bounded_channels_without_transport_mutexes() {
    let (sender, receiver) = mpsc::sync_channel(1);
    let handle = EngineControlHandle::new(sender);
    handle.try_send(ControlRequest::Ping { nonce: 7 }).unwrap();
    assert_eq!(receiver.recv().unwrap(), ControlRequest::Ping { nonce: 7 });
}

#[test]
fn authenticated_and_open_lan_bindings_enforce_their_explicit_safeguards() {
    let mut policy = NetworkPolicy {
        access: NetworkAccess::Authenticated,
        bind_address: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        tls_enabled: false,
        authentication_token: None,
        allowed_origins: BTreeSet::new(),
        advertised_hosts: BTreeSet::new(),
        maximum_clients: 64,
        maximum_payload_bytes: 1_048_576,
    };
    assert_eq!(policy.validate().unwrap_err(), NetworkPolicyError::TlsRequired);
    policy.tls_enabled = true;
    policy.authentication_token = Some("a-secure-token-with-at-least-32-bytes".into());
    policy.allowed_origins.insert("https://control.example".into());
    policy.advertised_hosts.insert("control.example".into());
    assert!(policy.validate().is_ok());
    assert!(policy.authorize("https://control.example", Some("a-secure-token-with-at-least-32-bytes")));
    assert!(policy.validate_host("control.example"));
    policy.access = NetworkAccess::OpenLan;
    policy.tls_enabled = false;
    policy.authentication_token = None;
    assert!(policy.validate().is_ok());
    assert!(policy.authorize("https://control.example", None));
    assert_eq!(
        policy.validate_payload(1_048_577).unwrap_err(),
        AdmissionError::PayloadTooLarge {
            bytes: 1_048_577,
            maximum: 1_048_576,
        }
    );
    let limiter = ConnectionLimiter::new(1);
    let permit = limiter.try_acquire().unwrap();
    assert!(matches!(limiter.try_acquire(), Err(AdmissionError::ClientLimit)));
    drop(permit);
    assert_eq!(limiter.active(), 0);
}

#[test]
fn canonical_transport_load_has_zero_intent_timeouts_and_binary_values_stay_latest_wins() {
    let started = std::time::Instant::now();
    let (sender, receiver) = mpsc::sync_channel(2_000);
    let handle = EngineControlHandle::new(sender);
    let metrics = Arc::new(TransportMetrics::default());
    let clients = (0..50)
        .map(|_| ClientOutboundQueue::new(8, 8, Arc::clone(&metrics)).unwrap())
        .collect::<Vec<_>>();
    for sequence in 0..1_000 {
        handle.try_send(ControlRequest::Ping { nonce: sequence }).unwrap();
        for client in &clients {
            client.enqueue_binary_latest(vec![sequence as u8; 32]);
        }
    }
    assert_eq!(receiver.try_iter().count(), 1_000);
    assert!(clients.iter().all(|client| client.queued_len() == 1));
    assert!(clients.iter().all(|client| matches!(
        client.drain(1).as_slice(),
        [OutboundFrame::Binary(frame)] if frame[0] == 231
    )));
    assert_eq!(metrics.snapshot().reliable_backpressure, 0);
    assert!(started.elapsed() < std::time::Duration::from_millis(500));
}
