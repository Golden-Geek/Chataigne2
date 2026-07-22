use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use golden_engine::node::Folder;
use serde_json::json;

use super::*;

#[test]
fn default_ui_server_config_uses_explicit_ipv4_loopback() {
    assert_eq!(UiServerConfig::default().bind_addr, "127.0.0.1:7010");
}

#[test]
fn readiness_counts_only_clients_with_active_subscriptions() {
    let readiness = UiSessionReadiness::default();
    let mut clients = HashMap::new();
    clients.insert(1, client_with_subscription_count(0));
    clients.insert(2, client_with_subscription_count(2));

    readiness.update(&clients);

    assert_eq!(readiness.active_websocket_clients.load(Ordering::Acquire), 2);
    assert_eq!(readiness.active_subscribed_websocket_clients.load(Ordering::Acquire), 1);

    clients.get_mut(&2).unwrap().subscriptions.clear();
    readiness.update(&clients);
    assert_eq!(readiness.active_subscribed_websocket_clients.load(Ordering::Acquire), 0);
}

#[test]
fn readiness_dto_has_a_stable_versioned_http_shape() {
    let dto = UiReadinessDto {
        version: 1,
        backend_ready: true,
        engine_read_model_ready: true,
        active_websocket_clients: 3,
        active_subscribed_websocket_clients: 2,
        read_model_revision: EngineTime {
            tick: 7,
            micro: 2,
            seq: 11,
        },
    };

    assert_eq!(
        serde_json::to_value(dto).unwrap(),
        json!({
            "version": 1,
            "backend_ready": true,
            "engine_read_model_ready": true,
            "active_websocket_clients": 3,
            "active_subscribed_websocket_clients": 2,
            "read_model_revision": {
                "tick": 7,
                "micro": 2,
                "seq": 11,
            },
        })
    );
}

#[test]
fn discovery_document_uses_relative_open_lan_endpoints() {
    assert_eq!(
        serde_json::to_value(ui_discovery_document()).unwrap(),
        json!({
            "version": 1,
            "service": "chataigne",
            "health_path": "/api/ui/health",
            "websocket_path": "/ws",
            "relative_endpoints": true,
        })
    );
}

fn client_with_subscription_count(count: usize) -> WsClientState {
    let outbound = Arc::new(WsOutboundQueue::new(DEFAULT_OUTBOUND_CAPACITY));
    let subscriptions = (0..count)
        .map(|index| {
            (
                format!("subscription-{index}"),
                WsSubscriptionState {
                    interest: UiInterest::workbench(format!("view-{index}"), UiSubscriptionScope::WholeGraph),
                    cursor: None,
                    last_runtime_stats: None,
                    pending_value_from: None,
                    pending_value_to: None,
                    pending_value_events: Vec::new(),
                },
            )
        })
        .collect();
    WsClientState {
        outbound,
        subscriptions,
        client_instance_id: None,
    }
}

#[test]
fn outbound_queue_supersedes_latest_wins_plane_for_the_same_view() {
    let queue = WsOutboundQueue::new(2);
    assert_eq!(queue.push(observation_message(1)), QueuePushResult::Queued);
    assert_eq!(queue.push(observation_message(2)), QueuePushResult::Superseded);
    assert_eq!(queue.len(), 1);

    let Some(WsOutbound::Message(WsServerMessage::Delta { delta, .. })) = queue.pop() else {
        panic!("expected observation delta");
    };
    assert_eq!(delta.batch.to.unwrap().tick, 2);
}

#[test]
fn outbound_queue_never_silently_drops_reliable_messages() {
    let queue = WsOutboundQueue::new(2);
    assert_eq!(queue.push(reliable_message(1)), QueuePushResult::Queued);
    assert_eq!(queue.push(reliable_message(2)), QueuePushResult::Queued);
    assert_eq!(queue.push(reliable_message(3)), QueuePushResult::Full);
    assert_eq!(queue.len(), 2);
}

#[test]
fn slow_client_is_removed_when_only_reliable_messages_fill_its_queue() {
    let mut clients = HashMap::new();
    clients.insert(7, client_with_subscription_count(1));
    let outbound = clients.get(&7).unwrap().outbound.clone();
    for client_id in 0..DEFAULT_OUTBOUND_CAPACITY as u64 {
        assert_eq!(outbound.push(reliable_message(client_id)), QueuePushResult::Queued);
    }

    send_to_client(
        &mut clients,
        7,
        WsServerMessage::Hello {
            protocol_version: UI_PROTOCOL_VERSION.to_string(),
            client_id: 99,
            session_id: "overflow".to_string(),
        },
    );

    assert!(!clients.contains_key(&7));
}

#[test]
fn canonical_subscribe_message_serializes_view_scope_and_planes() {
    let message = WsClientMessage::Subscribe {
        subscription_id: "sub-1".to_string(),
        interest: UiInterest {
            view_id: "state-machine".to_string(),
            scope: UiSubscriptionScope::Subtree {
                root: NodeId(42),
                max_depth: 3,
            },
            planes: vec![UiDataPlane::Structure, UiDataPlane::Preview],
        },
        from: None,
    };

    assert_eq!(
        serde_json::to_value(message).unwrap(),
        json!({
            "kind": "subscribe",
            "subscription_id": "sub-1",
            "interest": {
                "view_id": "state-machine",
                "scope": { "subtree": { "root": 42, "max_depth": 3 } },
                "planes": ["structure", "preview"]
            }
        })
    );
}

#[test]
fn cursor_resync_advances_past_the_replacement_marker() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    engine.clear_ui_event_log();
    engine.push_ui_custom_event(
        "__transport.resync_required",
        None,
        json!({ "reason": "project_replaced" }),
    );
    let read_model = Arc::new(UiReadModel::from_engine(&engine, UiProjectFileSpec::default()));
    read_model.publish_engine_events_since(&engine, None);
    let replacement_marker_time = read_model.current_event_time().expect("replacement marker time");
    let server_time = replacement_marker_time.max(read_model.current_snapshot().at);

    let mut clients = HashMap::new();
    clients.insert(1, client_with_subscription_count(1));
    let subscription = clients
        .get_mut(&1)
        .unwrap()
        .subscriptions
        .get_mut("subscription-0")
        .unwrap();
    subscription.cursor = Some(EngineTime {
        tick: server_time.tick + 1,
        micro: server_time.micro,
        seq: server_time.seq,
    });
    let outbound = clients.get(&1).unwrap().outbound.clone();
    let mut origins = HashMap::new();

    dispatch_ws_batches(&read_model, &mut clients, &mut origins, false);
    dispatch_ws_batches(&read_model, &mut clients, &mut origins, false);

    assert_eq!(
        clients
            .get(&1)
            .unwrap()
            .subscriptions
            .get("subscription-0")
            .unwrap()
            .cursor,
        Some(server_time)
    );
    let Some(WsOutbound::Message(WsServerMessage::ResyncRequired { reason, .. })) = outbound.pop() else {
        panic!("expected one resync request");
    };
    assert_eq!(reason, "cursor_ahead_of_server_time");
    assert!(
        outbound.pop().is_none(),
        "replacement marker must not request a second resync"
    );
}

fn observation_message(tick: u64) -> WsOutbound {
    WsOutbound::Message(WsServerMessage::Delta {
        subscription_id: "workbench".to_string(),
        delta: UiPlaneDelta {
            plane: UiDataPlane::Observation,
            batch: UiEventBatch {
                from: None,
                to: Some(EngineTime { tick, micro: 0, seq: 0 }),
                runtime: None,
                events: Vec::new(),
            },
        },
    })
}

fn reliable_message(client_id: u64) -> WsOutbound {
    WsOutbound::Message(WsServerMessage::Hello {
        protocol_version: UI_PROTOCOL_VERSION.to_string(),
        client_id,
        session_id: "test-session".to_string(),
    })
}
