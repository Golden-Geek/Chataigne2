use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use golden_engine::node::{Folder, NodeId};
use golden_engine::parameter::ParamValue;
use serde_json::json;

use super::runtime_pacer::DeadlineSchedule;
use super::*;

#[test]
fn runtime_pacer_keeps_an_absolute_deadline_across_ordinary_wake_lateness() {
    let origin = Instant::now();
    let interval = Duration::from_millis(5);
    let mut schedule = DeadlineSchedule::default();

    let first = schedule.next_deadline(origin, origin + Duration::from_millis(1), interval);
    let second_tick_started = origin + Duration::from_millis(9);
    let second = schedule.next_deadline(
        second_tick_started,
        second_tick_started + Duration::from_micros(100),
        interval,
    );

    assert_eq!(first, origin + Duration::from_millis(5));
    assert_eq!(second, origin + Duration::from_millis(10));
}

#[test]
fn runtime_pacer_allows_one_bounded_catch_up_tick() {
    let origin = Instant::now();
    let interval = Duration::from_millis(5);
    let mut schedule = DeadlineSchedule::default();

    schedule.next_deadline(origin, origin, interval);
    let overdue = schedule.next_deadline(
        origin + Duration::from_millis(9),
        origin + Duration::from_millis(11),
        interval,
    );
    let recovered = schedule.next_deadline(
        origin + Duration::from_millis(11),
        origin + Duration::from_millis(11),
        interval,
    );

    assert_eq!(overdue, origin + Duration::from_millis(10));
    assert_eq!(recovered, origin + Duration::from_millis(15));
}

#[test]
fn runtime_pacer_drops_a_long_stale_backlog() {
    let origin = Instant::now();
    let interval = Duration::from_millis(5);
    let mut schedule = DeadlineSchedule::default();

    schedule.next_deadline(origin, origin, interval);
    let resumed_at = origin + Duration::from_secs(1);
    let resumed = schedule.next_deadline(resumed_at, resumed_at, interval);
    let following = schedule.next_deadline(resumed_at, resumed_at, interval);

    assert_eq!(resumed, resumed_at);
    assert_eq!(following, resumed_at + interval);
}

#[test]
fn runtime_pacer_reanchors_when_the_requested_frequency_changes() {
    let origin = Instant::now();
    let mut schedule = DeadlineSchedule::default();

    schedule.next_deadline(origin, origin, Duration::from_millis(5));
    let changed_tick = origin + Duration::from_millis(5);
    let changed = schedule.next_deadline(
        changed_tick,
        changed_tick + Duration::from_millis(1),
        Duration::from_millis(10),
    );

    assert_eq!(changed, origin + Duration::from_millis(15));
}

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
            "websocket_path": "/api/ui/ws",
            "relative_endpoints": true,
        })
    );
}

#[test]
fn bundled_frontend_fallback_does_not_shadow_backend_namespaces() {
    const ASSETS: [UiAsset; 1] = [UiAsset {
        path: "/index.html",
        content_type: "text/html",
        bytes: b"frontend",
    }];

    assert!(resolve_frontend_asset(&ASSETS, "/api/ui/health").is_none());
    assert!(resolve_frontend_asset(&ASSETS, "/.well-known/chataigne").is_none());
    assert_eq!(
        resolve_frontend_asset(&ASSETS, "/evidence/sound-card").map(|asset| asset.path),
        Some("/index.html")
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
                    pending_value_events: PendingValueEvents::default(),
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

    let Some(WsOutbound::Message(WsServerMessage::Delta { deltas, .. })) = queue.pop() else {
        panic!("expected observation delta");
    };
    assert_eq!(deltas.len(), 1);
    assert_eq!(deltas[0].batch.to.unwrap().tick, 2);
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
fn outbound_queue_treats_multi_plane_delta_envelopes_as_reliable() {
    let queue = WsOutboundQueue::new(1);
    assert_eq!(queue.push(multi_plane_message(1)), QueuePushResult::Queued);
    assert_eq!(queue.push(observation_message(2)), QueuePushResult::Full);
    assert_eq!(queue.len(), 1);
}

#[test]
fn outbound_queue_never_merges_latest_wins_events_across_a_reliable_barrier() {
    let queue = WsOutboundQueue::new(3);
    assert_eq!(queue.push(observation_message(1)), QueuePushResult::Queued);
    assert_eq!(queue.push(multi_plane_message(2)), QueuePushResult::Queued);
    assert_eq!(queue.push(observation_message(3)), QueuePushResult::Queued);
    assert_eq!(queue.len(), 3);

    let Some(WsOutbound::Message(WsServerMessage::Delta { deltas, .. })) = queue.pop() else {
        panic!("expected first observation envelope");
    };
    assert_eq!(deltas[0].batch.to.map(|time| time.tick), Some(1));
    let Some(WsOutbound::Message(WsServerMessage::Delta { deltas, .. })) = queue.pop() else {
        panic!("expected reliable multi-plane barrier");
    };
    assert_eq!(deltas.len(), 2);
    assert_eq!(deltas[0].batch.to.map(|time| time.tick), Some(2));
    let Some(WsOutbound::Message(WsServerMessage::Delta { deltas, .. })) = queue.pop() else {
        panic!("expected final observation envelope");
    };
    assert_eq!(deltas[0].batch.to.map(|time| time.tick), Some(3));
}

#[test]
fn outbound_queue_treats_resync_as_a_subscription_ordering_barrier() {
    let queue = WsOutboundQueue::new(3);
    assert_eq!(queue.push(observation_message(1)), QueuePushResult::Queued);
    assert_eq!(
        queue.push(WsOutbound::Message(WsServerMessage::ResyncRequired {
            subscription_id: "workbench".to_string(),
            plane: None,
            reason: "test_barrier".to_string(),
        })),
        QueuePushResult::Queued
    );
    assert_eq!(queue.push(observation_message(3)), QueuePushResult::Queued);

    let Some(WsOutbound::Message(WsServerMessage::Delta { deltas, .. })) = queue.pop() else {
        panic!("expected first observation envelope");
    };
    assert_eq!(deltas[0].batch.to.map(|time| time.tick), Some(1));
    let Some(WsOutbound::Message(WsServerMessage::ResyncRequired { reason, .. })) = queue.pop() else {
        panic!("expected resync barrier");
    };
    assert_eq!(reason, "test_barrier");
    let Some(WsOutbound::Message(WsServerMessage::Delta { deltas, .. })) = queue.pop() else {
        panic!("expected final observation envelope");
    };
    assert_eq!(deltas[0].batch.to.map(|time| time.tick), Some(3));
}

#[test]
fn pending_value_events_coalesce_large_stream_with_amortized_linear_work() {
    const PARAM_COUNT: usize = 8_192;
    const REPLACEMENT_ROUNDS: usize = 8;

    let from = EngineTime {
        tick: 0,
        micro: 0,
        seq: 0,
    };
    let mut pending = PendingValueEvents::default();
    pending.queue(
        Some(from),
        (0..PARAM_COUNT)
            .map(|param| param_value_event(param, param + 1, 0))
            .collect(),
    );

    for round in 1..=REPLACEMENT_ROUNDS {
        let tick_base = round * PARAM_COUNT;
        pending.queue(
            Some(EngineTime {
                tick: tick_base as u64,
                micro: 0,
                seq: 0,
            }),
            (0..PARAM_COUNT)
                .rev()
                .enumerate()
                .map(|(offset, param)| param_value_event(param, tick_base + offset + 1, round as i32))
                .collect(),
        );
        assert!(
            pending.storage_len() <= PARAM_COUNT * 2,
            "stale slots must be compacted instead of growing with update count"
        );
    }

    let queued_event_count = PARAM_COUNT * (REPLACEMENT_ROUNDS + 1);
    assert!(
        pending.operation_count() <= queued_event_count * 3,
        "indexed replacement and bounded compaction must remain amortized linear"
    );

    let batch = pending.take_batch().expect("latest parameter values");
    assert_eq!(batch.from, Some(from));
    assert_eq!(
        batch.to,
        Some(EngineTime {
            tick: queued_event_count as u64,
            micro: 0,
            seq: 0,
        })
    );
    assert_eq!(batch.events.len(), PARAM_COUNT);
    for (index, event) in batch.events.into_iter().enumerate() {
        let UiEventKind::ParamChanged { param, new_value, .. } = event.kind else {
            panic!("expected parameter event");
        };
        assert_eq!(param, NodeId((PARAM_COUNT - index - 1) as u64));
        assert_eq!(new_value, ParamValue::Int(REPLACEMENT_ROUNDS as i32));
    }
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
fn websocket_intent_queues_received_control_before_hub_handoff() {
    let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();
    let hub = WsHubHandle {
        cmd_tx,
        readiness: Arc::new(UiSessionReadiness::default()),
    };
    let outbound = Arc::new(WsOutboundQueue::new(DEFAULT_OUTBOUND_CAPACITY));

    assert!(handle_ws_client_message(
        WsClientMessage::Intent {
            request_id: "intent-request".to_string(),
            intent: Box::new(UiEditIntent::Undo),
            include_self_events: true,
        },
        7,
        &hub,
        &outbound,
    ));

    assert_received_control(&outbound, "intent-request");
    let command = cmd_rx.try_recv().expect("intent handed to websocket hub");
    let WsHubCommand::Intent {
        client_id,
        request_id,
        intent,
        include_self_events,
    } = command
    else {
        panic!("expected intent command");
    };
    assert_eq!(client_id, 7);
    assert_eq!(request_id, "intent-request");
    assert_eq!(*intent, UiEditIntent::Undo);
    assert!(include_self_events);
}

#[test]
fn websocket_intent_batch_queues_received_control_before_hub_handoff() {
    let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();
    let hub = WsHubHandle {
        cmd_tx,
        readiness: Arc::new(UiSessionReadiness::default()),
    };
    let outbound = Arc::new(WsOutboundQueue::new(DEFAULT_OUTBOUND_CAPACITY));

    assert!(handle_ws_client_message(
        WsClientMessage::IntentBatch {
            request_id: "batch-request".to_string(),
            intents: vec![UiEditIntent::Undo, UiEditIntent::Redo],
            include_self_events: false,
        },
        11,
        &hub,
        &outbound,
    ));

    assert_received_control(&outbound, "batch-request");
    let command = cmd_rx.try_recv().expect("intent batch handed to websocket hub");
    let WsHubCommand::IntentBatch {
        client_id,
        request_id,
        intents,
        include_self_events,
    } = command
    else {
        panic!("expected intent batch command");
    };
    assert_eq!(client_id, 11);
    assert_eq!(request_id, "batch-request");
    assert_eq!(intents, vec![UiEditIntent::Undo, UiEditIntent::Redo]);
    assert!(!include_self_events);
}

fn assert_received_control(outbound: &WsOutboundQueue, expected_request_id: &str) {
    let Some(WsOutbound::Message(WsServerMessage::Control { update })) = outbound.pop() else {
        panic!("expected received control update before hub command");
    };
    assert_eq!(update.request_id, expected_request_id);
    assert_eq!(update.phase, UiControlPhase::Received);
    assert!(
        update.acknowledgement.is_none(),
        "received phase must not acknowledge a single intent"
    );
    assert!(
        update.acknowledgements.is_empty(),
        "received phase must not acknowledge a batch"
    );
    assert!(
        outbound.pop().is_none(),
        "message parsing must not emit a final acknowledgement"
    );
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

#[test]
fn custom_event_data_plane_uses_explicit_retention_not_topic_text() {
    let time = EngineTime {
        tick: 1,
        micro: 0,
        seq: 0,
    };
    let replay_event = UiEventDto {
        time,
        kind: UiEventKind::Custom {
            topic: "test.named_preview_but_reliable".to_string(),
            origin: None,
            payload: serde_json::Value::Null,
            retention: CustomEventRetention::Replay,
        },
    };
    let latest_event = UiEventDto {
        time,
        kind: UiEventKind::Custom {
            topic: "test.runtime_frame".to_string(),
            origin: None,
            payload: serde_json::Value::Null,
            retention: CustomEventRetention::Latest,
        },
    };

    assert_eq!(ui_data_plane(&replay_event), UiDataPlane::Trigger);
    assert_eq!(ui_data_plane(&latest_event), UiDataPlane::Preview);
}

fn observation_message(tick: u64) -> WsOutbound {
    WsOutbound::Message(WsServerMessage::Delta {
        subscription_id: "workbench".to_string(),
        deltas: vec![UiPlaneDelta {
            plane: UiDataPlane::Observation,
            batch: UiEventBatch {
                from: None,
                to: Some(EngineTime { tick, micro: 0, seq: 0 }),
                runtime: None,
                events: Vec::new(),
            },
        }],
    })
}

fn param_value_event(param: usize, tick: usize, new_value: i32) -> UiEventDto {
    UiEventDto {
        time: EngineTime {
            tick: tick as u64,
            micro: 0,
            seq: 0,
        },
        kind: UiEventKind::ParamChanged {
            param: NodeId(param as u64),
            old_value: ParamValue::Int(new_value.saturating_sub(1)),
            new_value: ParamValue::Int(new_value),
        },
    }
}

fn multi_plane_message(tick: u64) -> WsOutbound {
    let batch = UiEventBatch {
        from: None,
        to: Some(EngineTime { tick, micro: 0, seq: 0 }),
        runtime: None,
        events: Vec::new(),
    };
    WsOutbound::Message(WsServerMessage::Delta {
        subscription_id: "workbench".to_string(),
        deltas: vec![
            UiPlaneDelta {
                plane: UiDataPlane::Structure,
                batch: batch.clone(),
            },
            UiPlaneDelta {
                plane: UiDataPlane::Observation,
                batch,
            },
        ],
    })
}

fn reliable_message(client_id: u64) -> WsOutbound {
    WsOutbound::Message(WsServerMessage::Hello {
        protocol_version: UI_PROTOCOL_VERSION.to_string(),
        client_id,
        session_id: "test-session".to_string(),
    })
}
