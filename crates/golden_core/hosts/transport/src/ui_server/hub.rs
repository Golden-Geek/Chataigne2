use super::*;

pub(super) fn spawn_ws_hub<T: ProjectLifecycle + 'static>(
    runtime: ProductionRuntime<T>,
    read_model: Arc<UiReadModel>,
    dispatch_interval: Duration,
    value_flush_interval: Duration,
    preferences: Option<UiPreferencesConfig>,
    session_id: String,
    snapshot_encoding: SnapshotEncodingService,
) -> WsHubHandle {
    let (cmd_tx, cmd_rx) = mpsc::sync_channel::<WsHubCommand>(WS_HUB_COMMAND_CAPACITY);
    let readiness = Arc::new(UiSessionReadiness::default());
    let readiness_for_hub = readiness.clone();
    thread::spawn(move || {
        let context = WsHubContext {
            runtime,
            read_model,
            preferences,
            session_id,
            readiness: readiness_for_hub,
            snapshot_encoding,
        };
        ws_hub_loop(context, cmd_rx, dispatch_interval, value_flush_interval)
    });
    WsHubHandle { cmd_tx, readiness }
}

pub(super) fn ws_hub_loop<T: ProjectLifecycle>(
    context: WsHubContext<T>,
    cmd_rx: Receiver<WsHubCommand>,
    dispatch_interval: Duration,
    value_flush_interval: Duration,
) {
    let mut clients = HashMap::<u64, WsClientState>::new();
    let mut origins = HashMap::<EngineTime, WsEventOrigin>::new();
    let mut last_value_flush_at = Instant::now();
    let mut pending_snapshots = PendingWsSnapshots::default();

    loop {
        match cmd_rx.recv_timeout(dispatch_interval) {
            Ok(command) => {
                handle_ws_hub_command(&context, &mut clients, &mut origins, &mut pending_snapshots, command);
                for _ in 1..WS_HUB_SERVICE_BUDGET {
                    let Ok(next) = cmd_rx.try_recv() else {
                        break;
                    };
                    handle_ws_hub_command(&context, &mut clients, &mut origins, &mut pending_snapshots, next);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        collect_ws_snapshot_completions(&mut pending_snapshots, &mut clients);

        let value_flush_due = last_value_flush_at.elapsed() >= value_flush_interval;
        if dispatch_ws_batches(&context.read_model, &mut clients, &mut origins, value_flush_due) {
            last_value_flush_at = Instant::now();
        }
    }
}

pub(super) fn handle_ws_hub_command<T: ProjectLifecycle>(
    context: &WsHubContext<T>,
    clients: &mut HashMap<u64, WsClientState>,
    origins: &mut HashMap<EngineTime, WsEventOrigin>,
    pending_snapshots: &mut PendingWsSnapshots,
    command: WsHubCommand,
) {
    match command {
        WsHubCommand::RegisterClient {
            client_id,
            outbound,
            connection,
        } => {
            if clients.len() >= MAX_WS_CLIENTS && !clients.contains_key(&client_id) {
                connection.close();
                eprintln!("[ui-ws] rejected client {client_id}: maximum of {MAX_WS_CLIENTS} clients reached");
                let _ = outbound.push(WsOutbound::Close);
                context.readiness.update(clients);
                return;
            }
            clients.insert(
                client_id,
                WsClientState {
                    outbound,
                    subscriptions: HashMap::new(),
                    connection,
                },
            );
            eprintln!(
                "[ui-ws] client {client_id} registered (connected_clients={})",
                clients.len()
            );

            send_to_client(
                clients,
                client_id,
                WsServerMessage::Hello {
                    protocol_version: UI_PROTOCOL_VERSION.to_string(),
                    client_id,
                    session_id: context.session_id.clone(),
                },
            );
        }
        WsHubCommand::BindClientInstance {
            client_id,
            client_instance_id,
        } => {
            let Some(client) = clients.get_mut(&client_id) else {
                return;
            };
            if client.connection.client_instance_id() == Some(client_instance_id.as_str()) {
                return;
            }
            if client.connection.client_instance_id().is_some() {
                client.connection.close();
                let _ = client.outbound.push(WsOutbound::Close);
                return;
            }
            client.connection = client.connection.with_client_instance_id(&client_instance_id);

            // The live set is capped at MAX_WS_CLIENTS. Derive this rare lookup
            // instead of retaining a second index after slow-client eviction.
            let previous = clients.iter().find_map(|(id, client)| {
                (*id != client_id && client.connection.client_instance_id() == Some(client_instance_id.as_str()))
                    .then_some(*id)
            });
            if let Some(previous) = previous
                && let Some(client) = clients.remove(&previous)
            {
                let _ = client.outbound.push(WsOutbound::Close);
            }
        }
        WsHubCommand::UnregisterClient { client_id } => {
            pending_snapshots.remove_client(client_id);
            let removed = clients.remove(&client_id);
            let subscription_count = removed.as_ref().map_or(0, |client| client.subscriptions.len());
            eprintln!(
                "[ui-ws] client {client_id} unregistered (removed_subscriptions={subscription_count}, connected_clients={})",
                clients.len()
            );
        }
        WsHubCommand::Subscribe {
            client_id,
            subscription_id,
            interest,
            from,
        } => {
            let subscription_limit_reached = clients
                .get(&client_id)
                .is_some_and(|client| !client_accepts_subscription(client, &subscription_id));
            if subscription_limit_reached {
                send_to_client(
                    clients,
                    client_id,
                    WsServerMessage::Error {
                        code: None,
                        message: format!("maximum of {MAX_SUBSCRIPTIONS_PER_CLIENT} subscriptions per client reached"),
                        request_id: None,
                    },
                );
                context.readiness.update(clients);
                return;
            }
            let project_generation = context.read_model.current_project_generation();
            let stale = from.is_some_and(|cursor| cursor.project_generation != project_generation.get());
            if let Some(client) = clients.get_mut(&client_id) {
                let _replaced = client
                    .subscriptions
                    .insert(
                        subscription_id.clone(),
                        WsSubscriptionState {
                            interest,
                            project_generation,
                            cursor: from.map(|cursor| cursor.at),
                            last_runtime_stats: None,
                            pending_value_events: PendingValueEvents::default(),
                            awaiting_resync: stale,
                        },
                    )
                    .is_some();
            }
            if stale {
                send_to_client(
                    clients,
                    client_id,
                    WsServerMessage::ResyncRequired {
                        subscription_id,
                        plane: None,
                        reason: "project_generation_changed".into(),
                    },
                );
            }
        }
        WsHubCommand::Unsubscribe {
            client_id,
            subscription_id,
        } => {
            if let Some(client) = clients.get_mut(&client_id) {
                let _existed = client.subscriptions.remove(&subscription_id).is_some();
            } else {
                eprintln!("[ui-ws] unsubscribe ignored for unknown client {client_id} id='{subscription_id}'");
            }
        }
        WsHubCommand::Snapshot {
            client_id,
            request_id,
            scope,
        } => {
            let capture = context.read_model.capture_snapshot(scope);
            match pending_snapshots.submit(&context.snapshot_encoding, client_id, request_id.clone(), capture) {
                Ok(()) => {}
                Err(error) => send_to_client(
                    clients,
                    client_id,
                    WsServerMessage::Error {
                        code: None,
                        message: error.to_string(),
                        request_id: Some(request_id),
                    },
                ),
            }
        }
        WsHubCommand::Replay {
            client_id,
            request_id,
            scope,
            from,
        } => {
            let replay = context.read_model.replay_from_cursor(from, scope);
            send_to_client(
                clients,
                client_id,
                WsServerMessage::Replay {
                    request_id,
                    project_generation: replay.project_generation,
                    batch: replay.batch,
                },
            );
        }
        WsHubCommand::Intent {
            client_id,
            request_id,
            intent,
            include_self_events,
        } => {
            let kind = ui_intent_kind(&intent);
            let Some(batch) = apply_ws_transactions(context, clients, client_id, &request_id, vec![*intent]) else {
                return;
            };
            let result = batch.transactions.into_iter().next().expect("single-intent batch");
            save_preferences_after_change(
                &context.runtime,
                context.preferences.as_ref(),
                result.preferences_changed,
            );
            let timing = ui_intent_timing_from_application(kind, &result);
            log_ui_intent_timing("ui-ws", &timing);

            for time in result.events.events.iter().map(|event| event.time) {
                origins.insert(
                    time,
                    WsEventOrigin {
                        project_generation: result.project_generation,
                        client_id,
                        include_self_events,
                    },
                );
            }

            let phase = if result.acknowledgement.success {
                UiControlPhase::Applied
            } else {
                UiControlPhase::Rejected
            };
            send_to_client(
                clients,
                client_id,
                WsServerMessage::Control {
                    update: UiControlUpdate {
                        request_id,
                        phase,
                        acknowledgement: Some(result.acknowledgement),
                        acknowledgements: Vec::new(),
                    },
                },
            );
        }
        WsHubCommand::IntentBatch {
            client_id,
            request_id,
            intents,
            include_self_events,
        } => {
            let total_started = Instant::now();
            let Some(batch) = apply_ws_transactions(context, clients, client_id, &request_id, intents) else {
                return;
            };
            save_preferences_after_change(
                &context.runtime,
                context.preferences.as_ref(),
                batch.preferences_changed,
            );
            let mut produced_times = Vec::<(ProjectGeneration, EngineTime)>::new();
            let mut acks = Vec::<UiAck>::with_capacity(batch.transactions.len());
            let mut timing_rows = Vec::<UiIntentTiming>::with_capacity(batch.transactions.len());
            for result in batch.transactions {
                produced_times.extend(
                    result
                        .events
                        .events
                        .iter()
                        .map(|event| (result.project_generation, event.time)),
                );
                timing_rows.push(ui_intent_timing_from_application("batch", &result));
                acks.push(result.acknowledgement);
            }
            let batch_total_ms = total_started.elapsed().as_millis();
            for timing in &timing_rows {
                log_ui_intent_timing("ui-ws", timing);
            }
            if acks.len() > 1 || batch_total_ms >= 5 {
                eprintln!(
                    "[ui-ws] intent_batch count={} events={} total_ms={}",
                    acks.len(),
                    produced_times.len(),
                    batch_total_ms
                );
            }

            for (project_generation, time) in produced_times {
                origins.insert(
                    time,
                    WsEventOrigin {
                        project_generation,
                        client_id,
                        include_self_events,
                    },
                );
            }

            let phase = if acks.iter().all(|ack| ack.success) {
                UiControlPhase::Applied
            } else {
                UiControlPhase::Rejected
            };
            send_to_client(
                clients,
                client_id,
                WsServerMessage::Control {
                    update: UiControlUpdate {
                        request_id,
                        phase,
                        acknowledgement: None,
                        acknowledgements: acks,
                    },
                },
            );
        }
    }
    context.readiness.update(clients);
}

fn apply_ws_transactions<T: ProjectLifecycle>(
    context: &WsHubContext<T>,
    clients: &mut HashMap<u64, WsClientState>,
    client_id: u64,
    request_id: &str,
    intents: Vec<UiEditIntent>,
) -> Option<golden_engine::application::AppliedUiTransactionBatch> {
    let client = clients.get(&client_id)?;
    let pending = match context
        .runtime
        .submit_ui_client_transaction_batch(intents, &client.connection, true)
    {
        Ok(pending) => pending,
        Err(error) => {
            send_application_error(clients, client_id, request_id.to_owned(), &error);
            return None;
        }
    };
    send_to_client(
        clients,
        client_id,
        WsServerMessage::Control {
            update: UiControlUpdate::pending(request_id.to_owned(), UiControlPhase::Accepted),
        },
    );
    match pending.wait() {
        Ok(result) => Some(result),
        Err(error) => {
            send_application_error(clients, client_id, request_id.to_owned(), &error);
            None
        }
    }
}
