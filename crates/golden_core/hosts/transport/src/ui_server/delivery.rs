use super::*;

pub(super) fn client_accepts_subscription(client: &WsClientState, subscription_id: &str) -> bool {
    client.subscriptions.contains_key(subscription_id) || client.subscriptions.len() < MAX_SUBSCRIPTIONS_PER_CLIENT
}

pub(super) fn dispatch_ws_batches(
    read_model: &Arc<UiReadModel>,
    clients: &mut HashMap<u64, WsClientState>,
    origins: &mut HashMap<EngineTime, WsEventOrigin>,
    value_flush_due: bool,
) -> bool {
    let total_started = Instant::now();
    let mut pending = Vec::<(u64, WsServerMessage)>::new();
    let mut flushed_value_plane = false;
    let mut events_count = 0;
    let mut subscriptions_count = 0;
    let collect_started = Instant::now();
    let server_time = read_model.current_revision();
    let first_retained = read_model.first_retained_event_time();
    let last_evicted = read_model.last_evicted_event_time();

    for (client_id, client) in clients.iter_mut() {
        for (subscription_id, subscription) in client.subscriptions.iter_mut() {
            subscriptions_count += 1;
            if subscription.awaiting_resync {
                continue;
            }
            if let Some(cursor) = subscription.cursor {
                if cursor > server_time {
                    // A full snapshot covers the complete read model through
                    // `server_time`. Advance this subscription to that boundary
                    // before requesting the resync so the replacement marker is
                    // not replayed as a second resync on the next hub pass.
                    subscription.cursor = Some(server_time);
                    subscription.clear_pending_value_events();
                    pending.push((
                        *client_id,
                        WsServerMessage::ResyncRequired {
                            subscription_id: subscription_id.clone(),
                            plane: None,
                            reason: "cursor_ahead_of_server_time".to_string(),
                        },
                    ));
                    continue;
                }

                if let Some(evicted_through) = last_evicted
                    && cursor < evicted_through
                {
                    // The client must take a full snapshot, so retained deltas
                    // through the current server boundary are already covered.
                    subscription.cursor = Some(server_time);
                    subscription.clear_pending_value_events();
                    pending.push((
                        *client_id,
                        WsServerMessage::ResyncRequired {
                            subscription_id: subscription_id.clone(),
                            plane: None,
                            reason: "cursor_out_of_retention_window".to_string(),
                        },
                    ));
                    continue;
                }
            }

            let batch = read_model.replay(subscription.cursor, subscription.interest.scope.clone());
            let runtime_changed = batch.runtime != subscription.last_runtime_stats;
            subscription.last_runtime_stats = batch.runtime;
            if let Some(to) = batch.to {
                subscription.cursor = Some(to);
            }
            if batch.events.is_empty() && !runtime_changed {
                continue;
            }

            let mut visible_events = Vec::with_capacity(batch.events.len());
            for event in batch.events {
                let skip_for_sender = origins
                    .get(&event.time)
                    .is_some_and(|origin| origin.client_id == *client_id && !origin.include_self_events);
                if !skip_for_sender {
                    visible_events.push(event);
                }
            }
            let partition = read_model.partition_ui_feedback_events(visible_events);
            let values = partition.values;
            let mut structure = Vec::new();
            let mut triggers = Vec::new();
            let mut catalogs = Vec::new();
            let mut previews = Vec::new();
            let mut observations = Vec::new();
            for event in partition.other {
                match ui_data_plane(&event) {
                    UiDataPlane::Value => unreachable!("value events are partitioned by the read model"),
                    UiDataPlane::Structure => structure.push(event),
                    UiDataPlane::Trigger => triggers.push(event),
                    UiDataPlane::Catalog => catalogs.push(event),
                    UiDataPlane::Preview => previews.push(event),
                    UiDataPlane::Observation => observations.push(event),
                }
            }

            if !values.is_empty() && subscription.interest.includes(UiDataPlane::Value) {
                subscription.queue_value_events(batch.from, values);
            }
            let mut deltas = Vec::new();

            if !structure.is_empty() && subscription.interest.includes(UiDataPlane::Structure) {
                events_count += structure.len();
                deltas.push(UiPlaneDelta {
                    plane: UiDataPlane::Structure,
                    batch: UiEventBatch {
                        from: batch.from,
                        to: batch.to,
                        runtime: None,
                        events: structure,
                    },
                });
            }

            if !triggers.is_empty() && subscription.interest.includes(UiDataPlane::Trigger) {
                events_count += triggers.len();
                deltas.push(UiPlaneDelta {
                    plane: UiDataPlane::Trigger,
                    batch: UiEventBatch {
                        from: batch.from,
                        to: batch.to,
                        runtime: None,
                        events: triggers,
                    },
                });
            }

            for (plane, events) in [(UiDataPlane::Catalog, catalogs), (UiDataPlane::Preview, previews)] {
                if events.is_empty() || !subscription.interest.includes(plane) {
                    continue;
                }
                events_count += events.len();
                deltas.push(UiPlaneDelta {
                    plane,
                    batch: UiEventBatch {
                        from: batch.from,
                        to: batch.to,
                        runtime: None,
                        events,
                    },
                });
            }

            if subscription.interest.includes(UiDataPlane::Observation) && (!observations.is_empty() || runtime_changed)
            {
                events_count += observations.len();
                deltas.push(UiPlaneDelta {
                    plane: UiDataPlane::Observation,
                    batch: UiEventBatch {
                        from: batch.from,
                        to: batch.to,
                        runtime: if runtime_changed { batch.runtime } else { None },
                        events: observations,
                    },
                });
            }

            if !deltas.is_empty()
                && subscription.interest.includes(UiDataPlane::Value)
                && let Some(value_batch) = subscription.take_value_batch()
            {
                events_count += value_batch.events.len();
                flushed_value_plane = true;
                deltas.insert(
                    0,
                    UiPlaneDelta {
                        plane: UiDataPlane::Value,
                        batch: value_batch,
                    },
                );
            }

            if !deltas.is_empty() {
                pending.push((
                    *client_id,
                    WsServerMessage::Delta {
                        subscription_id: subscription_id.clone(),
                        deltas,
                    },
                ));
            }
        }
    }

    if value_flush_due {
        for (client_id, client) in clients.iter_mut() {
            for (subscription_id, subscription) in client.subscriptions.iter_mut() {
                if subscription.awaiting_resync {
                    continue;
                }
                if !subscription.interest.includes(UiDataPlane::Value) {
                    subscription.clear_pending_value_events();
                    continue;
                }
                let Some(batch) = subscription.take_value_batch() else {
                    continue;
                };
                events_count += batch.events.len();
                flushed_value_plane = true;
                pending.push((
                    *client_id,
                    WsServerMessage::Delta {
                        subscription_id: subscription_id.clone(),
                        deltas: vec![UiPlaneDelta {
                            plane: UiDataPlane::Value,
                            batch,
                        }],
                    },
                ));
            }
        }
    }
    let collect_ms = collect_started.elapsed().as_millis();

    if let Some(first_retained) = first_retained {
        origins.retain(|time, _| *time >= first_retained);
    } else {
        origins.clear();
    }

    let send_started = Instant::now();
    for (client_id, message) in pending {
        send_to_client(clients, client_id, message);
    }
    let send_ms = send_started.elapsed().as_millis();

    let total_ms = total_started.elapsed().as_millis();
    if total_ms >= 5 {
        eprintln!(
            "[ui-ws] dispatch clients={} subscriptions={} lock_wait_ms={} collect_ms={} serialize_ms=0 send_ms={} events={} total_ms={}",
            clients.len(),
            subscriptions_count,
            0,
            collect_ms,
            send_ms,
            events_count,
            total_ms
        );
    }

    flushed_value_plane
}

pub(super) fn ui_data_plane(event: &UiEventDto) -> UiDataPlane {
    match &event.kind {
        UiEventKind::ParamChanged { .. } => UiDataPlane::Trigger,
        UiEventKind::Custom {
            retention: CustomEventRetention::Latest,
            ..
        } => UiDataPlane::Preview,
        UiEventKind::Custom { topic, .. } if topic.contains("catalog") || topic.ends_with("_items_changed") => {
            UiDataPlane::Catalog
        }
        UiEventKind::Custom { topic, .. } if topic.starts_with("__runtime.") || topic.starts_with("__diagnostics.") => {
            UiDataPlane::Observation
        }
        UiEventKind::Custom { .. } => UiDataPlane::Trigger,
        _ => UiDataPlane::Structure,
    }
}

pub(super) fn send_to_client(clients: &mut HashMap<u64, WsClientState>, client_id: u64, message: WsServerMessage) {
    let delta_subscription = match &message {
        WsServerMessage::Delta { subscription_id, .. } => Some(subscription_id.clone()),
        _ => None,
    };
    if delta_subscription.as_ref().is_some_and(|subscription_id| {
        clients
            .get(&client_id)
            .and_then(|client| client.subscriptions.get(subscription_id))
            .is_some_and(|subscription| subscription.awaiting_resync)
    }) {
        return;
    }
    let result = clients
        .get(&client_id)
        .map(|client| client.outbound.push(WsOutbound::Message(message)));
    if result == Some(QueuePushResult::Full) {
        if let Some(subscription_id) = delta_subscription
            && let Some(client) = clients.get_mut(&client_id)
            && client
                .outbound
                .replace_subscription_with_resync(&subscription_id, "outbound_queue_overflow")
                == QueuePushResult::Queued
            && let Some(subscription) = client.subscriptions.get_mut(&subscription_id)
        {
            subscription.awaiting_resync = true;
            subscription.clear_pending_value_events();
            eprintln!("[ui-ws] pausing overloaded subscription '{subscription_id}' on client {client_id} for resync");
            return;
        }
        eprintln!("[ui-ws] disconnecting slow client {client_id}: reliable outbound queue exhausted");
        if let Some(client) = clients.remove(&client_id) {
            let _ = client.outbound.push(WsOutbound::Close);
        }
    }
}

pub(super) fn collect_ws_snapshot_completions(
    pending_snapshots: &mut PendingWsSnapshots,
    clients: &mut HashMap<u64, WsClientState>,
) {
    pending_snapshots.drain_ready(|completion| {
        match completion {
            WsSnapshotCompletion::Encoded {
                client_id,
                request_id,
                snapshot,
            } => {
                eprintln!(
                    "[ui-ws] snapshot request_id={} revision={:?} project_generation={} nodes={} bytes={} materialize_ms={} encode_ms={} cache_hit={}",
                    request_id,
                    snapshot.revision,
                    snapshot.project_generation,
                    snapshot.node_count,
                    snapshot.encoded.len(),
                    snapshot.materialize_elapsed.as_millis(),
                    snapshot.encode_elapsed.as_millis(),
                    snapshot.cache_hit,
                );
                send_ws_snapshot(clients, client_id, request_id, snapshot);
            }
            WsSnapshotCompletion::Failed {
                client_id,
                request_id,
                message,
            } => send_to_client(
                clients,
                client_id,
                WsServerMessage::Error { code: None,
                    message,
                    request_id: Some(request_id),
                },
            ),
        }
    });
}

pub(super) fn send_ws_snapshot(
    clients: &mut HashMap<u64, WsClientState>,
    client_id: u64,
    request_id: String,
    snapshot: SnapshotEncodingResult,
) {
    let result = clients
        .get(&client_id)
        .map(|client| client.outbound.push(WsOutbound::Snapshot { request_id, snapshot }));
    if result == Some(QueuePushResult::Full) {
        eprintln!("[ui-ws] disconnecting slow client {client_id}: snapshot outbound capacity exhausted");
        if let Some(client) = clients.remove(&client_id) {
            let _ = client.outbound.push(WsOutbound::Close);
        }
    }
}

pub(super) fn make_server_session_id() -> String {
    let epoch_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("{}-{epoch_nanos}", std::process::id())
}
