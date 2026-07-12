use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::mpsc;

use serde_json::json;

use super::*;

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

fn client_with_subscription_count(count: usize) -> WsClientState {
    let (outbound, _receiver) = mpsc::channel();
    let subscriptions = (0..count)
        .map(|index| {
            (
                format!("subscription-{index}"),
                WsSubscriptionState {
                    scope: UiSubscriptionScope::WholeGraph,
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
