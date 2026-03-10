use std::collections::HashMap;
use std::io::{Error, ErrorKind, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::app::{prepare_engine_for_runtime, project_host, ProjectLifecycle};
use crate::engine::{Engine, EngineTime};
use crate::node::Node;
use crate::ui_sync::{
    UiAck, UiAckStatus, UiContextCandidatesRequest as ContextCandidatesRequest, UiEditIntent, UiEventBatch, UiEventDto,
    UiEventKind, UiParamControlInfoRequest as ParamControlInfoRequest, UiProjectPathRequest as ProjectPathRequest,
    UiReferenceTargetsRequest as ReferenceTargetsRequest, UiReplayRequest as ReplayRequest,
    UiScriptConfigRequest as ScriptConfigRequest, UiScriptReloadRequest as ScriptReloadRequest,
    UiScriptStateRequest as ScriptStateRequest, UiSnapshotRequest as SnapshotRequest, UiSubscriptionScope,
    UI_PROTOCOL_VERSION,
};
use serde::{Deserialize, Serialize};

const WS_MAX_PAYLOAD_BYTES: usize = 1024 * 1024;
const WS_RETRY_INTERVAL: Duration = Duration::from_millis(16);
const WS_PING_INTERVAL: Duration = Duration::from_secs(10);
const WS_PONG_TIMEOUT: Duration = Duration::from_secs(30);

static NEXT_WS_CLIENT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
/// Runtime settings for the built-in HTTP and WebSocket UI server.
pub struct UiServerConfig {
    /// Socket address where the built-in UI server listens.
    pub bind_addr: String,
    /// Runtime tick interval used by the engine loop hosted by the UI server.
    pub tick_interval: Duration,
}

impl Default for UiServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "localhost:7010".to_string(),
            tick_interval: Duration::from_millis(16),
        }
    }
}

struct ServerState<T: ProjectLifecycle> {
    engine: Arc<Mutex<Engine<T>>>,
    ws_hub: WsHubHandle,
}

impl<T: ProjectLifecycle> Clone for ServerState<T> {
    fn clone(&self) -> Self {
        Self {
            engine: self.engine.clone(),
            ws_hub: self.ws_hub.clone(),
        }
    }
}

fn apply_ui_intent_with_transport<T: ProjectLifecycle>(
    engine: &mut Engine<T>,
    intent: UiEditIntent,
    ui_client_instance_id: Option<&str>,
) -> UiAck {
    let before_len = engine.ui_event_log().len();

    match intent {
        UiEditIntent::DuplicateNode {
            source,
            new_parent,
            new_prev_sibling,
            label,
        } => match engine.duplicate_subtree_with(
            source,
            new_parent,
            new_prev_sibling,
            label,
            |node| node.project_encode_data(),
            |node_type, data, meta| T::project_decode_node(node_type, data, meta),
        ) {
            Ok(_) => UiAck {
                success: true,
                status: UiAckStatus::Applied,
                error_code: None,
                error_message: None,
                earliest_event_time: engine.ui_event_log().get(before_len).map(|event| event.time),
                history: engine.ui_history_state(),
            },
            Err(err) => UiAck {
                success: false,
                status: UiAckStatus::Rejected,
                error_code: Some("duplicate_node_failed".to_string()),
                error_message: Some(err.to_string()),
                earliest_event_time: None,
                history: engine.ui_history_state(),
            },
        },
        other => engine.apply_ui_intent_from_client(other, ui_client_instance_id),
    }
}

fn normalize_ui_client_instance_id(raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.is_empty() || value.len() > 128 {
        return None;
    }
    Some(value.to_string())
}

fn ui_client_instance_id_from_headers(headers: &HashMap<String, String>) -> Option<String> {
    headers
        .get("x-gc-ui-client-instance")
        .and_then(|value| normalize_ui_client_instance_id(value))
}

#[derive(Clone)]
struct WsHubHandle {
    cmd_tx: Sender<WsHubCommand>,
}

struct HttpRequest {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

struct WsClientState {
    outbound: Sender<WsOutbound>,
    subscriptions: HashMap<String, WsSubscriptionState>,
    client_instance_id: Option<String>,
}

struct WsSubscriptionState {
    scope: UiSubscriptionScope,
    cursor: Option<EngineTime>,
}

struct WsEventOrigin {
    client_id: u64,
    include_self_events: bool,
}

enum WsHubCommand {
    RegisterClient {
        client_id: u64,
        outbound: Sender<WsOutbound>,
    },
    BindClientInstance {
        client_id: u64,
        client_instance_id: String,
    },
    UnregisterClient {
        client_id: u64,
    },
    Subscribe {
        client_id: u64,
        subscription_id: String,
        scope: UiSubscriptionScope,
        from: Option<EngineTime>,
    },
    Unsubscribe {
        client_id: u64,
        subscription_id: String,
    },
    Intent {
        client_id: u64,
        request_id: String,
        intent: UiEditIntent,
        include_self_events: bool,
    },
    IntentBatch {
        client_id: u64,
        request_id: String,
        intents: Vec<UiEditIntent>,
        include_self_events: bool,
    },
}

enum WsOutbound {
    Message(WsServerMessage),
    Ping(Vec<u8>),
    Pong(Vec<u8>),
    Close,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum WsServerMessage {
    Hello {
        protocol_version: String,
        client_id: u64,
        session_id: String,
    },
    Batch {
        subscription_id: String,
        batch: UiEventBatch,
    },
    IntentAck {
        request_id: String,
        ack: UiAck,
    },
    IntentBatchAck {
        request_id: String,
        acks: Vec<UiAck>,
    },
    ResyncRequired {
        subscription_id: String,
        reason: String,
    },
    Error {
        message: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
    },
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum WsClientMessage {
    Hello {
        protocol_version: String,
        #[serde(default)]
        client_instance_id: Option<String>,
    },
    Subscribe {
        subscription_id: String,
        scope: UiSubscriptionScope,
        #[serde(default)]
        from: Option<EngineTime>,
    },
    Unsubscribe {
        subscription_id: String,
    },
    Intent {
        request_id: String,
        intent: UiEditIntent,
        #[serde(default)]
        include_self_events: bool,
    },
    IntentBatch {
        request_id: String,
        intents: Vec<UiEditIntent>,
        #[serde(default)]
        include_self_events: bool,
    },
}

enum WsIncomingFrame {
    Text(String),
    Close,
    Ping(Vec<u8>),
    Pong,
}

/// Prepares an engine and runs it through the built-in HTTP and WebSocket host.
pub fn run_with_ui_server_config<T: ProjectLifecycle + 'static>(
    mut engine: Engine<T>,
    config: UiServerConfig,
) -> std::io::Result<()> {
    prepare_engine_for_runtime(&mut engine)?;
    let shared_engine = Arc::new(Mutex::new(engine));
    run_ui_server(shared_engine, config)
}

/// Runs the built-in HTTP and WebSocket host around a shared engine instance.
pub fn run_ui_server<T: ProjectLifecycle + 'static>(
    engine: Arc<Mutex<Engine<T>>>,
    config: UiServerConfig,
) -> std::io::Result<()> {
    spawn_runtime_loop(engine.clone(), config.tick_interval);
    let ws_hub = spawn_ws_hub(
        engine.clone(),
        config.tick_interval.max(WS_RETRY_INTERVAL),
        make_server_session_id(),
    );

    let listener = TcpListener::bind(&config.bind_addr)?;
    println!("UI API listening on http://{}", config.bind_addr);

    let state = ServerState { engine, ws_hub };

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let state = state.clone();
                thread::spawn(move || {
                    if let Err(err) = handle_connection(&mut stream, &state) {
                        eprintln!("ui server request failed: {err}");
                    }
                });
            }
            Err(err) => {
                eprintln!("ui server accept failed: {err}");
            }
        }
    }

    Ok(())
}

fn spawn_runtime_loop<T: Node + 'static>(engine: Arc<Mutex<Engine<T>>>, tick_interval: Duration) {
    thread::spawn(move || {
        let mut last_tick_start = Instant::now();

        loop {
            let tick_start = Instant::now();
            let elapsed = tick_start.saturating_duration_since(last_tick_start);
            last_tick_start = tick_start;

            let mut guard = match engine.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };

            if let Err(err) = guard.run_tick(elapsed) {
                eprintln!("runtime tick failed: {err}");
            }
            drop(guard);

            let spent = tick_start.elapsed();
            if spent < tick_interval {
                thread::sleep(tick_interval - spent);
            } else {
                thread::yield_now();
            }
        }
    });
}

fn spawn_ws_hub<T: ProjectLifecycle + 'static>(
    engine: Arc<Mutex<Engine<T>>>,
    dispatch_interval: Duration,
    session_id: String,
) -> WsHubHandle {
    let (cmd_tx, cmd_rx) = mpsc::channel::<WsHubCommand>();
    thread::spawn(move || ws_hub_loop(engine, cmd_rx, dispatch_interval, session_id));
    WsHubHandle { cmd_tx }
}

fn ws_hub_loop<T: ProjectLifecycle>(
    engine: Arc<Mutex<Engine<T>>>,
    cmd_rx: Receiver<WsHubCommand>,
    dispatch_interval: Duration,
    session_id: String,
) {
    let mut clients = HashMap::<u64, WsClientState>::new();
    let mut client_instances = HashMap::<String, u64>::new();
    let mut origins = HashMap::<EngineTime, WsEventOrigin>::new();

    loop {
        match cmd_rx.recv_timeout(dispatch_interval) {
            Ok(command) => {
                handle_ws_hub_command(
                    &engine,
                    &mut clients,
                    &mut client_instances,
                    &mut origins,
                    command,
                    &session_id,
                );
                while let Ok(next) = cmd_rx.try_recv() {
                    handle_ws_hub_command(
                        &engine,
                        &mut clients,
                        &mut client_instances,
                        &mut origins,
                        next,
                        &session_id,
                    );
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        dispatch_ws_batches(&engine, &mut clients, &mut origins);
    }
}

fn handle_ws_hub_command<T: ProjectLifecycle>(
    engine: &Arc<Mutex<Engine<T>>>,
    clients: &mut HashMap<u64, WsClientState>,
    client_instances: &mut HashMap<String, u64>,
    origins: &mut HashMap<EngineTime, WsEventOrigin>,
    command: WsHubCommand,
    session_id: &str,
) {
    match command {
        WsHubCommand::RegisterClient { client_id, outbound } => {
            clients.insert(
                client_id,
                WsClientState {
                    outbound,
                    subscriptions: HashMap::new(),
                    client_instance_id: None,
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
                    session_id: session_id.to_string(),
                },
            );
        }
        WsHubCommand::BindClientInstance {
            client_id,
            client_instance_id,
        } => {
            let previous_instance_id = {
                let Some(client) = clients.get_mut(&client_id) else {
                    return;
                };
                client.client_instance_id.replace(client_instance_id.clone())
            };

            if let Some(previous_instance_id) = previous_instance_id {
                if client_instances
                    .get(&previous_instance_id)
                    .is_some_and(|mapped_client_id| *mapped_client_id == client_id)
                {
                    client_instances.remove(&previous_instance_id);
                }
            }

            if let Some(previous_client_id) = client_instances.insert(client_instance_id.clone(), client_id) {
                if previous_client_id != client_id {
                    if let Some(previous_client) = clients.remove(&previous_client_id) {
                        let _ = previous_client.outbound.send(WsOutbound::Close);
                    }

                    let mut guard = lock_engine(engine);
                    let _ = guard.cancel_active_ui_edit_session_for_client(&client_instance_id);
                }
            }
        }
        WsHubCommand::UnregisterClient { client_id } => {
            let removed = clients.remove(&client_id);
            let subscription_count = removed.as_ref().map_or(0, |client| client.subscriptions.len());
            if let Some(client_instance_id) = removed.as_ref().and_then(|client| client.client_instance_id.as_ref()) {
                if client_instances
                    .get(client_instance_id)
                    .is_some_and(|mapped_client_id| *mapped_client_id == client_id)
                {
                    client_instances.remove(client_instance_id);
                }

                let mut guard = lock_engine(engine);
                let _ = guard.cancel_active_ui_edit_session_for_client(client_instance_id);
            }
            eprintln!(
                "[ui-ws] client {client_id} unregistered (removed_subscriptions={subscription_count}, connected_clients={})",
                clients.len()
            );
        }
        WsHubCommand::Subscribe {
            client_id,
            subscription_id,
            scope,
            from,
        } => {
            if let Some(client) = clients.get_mut(&client_id) {
                let _replaced = client
                    .subscriptions
                    .insert(subscription_id, WsSubscriptionState { scope, cursor: from })
                    .is_some();
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
        WsHubCommand::Intent {
            client_id,
            request_id,
            intent,
            include_self_events,
        } => {
            let (ack, produced_times) = {
                let mut guard = lock_engine(engine);
                let before_len = guard.ui_event_log().len();
                let client_instance_id = clients
                    .get(&client_id)
                    .and_then(|client| client.client_instance_id.as_deref());
                let ack = apply_ui_intent_with_transport(&mut guard, intent, client_instance_id);
                let produced_times = guard
                    .ui_event_log()
                    .iter()
                    .skip(before_len)
                    .map(|event| event.time)
                    .collect::<Vec<_>>();
                (ack, produced_times)
            };

            for time in produced_times {
                origins.insert(
                    time,
                    WsEventOrigin {
                        client_id,
                        include_self_events,
                    },
                );
            }

            send_to_client(clients, client_id, WsServerMessage::IntentAck { request_id, ack });
        }
        WsHubCommand::IntentBatch {
            client_id,
            request_id,
            intents,
            include_self_events,
        } => {
            let (acks, produced_times) = {
                let mut guard = lock_engine(engine);
                let mut acks = Vec::<UiAck>::with_capacity(intents.len());
                let mut produced_times = Vec::<EngineTime>::new();
                let client_instance_id = clients
                    .get(&client_id)
                    .and_then(|client| client.client_instance_id.as_deref());

                for intent in intents {
                    let before_len = guard.ui_event_log().len();
                    let ack = apply_ui_intent_with_transport(&mut guard, intent, client_instance_id);
                    acks.push(ack);
                    produced_times.extend(guard.ui_event_log().iter().skip(before_len).map(|event| event.time));
                }

                (acks, produced_times)
            };

            for time in produced_times {
                origins.insert(
                    time,
                    WsEventOrigin {
                        client_id,
                        include_self_events,
                    },
                );
            }

            send_to_client(clients, client_id, WsServerMessage::IntentBatchAck { request_id, acks });
        }
    }
}

fn dispatch_ws_batches<T: Node>(
    engine: &Arc<Mutex<Engine<T>>>,
    clients: &mut HashMap<u64, WsClientState>,
    origins: &mut HashMap<EngineTime, WsEventOrigin>,
) {
    let mut pending = Vec::<(u64, WsServerMessage)>::new();

    let first_retained = {
        let mut guard = lock_engine(engine);
        guard.sync_logger_ui_events();
        let server_time = guard.time;
        let first_retained = guard.ui_event_log().first().map(|event| event.time);

        for (client_id, client) in clients.iter_mut() {
            for (subscription_id, subscription) in client.subscriptions.iter_mut() {
                if let Some(cursor) = subscription.cursor {
                    if cursor > server_time {
                        subscription.cursor = None;
                        pending.push((
                            *client_id,
                            WsServerMessage::ResyncRequired {
                                subscription_id: subscription_id.clone(),
                                reason: "cursor_ahead_of_server_time".to_string(),
                            },
                        ));
                        continue;
                    }

                    if let Some(first_time) = first_retained {
                        if cursor < first_time {
                            subscription.cursor = Some(first_time);
                            pending.push((
                                *client_id,
                                WsServerMessage::ResyncRequired {
                                    subscription_id: subscription_id.clone(),
                                    reason: "cursor_out_of_retention_window".to_string(),
                                },
                            ));
                            continue;
                        }
                    }
                }

                let batch = guard.ui_event_batch(subscription.cursor, subscription.scope.clone());
                if let Some(to) = batch.to {
                    subscription.cursor = Some(to);
                }
                if batch.events.is_empty() {
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

                if !visible_events.is_empty() {
                    pending.push((
                        *client_id,
                        WsServerMessage::Batch {
                            subscription_id: subscription_id.clone(),
                            batch: UiEventBatch {
                                from: batch.from,
                                to: batch.to,
                                events: visible_events,
                            },
                        },
                    ));
                }
            }
        }

        first_retained
    };

    if let Some(first_time) = first_retained {
        origins.retain(|time, _| *time >= first_time);
    } else {
        origins.clear();
    }

    let mut disconnected = Vec::<u64>::new();
    for (client_id, message) in pending {
        if let Some(client) = clients.get(&client_id) {
            if client.outbound.send(WsOutbound::Message(message)).is_err() {
                disconnected.push(client_id);
            }
        }
    }

    disconnected.sort_unstable();
    disconnected.dedup();
    for client_id in disconnected {
        clients.remove(&client_id);
    }
}

fn send_to_client(clients: &mut HashMap<u64, WsClientState>, client_id: u64, message: WsServerMessage) {
    let mut is_disconnected = false;
    if let Some(client) = clients.get(&client_id) {
        if client.outbound.send(WsOutbound::Message(message)).is_err() {
            is_disconnected = true;
        }
    }

    if is_disconnected {
        clients.remove(&client_id);
    }
}

fn make_server_session_id() -> String {
    let epoch_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("{}-{epoch_nanos}", std::process::id())
}

fn make_resync_event_batch(from: Option<EngineTime>, time: EngineTime, reason: &str) -> UiEventBatch {
    UiEventBatch {
        from,
        to: Some(time),
        events: vec![UiEventDto {
            time,
            kind: UiEventKind::Custom {
                topic: "__transport.resync_required".to_string(),
                origin: None,
                payload: serde_json::json!({ "reason": reason }),
            },
        }],
    }
}

fn handle_connection<T: ProjectLifecycle>(stream: &mut TcpStream, state: &ServerState<T>) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;

    let request = match read_http_request(stream) {
        Ok(request) => request,
        Err(err) => {
            if is_client_disconnect_error(&err) {
                return Ok(());
            }

            if let Err(write_err) = write_json_error(stream, "400 Bad Request", &format!("invalid request: {err}"))
            {
                if !is_client_disconnect_error(&write_err) {
                    return Err(write_err);
                }
            }
            return Ok(());
        }
    };

    if request.method.eq_ignore_ascii_case("OPTIONS") {
        write_response(stream, "204 No Content", "text/plain", &[])?;
        return Ok(());
    }

    if request.method.eq_ignore_ascii_case("GET") && request.path == "/api/ui/ws" {
        if !is_websocket_upgrade_request(&request) {
            write_json_error(stream, "426 Upgrade Required", "websocket upgrade required")?;
            return Ok(());
        }
        return handle_ws_connection(stream, state, &request);
    }

    let route = request.path.as_str();
    match (request.method.as_str(), route) {
        ("GET", "/api/ui/health") => {
            write_json(stream, "200 OK", &serde_json::json!({ "ok": true }))?;
        }
        ("GET", "/api/ui/user-contexts") => {
            let guard = lock_engine(&state.engine);
            let contexts = guard.ui_user_contexts();
            drop(guard);

            write_json(stream, "200 OK", &contexts)?;
        }
        ("POST", "/api/ui/snapshot") => {
            let payload: SnapshotRequest = if request.body.is_empty() {
                SnapshotRequest {
                    scope: UiSubscriptionScope::WholeGraph,
                    cancel_active_edit_session: false,
                }
            } else {
                serde_json::from_slice(&request.body)
                    .map_err(|err| Error::new(ErrorKind::InvalidData, err.to_string()))?
            };
            let request_started = Instant::now();
            let scope = payload.scope;
            let client_instance_id = ui_client_instance_id_from_headers(&request.headers);
            let guard = lock_engine(&state.engine);
            let mut guard = guard;
            if payload.cancel_active_edit_session {
                if let Some(client_instance_id) = client_instance_id.as_deref() {
                    let _ = guard.cancel_active_ui_edit_session_for_client(client_instance_id);
                }
            }
            let build_started = Instant::now();
            let snapshot = guard.ui_snapshot(scope.clone());
            let build_elapsed = build_started.elapsed();
            drop(guard);

            let serialize_started = Instant::now();
            let body = serde_json::to_vec(&snapshot)
                .map_err(|err| Error::new(ErrorKind::InvalidData, format!("failed to serialize json: {err}")))?;
            let serialize_elapsed = serialize_started.elapsed();
            eprintln!(
                "[ui-http] snapshot scope={scope:?} nodes={} bytes={} build_ms={} serialize_ms={} total_ms={}",
                snapshot.nodes.len(),
                body.len(),
                build_elapsed.as_millis(),
                serialize_elapsed.as_millis(),
                request_started.elapsed().as_millis()
            );

            write_response(stream, "200 OK", "application/json; charset=utf-8", &body)?;
        }
        ("POST", "/api/ui/replay") => {
            let payload: ReplayRequest = serde_json::from_slice(&request.body)
                .map_err(|err| Error::new(ErrorKind::InvalidData, format!("invalid replay payload: {err}")))?;
            eprintln!("[ui-http] replay scope={:?} from={:?}", payload.scope, payload.from);

            let mut guard = lock_engine(&state.engine);
            guard.sync_logger_ui_events();
            let first_retained = guard.ui_event_log().first().map(|event| event.time);
            let batch = if let Some(from) = payload.from {
                if from > guard.time {
                    make_resync_event_batch(payload.from, guard.time, "cursor_ahead_of_server_time")
                } else if let Some(first_time) = first_retained {
                    if from < first_time {
                        make_resync_event_batch(payload.from, guard.time, "cursor_out_of_retention_window")
                    } else {
                        guard.ui_event_batch(payload.from, payload.scope)
                    }
                } else {
                    guard.ui_event_batch(payload.from, payload.scope)
                }
            } else {
                guard.ui_event_batch(payload.from, payload.scope)
            };
            drop(guard);

            write_json(stream, "200 OK", &batch)?;
        }
        ("POST", "/api/ui/reference-targets") => {
            let payload: ReferenceTargetsRequest = serde_json::from_slice(&request.body).map_err(|err| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!("invalid reference-targets payload: {err}"),
                )
            })?;

            let guard = lock_engine(&state.engine);
            let targets = guard.ui_reference_targets_for_param(payload.param);
            drop(guard);

            write_json(stream, "200 OK", &targets)?;
        }
        ("POST", "/api/ui/context-candidates") => {
            let payload: ContextCandidatesRequest = serde_json::from_slice(&request.body).map_err(|err| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!("invalid context-candidates payload: {err}"),
                )
            })?;

            let guard = lock_engine(&state.engine);
            let candidates = guard.ui_context_candidates_for_param(payload.param);
            drop(guard);

            write_json(stream, "200 OK", &candidates)?;
        }
        ("POST", "/api/ui/param-control-info") => {
            let payload: ParamControlInfoRequest = serde_json::from_slice(&request.body).map_err(|err| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!("invalid param-control-info payload: {err}"),
                )
            })?;

            let guard = lock_engine(&state.engine);
            let info_result = guard.ui_param_control_info(payload.param);
            drop(guard);

            match info_result {
                Ok(info) => {
                    write_json(stream, "200 OK", &info)?;
                }
                Err(err) => {
                    write_json_error(stream, "400 Bad Request", &err)?;
                }
            }
        }
        ("POST", "/api/ui/script-state") => {
            let payload: ScriptStateRequest = serde_json::from_slice(&request.body)
                .map_err(|err| Error::new(ErrorKind::InvalidData, format!("invalid script-state payload: {err}")))?;

            let guard = lock_engine(&state.engine);
            let state_result = guard.ui_script_state(payload.node);
            drop(guard);

            match state_result {
                Ok(script_state) => {
                    write_json(stream, "200 OK", &script_state)?;
                }
                Err(err) => {
                    write_json_error(stream, "400 Bad Request", &err)?;
                }
            }
        }
        ("POST", "/api/ui/script-config") => {
            let payload: ScriptConfigRequest = serde_json::from_slice(&request.body)
                .map_err(|err| Error::new(ErrorKind::InvalidData, format!("invalid script-config payload: {err}")))?;

            let mut guard = lock_engine(&state.engine);
            let update_result = guard.ui_set_script_config(payload.node, payload.config, payload.force_reload);
            drop(guard);

            match update_result {
                Ok(()) => {
                    write_json(stream, "200 OK", &serde_json::json!({ "ok": true }))?;
                }
                Err(err) => {
                    write_json_error(stream, "400 Bad Request", &err)?;
                }
            }
        }
        ("POST", "/api/ui/script-reload") => {
            let payload: ScriptReloadRequest = serde_json::from_slice(&request.body)
                .map_err(|err| Error::new(ErrorKind::InvalidData, format!("invalid script-reload payload: {err}")))?;

            let mut guard = lock_engine(&state.engine);
            let reload_result = guard.ui_reload_script(payload.node);
            drop(guard);

            match reload_result {
                Ok(()) => {
                    write_json(stream, "200 OK", &serde_json::json!({ "ok": true }))?;
                }
                Err(err) => {
                    write_json_error(stream, "400 Bad Request", &err)?;
                }
            }
        }
        ("POST", "/api/ui/project-new") => match project_host::create_new_project(&state.engine) {
            Ok(()) => {
                write_json(stream, "200 OK", &serde_json::json!({ "ok": true }))?;
            }
            Err(err) => {
                write_json_error(stream, "400 Bad Request", &format!("project-new failed: {err}"))?;
            }
        },
        ("POST", "/api/ui/project-save") => {
            let payload: ProjectPathRequest = serde_json::from_slice(&request.body)
                .map_err(|err| Error::new(ErrorKind::InvalidData, format!("invalid project-save payload: {err}")))?;
            match project_host::save_project(&state.engine, &payload.path) {
                Ok(()) => {
                    write_json(stream, "200 OK", &serde_json::json!({ "ok": true }))?;
                }
                Err(err) => {
                    write_json_error(stream, "400 Bad Request", &format!("project-save failed: {err}"))?;
                }
            }
        }
        ("POST", "/api/ui/project-load") => {
            let payload: ProjectPathRequest = serde_json::from_slice(&request.body)
                .map_err(|err| Error::new(ErrorKind::InvalidData, format!("invalid project-load payload: {err}")))?;
            match project_host::load_project(&state.engine, &payload.path) {
                Ok(()) => {
                    write_json(stream, "200 OK", &serde_json::json!({ "ok": true }))?;
                }
                Err(err) => {
                    write_json_error(stream, "400 Bad Request", &format!("project-load failed: {err}"))?;
                }
            }
        }
        ("POST", "/api/ui/intent") => {
            let intent: UiEditIntent = serde_json::from_slice(&request.body)
                .map_err(|err| Error::new(ErrorKind::InvalidData, format!("invalid intent payload: {err}")))?;
            let client_instance_id = ui_client_instance_id_from_headers(&request.headers);

            let mut guard = lock_engine(&state.engine);
            let ack = apply_ui_intent_with_transport(&mut guard, intent, client_instance_id.as_deref());
            drop(guard);

            write_json(stream, "200 OK", &ack)?;
        }
        ("POST", "/api/ui/intent/batch") => {
            let intents: Vec<UiEditIntent> = serde_json::from_slice(&request.body)
                .map_err(|err| Error::new(ErrorKind::InvalidData, format!("invalid intent batch payload: {err}")))?;
            let client_instance_id = ui_client_instance_id_from_headers(&request.headers);

            let mut guard = lock_engine(&state.engine);
            let mut acks = Vec::<UiAck>::with_capacity(intents.len());
            for intent in intents {
                acks.push(apply_ui_intent_with_transport(
                    &mut guard,
                    intent,
                    client_instance_id.as_deref(),
                ));
            }
            drop(guard);

            write_json(stream, "200 OK", &acks)?;
        }
        _ => {
            write_json_error(stream, "404 Not Found", "unknown route")?;
        }
    }

    Ok(())
}

fn handle_ws_connection<T: ProjectLifecycle>(
    stream: &mut TcpStream,
    state: &ServerState<T>,
    request: &HttpRequest,
) -> std::io::Result<()> {
    let version = request
        .headers
        .get("sec-websocket-version")
        .map(String::as_str)
        .unwrap_or("");
    if version.trim() != "13" {
        write_json_error(stream, "426 Upgrade Required", "unsupported websocket version")?;
        return Ok(());
    }

    let key = request
        .headers
        .get("sec-websocket-key")
        .ok_or_else(|| Error::new(ErrorKind::InvalidData, "missing sec-websocket-key"))?;
    let accept = websocket_accept_key(key);

    let response = format!(
        "HTTP/1.1 101 Switching Protocols\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Accept: {accept}\r\n\
         Access-Control-Allow-Origin: *\r\n\r\n"
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()?;

    let client_id = NEXT_WS_CLIENT_ID.fetch_add(1, Ordering::Relaxed);
    eprintln!("[ui-ws] upgraded connection to websocket (client_id={client_id})");
    let (outbound_tx, outbound_rx) = mpsc::channel::<WsOutbound>();
    state
        .ws_hub
        .cmd_tx
        .send(WsHubCommand::RegisterClient {
            client_id,
            outbound: outbound_tx.clone(),
        })
        .map_err(|_| Error::new(ErrorKind::BrokenPipe, "websocket hub unavailable"))?;

    stream.set_read_timeout(Some(Duration::from_millis(250)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;

    let writer_stream = stream.try_clone()?;
    let writer_handle = thread::spawn(move || ws_writer_loop(writer_stream, outbound_rx));
    let mut last_ping_at = Instant::now();
    let mut awaiting_pong_since = None::<Instant>;

    loop {
        let frame = match read_ws_frame(stream) {
            Ok(Some(frame)) => Some(frame),
            Ok(None) => break,
            Err(err) if err.kind() == ErrorKind::TimedOut || err.kind() == ErrorKind::WouldBlock => None,
            Err(err) => {
                eprintln!("websocket read failed for client {client_id}: {err}");
                break;
            }
        };

        if let Some(frame) = frame {
            match frame {
                WsIncomingFrame::Text(text) => {
                    awaiting_pong_since = None;
                    let message: WsClientMessage = match serde_json::from_str(&text) {
                        Ok(message) => message,
                        Err(err) => {
                            let _ = outbound_tx.send(WsOutbound::Message(WsServerMessage::Error {
                                message: format!("invalid websocket message: {err}"),
                                request_id: None,
                            }));
                            continue;
                        }
                    };

                    if !handle_ws_client_message(message, client_id, &state.ws_hub, &outbound_tx) {
                        break;
                    }
                }
                WsIncomingFrame::Ping(payload) => {
                    awaiting_pong_since = None;
                    let _ = outbound_tx.send(WsOutbound::Pong(payload));
                }
                WsIncomingFrame::Pong => {
                    awaiting_pong_since = None;
                }
                WsIncomingFrame::Close => break,
            }
        }

        let now = Instant::now();
        if let Some(since) = awaiting_pong_since {
            if now.duration_since(since) > WS_PONG_TIMEOUT {
                eprintln!("[ui-ws] client {client_id} timed out waiting for pong");
                break;
            }
        }

        if now.duration_since(last_ping_at) >= WS_PING_INTERVAL {
            if outbound_tx.send(WsOutbound::Ping(Vec::new())).is_err() {
                eprintln!("[ui-ws] failed to queue ping for client {client_id}");
                break;
            }
            last_ping_at = now;
            if awaiting_pong_since.is_none() {
                awaiting_pong_since = Some(now);
            }
        }
    }

    let _ = state.ws_hub.cmd_tx.send(WsHubCommand::UnregisterClient { client_id });
    let _ = outbound_tx.send(WsOutbound::Close);
    let _ = writer_handle.join();
    eprintln!("[ui-ws] websocket loop ended (client_id={client_id})");
    Ok(())
}

fn handle_ws_client_message(
    message: WsClientMessage,
    client_id: u64,
    hub: &WsHubHandle,
    outbound: &Sender<WsOutbound>,
) -> bool {
    match message {
        WsClientMessage::Hello {
            protocol_version,
            client_instance_id,
        } => {
            if protocol_version != UI_PROTOCOL_VERSION {
                let _ = outbound.send(WsOutbound::Message(WsServerMessage::Error {
                    message: format!("protocol mismatch: client={protocol_version}, server={UI_PROTOCOL_VERSION}"),
                    request_id: None,
                }));
            }

            if let Some(client_instance_id) = client_instance_id {
                let Some(client_instance_id) = normalize_ui_client_instance_id(&client_instance_id) else {
                    let _ = outbound.send(WsOutbound::Message(WsServerMessage::Error {
                        message: "client_instance_id is invalid".to_string(),
                        request_id: None,
                    }));
                    return true;
                };

                if !hub
                    .cmd_tx
                    .send(WsHubCommand::BindClientInstance {
                        client_id,
                        client_instance_id,
                    })
                    .is_ok()
                {
                    return false;
                }
            }
            true
        }
        WsClientMessage::Subscribe {
            subscription_id,
            scope,
            from,
        } => {
            if subscription_id.trim().is_empty() {
                let _ = outbound.send(WsOutbound::Message(WsServerMessage::Error {
                    message: "subscription_id cannot be empty".to_string(),
                    request_id: None,
                }));
                return true;
            }

            hub.cmd_tx
                .send(WsHubCommand::Subscribe {
                    client_id,
                    subscription_id,
                    scope,
                    from,
                })
                .is_ok()
        }
        WsClientMessage::Unsubscribe { subscription_id } => hub
            .cmd_tx
            .send(WsHubCommand::Unsubscribe {
                client_id,
                subscription_id,
            })
            .is_ok(),
        WsClientMessage::Intent {
            request_id,
            intent,
            include_self_events,
        } => {
            if request_id.trim().is_empty() {
                let _ = outbound.send(WsOutbound::Message(WsServerMessage::Error {
                    message: "request_id cannot be empty".to_string(),
                    request_id: None,
                }));
                return true;
            }

            hub.cmd_tx
                .send(WsHubCommand::Intent {
                    client_id,
                    request_id,
                    intent,
                    include_self_events,
                })
                .is_ok()
        }
        WsClientMessage::IntentBatch {
            request_id,
            intents,
            include_self_events,
        } => {
            if request_id.trim().is_empty() {
                let _ = outbound.send(WsOutbound::Message(WsServerMessage::Error {
                    message: "request_id cannot be empty".to_string(),
                    request_id: None,
                }));
                return true;
            }
            if intents.is_empty() {
                let _ = outbound.send(WsOutbound::Message(WsServerMessage::Error {
                    message: "intent batch cannot be empty".to_string(),
                    request_id: Some(request_id),
                }));
                return true;
            }

            hub.cmd_tx
                .send(WsHubCommand::IntentBatch {
                    client_id,
                    request_id,
                    intents,
                    include_self_events,
                })
                .is_ok()
        }
    }
}

fn ws_writer_loop(mut stream: TcpStream, outbound_rx: Receiver<WsOutbound>) {
    let _ = stream.set_write_timeout(Some(Duration::from_secs(3)));

    while let Ok(outbound) = outbound_rx.recv() {
        let close_requested = matches!(outbound, WsOutbound::Close);
        let write_result = match outbound {
            WsOutbound::Message(message) => match serde_json::to_string(&message) {
                Ok(text) => write_ws_frame(&mut stream, 0x1, text.as_bytes()),
                Err(err) => Err(Error::new(
                    ErrorKind::InvalidData,
                    format!("failed to serialize websocket message: {err}"),
                )),
            },
            WsOutbound::Ping(payload) => write_ws_frame(&mut stream, 0x9, &payload),
            WsOutbound::Pong(payload) => write_ws_frame(&mut stream, 0xA, &payload),
            WsOutbound::Close => write_ws_frame(&mut stream, 0x8, &[]),
        };

        if write_result.is_err() || close_requested {
            break;
        }
    }

    let _ = stream.shutdown(Shutdown::Both);
}

fn is_websocket_upgrade_request(request: &HttpRequest) -> bool {
    request.method.eq_ignore_ascii_case("GET")
        && header_contains_token(request, "connection", "upgrade")
        && request
            .headers
            .get("upgrade")
            .is_some_and(|value| value.eq_ignore_ascii_case("websocket"))
        && request.headers.contains_key("sec-websocket-key")
}

fn header_contains_token(request: &HttpRequest, header_name: &str, expected_token: &str) -> bool {
    request.headers.get(header_name).is_some_and(|value| {
        value
            .split(',')
            .any(|part| part.trim().eq_ignore_ascii_case(expected_token))
    })
}

fn websocket_accept_key(key: &str) -> String {
    let mut input = Vec::<u8>::new();
    input.extend_from_slice(key.trim().as_bytes());
    input.extend_from_slice(b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
    let digest = sha1_digest(&input);
    base64_encode(&digest)
}

fn sha1_digest(input: &[u8]) -> [u8; 20] {
    let mut h0: u32 = 0x6745_2301;
    let mut h1: u32 = 0xEFCD_AB89;
    let mut h2: u32 = 0x98BA_DCFE;
    let mut h3: u32 = 0x1032_5476;
    let mut h4: u32 = 0xC3D2_E1F0;

    let bit_len = (input.len() as u64) * 8;
    let mut message = input.to_vec();
    message.push(0x80);
    while (message.len() % 64) != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in message.chunks_exact(64) {
        let mut w = [0u32; 80];
        for (idx, word) in w.iter_mut().take(16).enumerate() {
            let base = idx * 4;
            *word = u32::from_be_bytes([chunk[base], chunk[base + 1], chunk[base + 2], chunk[base + 3]]);
        }
        for idx in 16..80 {
            w[idx] = (w[idx - 3] ^ w[idx - 8] ^ w[idx - 14] ^ w[idx - 16]).rotate_left(1);
        }

        let mut a = h0;
        let mut b = h1;
        let mut c = h2;
        let mut d = h3;
        let mut e = h4;

        for (idx, word) in w.iter().enumerate() {
            let (f, k) = match idx {
                0..=19 => ((b & c) | ((!b) & d), 0x5A82_7999),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };

            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
    }

    let mut out = [0u8; 20];
    out[0..4].copy_from_slice(&h0.to_be_bytes());
    out[4..8].copy_from_slice(&h1.to_be_bytes());
    out[8..12].copy_from_slice(&h2.to_be_bytes());
    out[12..16].copy_from_slice(&h3.to_be_bytes());
    out[16..20].copy_from_slice(&h4.to_be_bytes());
    out
}

fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    let mut idx = 0usize;

    while idx + 3 <= data.len() {
        let n = ((data[idx] as u32) << 16) | ((data[idx + 1] as u32) << 8) | data[idx + 2] as u32;
        out.push(TABLE[((n >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3F) as usize] as char);
        out.push(TABLE[((n >> 6) & 0x3F) as usize] as char);
        out.push(TABLE[(n & 0x3F) as usize] as char);
        idx += 3;
    }

    let rem = data.len() - idx;
    if rem == 1 {
        let n = (data[idx] as u32) << 16;
        out.push(TABLE[((n >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3F) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rem == 2 {
        let n = ((data[idx] as u32) << 16) | ((data[idx + 1] as u32) << 8);
        out.push(TABLE[((n >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3F) as usize] as char);
        out.push(TABLE[((n >> 6) & 0x3F) as usize] as char);
        out.push('=');
    }

    out
}

fn read_ws_frame(stream: &mut TcpStream) -> std::io::Result<Option<WsIncomingFrame>> {
    let mut header = [0u8; 2];
    match stream.read_exact(&mut header) {
        Ok(()) => {}
        Err(err) if err.kind() == ErrorKind::UnexpectedEof => return Ok(None),
        Err(err) => return Err(err),
    }

    let opcode = header[0] & 0x0F;
    let masked = (header[1] & 0x80) != 0;
    let mut payload_len = (header[1] & 0x7F) as u64;

    if payload_len == 126 {
        let mut extended = [0u8; 2];
        stream.read_exact(&mut extended)?;
        payload_len = u16::from_be_bytes(extended) as u64;
    } else if payload_len == 127 {
        let mut extended = [0u8; 8];
        stream.read_exact(&mut extended)?;
        payload_len = u64::from_be_bytes(extended);
    }

    if payload_len > WS_MAX_PAYLOAD_BYTES as u64 {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "websocket frame exceeds payload limit",
        ));
    }

    if !masked {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "client websocket frames must be masked",
        ));
    }

    let mut mask = [0u8; 4];
    stream.read_exact(&mut mask)?;

    let mut payload = vec![0u8; payload_len as usize];
    if !payload.is_empty() {
        stream.read_exact(&mut payload)?;
        for (idx, byte) in payload.iter_mut().enumerate() {
            *byte ^= mask[idx % 4];
        }
    }

    match opcode {
        0x1 => {
            let text = std::str::from_utf8(&payload)
                .map_err(|err| Error::new(ErrorKind::InvalidData, format!("invalid websocket text payload: {err}")))?;
            Ok(Some(WsIncomingFrame::Text(text.to_string())))
        }
        0x8 => Ok(Some(WsIncomingFrame::Close)),
        0x9 => Ok(Some(WsIncomingFrame::Ping(payload))),
        0xA => Ok(Some(WsIncomingFrame::Pong)),
        _ => Err(Error::new(
            ErrorKind::InvalidData,
            format!("unsupported websocket opcode: {opcode}"),
        )),
    }
}

fn write_ws_frame(stream: &mut TcpStream, opcode: u8, payload: &[u8]) -> std::io::Result<()> {
    let mut frame = Vec::<u8>::with_capacity(16 + payload.len());
    frame.push(0x80 | (opcode & 0x0F));

    if payload.len() < 126 {
        frame.push(payload.len() as u8);
    } else if payload.len() <= u16::MAX as usize {
        frame.push(126);
        frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    } else {
        frame.push(127);
        frame.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    }

    frame.extend_from_slice(payload);
    stream.write_all(&frame)?;
    stream.flush()?;
    Ok(())
}

fn lock_engine<T: Node>(engine: &Arc<Mutex<Engine<T>>>) -> std::sync::MutexGuard<'_, Engine<T>> {
    match engine.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn read_http_request(stream: &mut TcpStream) -> std::io::Result<HttpRequest> {
    let mut buffer = Vec::<u8>::with_capacity(2048);
    let mut temp = [0u8; 1024];
    let mut header_end = None::<usize>;
    let mut content_length = 0usize;
    let mut parsed_headers = HashMap::<String, String>::new();

    loop {
        let read = stream.read(&mut temp)?;
        if read == 0 {
            if buffer.is_empty() {
                return Err(Error::new(
                    ErrorKind::UnexpectedEof,
                    "peer disconnected before sending a request",
                ));
            }
            break;
        }

        buffer.extend_from_slice(&temp[..read]);
        if buffer.len() > 1024 * 1024 {
            return Err(Error::new(ErrorKind::InvalidData, "request exceeds 1MB"));
        }

        if header_end.is_none() {
            if let Some(idx) = find_header_end(&buffer) {
                header_end = Some(idx + 4);
                let header_text = std::str::from_utf8(&buffer[..idx])
                    .map_err(|err| Error::new(ErrorKind::InvalidData, format!("invalid header utf-8: {err}")))?;
                parsed_headers = parse_headers(header_text)?;
                content_length = parse_content_length(&parsed_headers)?;
            }
        }

        if let Some(end) = header_end {
            if buffer.len() >= end + content_length {
                break;
            }
        }
    }

    let header_end =
        header_end.ok_or_else(|| Error::new(ErrorKind::InvalidData, "malformed request: missing header terminator"))?;
    if buffer.len() < header_end + content_length {
        return Err(Error::new(ErrorKind::UnexpectedEof, "request body truncated"));
    }

    let header_text = std::str::from_utf8(&buffer[..header_end - 4])
        .map_err(|err| Error::new(ErrorKind::InvalidData, format!("invalid header utf-8: {err}")))?;
    let mut lines = header_text.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| Error::new(ErrorKind::InvalidData, "missing request line"))?;

    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| Error::new(ErrorKind::InvalidData, "missing method"))?
        .to_string();
    let target = parts
        .next()
        .ok_or_else(|| Error::new(ErrorKind::InvalidData, "missing request target"))?;

    let path = target.split('?').next().unwrap_or(target).to_string();
    let body = buffer[header_end..header_end + content_length].to_vec();

    Ok(HttpRequest {
        method,
        path,
        headers: parsed_headers,
        body,
    })
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|chunk| chunk == b"\r\n\r\n")
}

fn parse_headers(raw_headers: &str) -> std::io::Result<HashMap<String, String>> {
    let mut lines = raw_headers.lines();
    let _request_line = lines
        .next()
        .ok_or_else(|| Error::new(ErrorKind::InvalidData, "missing request line"))?;

    let mut headers = HashMap::<String, String>::new();
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| Error::new(ErrorKind::InvalidData, "malformed header line"))?;
        let key = name.trim().to_ascii_lowercase();
        let value = value.trim();
        headers
            .entry(key)
            .and_modify(|existing| {
                existing.push(',');
                existing.push_str(value);
            })
            .or_insert_with(|| value.to_string());
    }
    Ok(headers)
}

fn parse_content_length(headers: &HashMap<String, String>) -> std::io::Result<usize> {
    let Some(value) = headers.get("content-length") else {
        return Ok(0);
    };

    value
        .trim()
        .parse::<usize>()
        .map_err(|err| Error::new(ErrorKind::InvalidData, format!("invalid content-length: {err}")))
}

fn write_json<T: serde::Serialize>(stream: &mut TcpStream, status: &str, payload: &T) -> std::io::Result<()> {
    let body = serde_json::to_vec(payload)
        .map_err(|err| Error::new(ErrorKind::InvalidData, format!("failed to serialize json: {err}")))?;
    write_response(stream, status, "application/json; charset=utf-8", &body)
}

fn write_json_error(stream: &mut TcpStream, status: &str, message: &str) -> std::io::Result<()> {
    write_json(stream, status, &serde_json::json!({ "error": message }))
}

fn write_response(stream: &mut TcpStream, status: &str, content_type: &str, body: &[u8]) -> std::io::Result<()> {
    let headers = format!(
        "HTTP/1.1 {status}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
         Access-Control-Allow-Headers: Content-Type, X-GC-UI-Client-Instance\r\n\
         Connection: close\r\n\r\n",
        body.len()
    );

    stream.write_all(headers.as_bytes())?;
    if !body.is_empty() {
        stream.write_all(body)?;
    }
    stream.flush()?;
    Ok(())
}

fn is_client_disconnect_error(err: &Error) -> bool {
    matches!(
        err.kind(),
        ErrorKind::UnexpectedEof
            | ErrorKind::ConnectionReset
            | ErrorKind::ConnectionAborted
            | ErrorKind::BrokenPipe
            | ErrorKind::NotConnected
    )
}