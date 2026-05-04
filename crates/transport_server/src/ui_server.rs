use std::collections::HashMap;
use std::io::{Error, ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures_util::{SinkExt, StreamExt};
use golden_engine::app::{ProjectLifecycle, prepare_engine_for_runtime};
use golden_engine::engine::{Engine, EngineTime};
use golden_engine::node::Node;
use golden_protocol::{
    UI_PROTOCOL_VERSION, UiAck, UiAckStatus, UiContextCandidatesRequest as ContextCandidatesRequest, UiEditIntent,
    UiEventBatch, UiEventDto, UiEventKind, UiParamControlInfoRequest as ParamControlInfoRequest,
    UiProjectPathDto as ProjectPathDto, UiProjectPathRequest as ProjectPathRequest,
    UiProjectUploadRequest as ProjectUploadRequest, UiReferenceTargetsRequest as ReferenceTargetsRequest,
    UiReplayRequest as ReplayRequest, UiScriptConfigRequest as ScriptConfigRequest,
    UiScriptReloadRequest as ScriptReloadRequest, UiScriptStateRequest as ScriptStateRequest,
    UiSnapshotRequest as SnapshotRequest, UiSubscriptionScope,
};
use serde::{Deserialize, Serialize};
use tokio::runtime::{Builder, Runtime};
use tokio::time::timeout;
use tokio_tungstenite::{
    WebSocketStream,
    tungstenite::{
        handshake::server::{Request as WsRequest, create_response, write_response as write_ws_handshake_response},
        http::{HeaderValue, Method, Version},
        protocol::{Message, Role, WebSocketConfig},
    },
};

use crate::project_host;

const HTTP_MAX_REQUEST_BYTES: usize = 64 * 1024 * 1024;
const WS_MAX_PAYLOAD_BYTES: usize = 1024 * 1024;
const WS_RETRY_INTERVAL: Duration = Duration::from_millis(16);
const WS_IO_POLL_INTERVAL: Duration = Duration::from_millis(5);
const WS_PING_INTERVAL: Duration = Duration::from_secs(10);
const WS_PONG_TIMEOUT: Duration = Duration::from_secs(30);

static NEXT_WS_CLIENT_ID: AtomicU64 = AtomicU64::new(1);

type UiWebSocketStream = WebSocketStream<tokio::net::TcpStream>;

#[derive(Debug, Clone, Copy)]
/// A bundled frontend asset served by the built-in UI host.
pub struct UiAsset {
    /// Absolute HTTP path where the asset is served, such as `/index.html` or `/_app/app.js`.
    pub path: &'static str,
    /// Response content type sent when the asset is served.
    pub content_type: &'static str,
    /// Embedded file contents.
    pub bytes: &'static [u8],
}

#[derive(Clone)]
/// Runtime settings for the built-in HTTP and WebSocket UI server.
pub struct UiServerConfig {
    /// Socket address where the built-in UI server listens.
    pub bind_addr: String,
    /// Runtime tick interval used by the engine loop hosted by the UI server.
    pub tick_interval: Duration,
    /// Optional bundled frontend assets served from non-API routes.
    pub frontend_assets: &'static [UiAsset],
}

impl Default for UiServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "localhost:7010".to_string(),
            tick_interval: Duration::from_millis(16),
            frontend_assets: &[],
        }
    }
}

struct ServerState<T: ProjectLifecycle> {
    engine: Arc<Mutex<Engine<T>>>,
    ws_hub: WsHubHandle,
    frontend_assets: &'static [UiAsset],
}

impl<T: ProjectLifecycle> Clone for ServerState<T> {
    fn clone(&self) -> Self {
        Self {
            engine: self.engine.clone(),
            ws_hub: self.ws_hub.clone(),
            frontend_assets: self.frontend_assets,
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

enum UiWebSocketPoll {
    Message(Message),
    Closed,
    Idle,
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
    println!(
        "UI host listening on http://{} (bundled_frontend={})",
        config.bind_addr,
        !config.frontend_assets.is_empty()
    );

    let state = ServerState {
        engine,
        ws_hub,
        frontend_assets: config.frontend_assets,
    };

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
                report_runtime_tick_failure(&err);
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

fn report_runtime_tick_failure(err: &impl std::fmt::Display) {
    eprintln!("runtime tick failed: {err}");

    #[cfg(debug_assertions)]
    {
        eprintln!("debug build aborting process after runtime tick failure");
        let _ = std::io::stderr().flush();
        std::process::abort();
    }
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

            if let Err(write_err) = write_json_error(stream, "400 Bad Request", &format!("invalid request: {err}")) {
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

    if request.method.eq_ignore_ascii_case("GET") {
        if let Some(asset) = resolve_frontend_asset(state.frontend_assets, &request.path) {
            write_response(stream, "200 OK", asset.content_type, asset.bytes)?;
            return Ok(());
        }
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
            let mut snapshot = guard.ui_snapshot(scope.clone());
            snapshot.project_file = T::project_file_spec().into();
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
        ("POST", "/api/ui/project-upload-load") => {
            let payload: ProjectUploadRequest = serde_json::from_slice(&request.body).map_err(|err| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!("invalid project-upload-load payload: {err}"),
                )
            })?;
            match project_host::upload_project_and_load(&state.engine, &payload.file_name, &payload.contents) {
                Ok(path) => {
                    write_json(stream, "200 OK", &ProjectPathDto { path })?;
                }
                Err(err) => {
                    write_json_error(stream, "400 Bad Request", &format!("project-upload-load failed: {err}"))?;
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

fn resolve_frontend_asset<'a>(assets: &'a [UiAsset], request_path: &str) -> Option<&'a UiAsset> {
    if assets.is_empty() || request_path.starts_with("/api/") {
        return None;
    }

    let normalized_path = normalize_frontend_asset_path(request_path);
    if let Some(asset) = assets.iter().find(|asset| asset.path == normalized_path) {
        return Some(asset);
    }

    if looks_like_asset_path(normalized_path) {
        return None;
    }

    assets.iter().find(|asset| asset.path == "/index.html")
}

fn normalize_frontend_asset_path(request_path: &str) -> &str {
    match request_path {
        "" | "/" => "/index.html",
        path => path,
    }
}

fn looks_like_asset_path(request_path: &str) -> bool {
    request_path
        .rsplit('/')
        .next()
        .is_some_and(|segment| segment.contains('.'))
}

fn handle_ws_connection<T: ProjectLifecycle>(
    stream: &mut TcpStream,
    state: &ServerState<T>,
    request: &HttpRequest,
) -> std::io::Result<()> {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| Error::new(ErrorKind::Other, format!("failed to build websocket runtime: {error}")))?;

    let ws_request = build_websocket_request(request)?;
    let mut response = match create_response(&ws_request) {
        Ok(response) => response,
        Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
            write_ws_handshake_response(&mut *stream, &response).map_err(|error| {
                Error::new(
                    ErrorKind::Other,
                    format!("failed to write websocket handshake rejection: {error}"),
                )
            })?;
            stream.flush()?;
            return Ok(());
        }
        Err(error) => {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("invalid websocket upgrade request: {error}"),
            ));
        }
    };
    response
        .headers_mut()
        .insert("Access-Control-Allow-Origin", HeaderValue::from_static("*"));
    write_ws_handshake_response(&mut *stream, &response).map_err(|error| {
        Error::new(
            ErrorKind::Other,
            format!("failed to write websocket handshake response: {error}"),
        )
    })?;
    stream.flush()?;
    stream.set_read_timeout(None)?;
    stream.set_write_timeout(None)?;
    stream.set_nonblocking(true)?;

    let tokio_stream = {
        let cloned_stream = stream.try_clone()?;
        let _guard = runtime.enter();
        tokio::net::TcpStream::from_std(cloned_stream).map_err(|error| {
            Error::new(
                ErrorKind::Other,
                format!("failed to convert websocket stream to tokio stream: {error}"),
            )
        })?
    };
    let mut websocket = runtime.block_on(WebSocketStream::from_raw_socket(
        tokio_stream,
        Role::Server,
        Some(ui_websocket_config()),
    ));

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

    let mut last_ping_at = Instant::now();
    let mut awaiting_pong_since = None::<Instant>;

    'connection: loop {
        loop {
            match outbound_rx.try_recv() {
                Ok(outbound) => match send_ws_outbound(&runtime, &mut websocket, outbound) {
                    Ok(true) => break 'connection,
                    Ok(false) => {}
                    Err(err) => {
                        eprintln!("websocket write failed for client {client_id}: {err}");
                        break 'connection;
                    }
                },
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break 'connection,
            }
        }

        match poll_ws_message(&runtime, &mut websocket, WS_IO_POLL_INTERVAL) {
            Ok(UiWebSocketPoll::Idle) => {}
            Ok(UiWebSocketPoll::Closed) => break,
            Ok(UiWebSocketPoll::Message(message)) => match message {
                Message::Text(text) => {
                    awaiting_pong_since = None;
                    let message: WsClientMessage = match serde_json::from_str(text.as_ref()) {
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
                Message::Binary(_) => {
                    let _ = outbound_tx.send(WsOutbound::Message(WsServerMessage::Error {
                        message: "binary websocket messages are not supported".to_string(),
                        request_id: None,
                    }));
                }
                Message::Ping(_) => {
                    awaiting_pong_since = None;
                    if let Err(err) = flush_ws_stream(&runtime, &mut websocket) {
                        eprintln!("websocket flush failed for client {client_id}: {err}");
                        break;
                    }
                }
                Message::Pong(_) => {
                    awaiting_pong_since = None;
                }
                Message::Close(_) => break,
                Message::Frame(_) => {}
            },
            Err(err) => {
                eprintln!("websocket read failed for client {client_id}: {err}");
                break;
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
            match send_ws_outbound(&runtime, &mut websocket, WsOutbound::Ping(Vec::new())) {
                Ok(true) => break,
                Ok(false) => {}
                Err(err) => {
                    eprintln!("[ui-ws] failed to send ping for client {client_id}: {err}");
                    break;
                }
            }
            last_ping_at = now;
            if awaiting_pong_since.is_none() {
                awaiting_pong_since = Some(now);
            }
        }
    }

    let _ = state.ws_hub.cmd_tx.send(WsHubCommand::UnregisterClient { client_id });
    let _ = runtime.block_on(websocket.close(None));
    eprintln!("[ui-ws] websocket loop ended (client_id={client_id})");
    Ok(())
}

fn send_ws_outbound(
    runtime: &Runtime,
    websocket: &mut UiWebSocketStream,
    outbound: WsOutbound,
) -> std::io::Result<bool> {
    let close_requested = matches!(outbound, WsOutbound::Close);
    let message = match outbound {
        WsOutbound::Message(message) => {
            let text = serde_json::to_string(&message).map_err(|err| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!("failed to serialize websocket message: {err}"),
                )
            })?;
            Message::text(text)
        }
        WsOutbound::Ping(payload) => Message::Ping(payload.into()),
        WsOutbound::Close => Message::Close(None),
    };

    runtime
        .block_on(async { websocket.send(message).await })
        .map_err(|error| Error::new(ErrorKind::Other, format!("failed to write websocket frame: {error}")))?;
    Ok(close_requested)
}

fn poll_ws_message(
    runtime: &Runtime,
    websocket: &mut UiWebSocketStream,
    poll_interval: Duration,
) -> std::io::Result<UiWebSocketPoll> {
    match runtime.block_on(async { timeout(poll_interval, websocket.next()).await }) {
        Err(_) => Ok(UiWebSocketPoll::Idle),
        Ok(None) => Ok(UiWebSocketPoll::Closed),
        Ok(Some(Ok(message))) => Ok(UiWebSocketPoll::Message(message)),
        Ok(Some(Err(error))) => Err(Error::new(ErrorKind::Other, format!("websocket read failed: {error}"))),
    }
}

fn flush_ws_stream(runtime: &Runtime, websocket: &mut UiWebSocketStream) -> std::io::Result<()> {
    runtime
        .block_on(async { websocket.flush().await })
        .map_err(|error| Error::new(ErrorKind::Other, format!("failed to flush websocket stream: {error}")))
}

fn build_websocket_request(request: &HttpRequest) -> std::io::Result<WsRequest> {
    let mut builder = WsRequest::builder()
        .method(Method::GET)
        .uri(request.path.as_str())
        .version(Version::HTTP_11);

    for (name, value) in &request.headers {
        builder = builder.header(name.as_str(), value.as_str());
    }

    builder
        .body(())
        .map_err(|error| Error::new(ErrorKind::InvalidData, format!("invalid websocket request: {error}")))
}

fn ui_websocket_config() -> WebSocketConfig {
    WebSocketConfig::default()
        .max_message_size(Some(WS_MAX_PAYLOAD_BYTES))
        .max_frame_size(Some(WS_MAX_PAYLOAD_BYTES))
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
        if buffer.len() > HTTP_MAX_REQUEST_BYTES {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("request exceeds {} bytes", HTTP_MAX_REQUEST_BYTES),
            ));
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
