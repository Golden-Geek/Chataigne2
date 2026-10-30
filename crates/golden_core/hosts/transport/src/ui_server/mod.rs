use std::collections::HashMap;
use std::io::{Error, ErrorKind, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures_util::{SinkExt, StreamExt};
use golden_engine::app::{ProjectGeneration, ProjectLifecycle, apply_preferences_runtime_limits};
use golden_engine::application::{ApplicationError, AppliedUiTransaction, ProductionRuntime, UiClientLease};
use golden_engine::engine::{Engine, EngineTime};
use golden_engine::events::CustomEventRetention;
use golden_engine::ui_read_model::UiReadModel;
use golden_protocol::{
    UI_PROTOCOL_VERSION, UiAck, UiApplicationInfoDto, UiClientMessage as WsClientMessage,
    UiContextCandidatesRequest as ContextCandidatesRequest, UiControlPhase, UiControlUpdate, UiDataPlane,
    UiDiscoveryDto, UiEditIntent, UiEventBatch, UiEventDto, UiEventKind, UiInterest,
    UiParamControlInfoRequest as ParamControlInfoRequest, UiPlaneDelta, UiProjectFileSpec,
    UiProjectLoadRecoveryDto as ProjectLoadRecoveryDto, UiProjectPathDto as ProjectPathDto,
    UiProjectPathRequest as ProjectPathRequest, UiProjectUploadRequest as ProjectUploadRequest,
    UiReferenceTargetsRequest as ReferenceTargetsRequest, UiReplayCursor, UiReplayRequest as ReplayRequest,
    UiRuntimeStatsDto, UiScriptConfigRequest as ScriptConfigRequest, UiScriptReloadRequest as ScriptReloadRequest,
    UiScriptStateRequest as ScriptStateRequest, UiServerMessage as WsServerMessage,
    UiSnapshotRequest as SnapshotRequest, UiSubscriptionScope,
};
use serde::{Deserialize, Serialize};
use tokio::runtime::{Builder, Runtime};
use tokio::time::timeout;
use tokio_tungstenite::{
    WebSocketStream,
    tungstenite::{
        handshake::server::{Request as WsRequest, create_response, write_response as write_ws_handshake_response},
        http::{Method, Version},
        protocol::{Message, Role, WebSocketConfig},
    },
};

use crate::project_host;

mod access;
mod admission;
mod application_error;
mod content_security;
mod delivery;
mod http;
mod hub;
mod session;
mod startup;
mod websocket;

use application_error::*;
use content_security::FrontendSecurity;
use delivery::*;
use http::*;
use hub::*;
use session::{Capability, SessionGrant, SessionRegistry};
use websocket::*;

mod outbound_queue;
mod pending_value_events;
mod runtime_pacer;
mod snapshot_encoding;
#[cfg(test)]
mod tests;

use access::{RequestAccessPolicy, validate_preflight};
use admission::ConnectionLimiter;
use outbound_queue::{DEFAULT_OUTBOUND_CAPACITY, QueuePushResult, WsOutboundQueue};
use pending_value_events::PendingValueEvents;
use runtime_pacer::RuntimeLoopPacer;
use snapshot_encoding::{
    PendingWsSnapshots, SnapshotEncodingResult, SnapshotEncodingService, WsSnapshotCompletion,
    encode_websocket_message, receive_encoded_snapshot,
};

const MAX_CONNECTIONS: usize = 16;
const HTTP_MAX_HEADER_BYTES: usize = 32 * 1024;
const HTTP_MAX_REQUEST_BYTES: usize = 16 * 1024 * 1024;
const HTTP_IO_TIMEOUT: Duration = Duration::from_secs(3);
const WS_MAX_PAYLOAD_BYTES: usize = 1024 * 1024;
const WS_HUB_COMMAND_CAPACITY: usize = 64;
const WS_HUB_SERVICE_BUDGET: usize = 32;
const MAX_WS_CLIENTS: usize = MAX_CONNECTIONS;
const MAX_SUBSCRIPTIONS_PER_CLIENT: usize = 32;
const MAX_UI_INTENTS_PER_BATCH: usize = 256;
const WS_RETRY_INTERVAL: Duration = Duration::from_millis(16);
const DEFAULT_WS_VALUE_FLUSH_INTERVAL: Duration = Duration::from_millis(16);
const WS_IO_POLL_INTERVAL: Duration = Duration::from_millis(5);
const WS_PING_INTERVAL: Duration = Duration::from_secs(10);
const WS_PONG_TIMEOUT: Duration = Duration::from_secs(30);
const ENGINE_STATS_INTERVAL: Duration = Duration::from_millis(250);
const UI_WEBSOCKET_PATH: &str = "/api/ui/ws";

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
/// Host-managed Preferences persistence settings.
pub struct UiPreferencesConfig {
    /// Absolute path to the app-data Preferences JSON file.
    pub file_path: PathBuf,
    /// Default app data folder stored in Preferences when no value exists yet.
    pub default_data_folder: String,
}

#[derive(Clone)]
/// Runtime settings for the built-in HTTP and WebSocket UI server.
pub struct UiServerConfig {
    /// Socket address where the built-in UI server listens.
    pub bind_addr: String,
    /// Explicit browser origins allowed to call this host across origins (for example a dev frontend).
    pub allowed_origins: Vec<String>,
    /// Additional exact Host authorities, including their listener port, for trusted DNS/proxy names.
    /// Direct listener IPs and loopback localhost are admitted automatically; wildcard names are not.
    pub allowed_hosts: Vec<String>,
    /// Maximum capabilities granted by password-free bootstrap on this trusted listener.
    pub session_capabilities: golden_protocol::UiSessionCapabilities,
    /// Runtime tick interval used by the engine loop hosted by the UI server.
    pub tick_interval: Duration,
    /// Minimum interval between value-only WebSocket batches.
    pub value_flush_interval: Duration,
    /// Optional bundled frontend assets served from non-API routes.
    pub frontend_assets: &'static [UiAsset],
    /// Optional app-data Preferences persistence.
    pub preferences: Option<UiPreferencesConfig>,
}

/// Versioned readiness snapshot for the built-in UI host and its active UI sessions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiReadinessDto {
    /// Schema version for this HTTP response.
    pub version: u16,
    /// Whether the HTTP backend has completed listener startup.
    pub backend_ready: bool,
    /// Whether the immutable engine/UI read model is available.
    pub engine_read_model_ready: bool,
    /// Number of currently connected UI WebSocket clients.
    pub active_websocket_clients: u64,
    /// Number of connected UI clients with at least one active subscription.
    pub active_subscribed_websocket_clients: u64,
    /// Revision currently published by the immutable UI read model.
    pub read_model_revision: EngineTime,
}

fn ui_discovery_document<T: ProjectLifecycle>() -> UiDiscoveryDto {
    UiDiscoveryDto {
        version: 1,
        service: "golden-ui".to_string(),
        health_path: "/api/ui/health".to_string(),
        websocket_path: UI_WEBSOCKET_PATH.to_string(),
        relative_endpoints: true,
        application: UiApplicationInfoDto {
            id: T::application_id().to_string(),
            display_name: T::application_display_name().to_string(),
            version: T::application_version().to_string(),
            data_directory_name: T::app_data_directory_name().to_string(),
        },
        protocol_version: UI_PROTOCOL_VERSION.to_string(),
    }
}

impl Default for UiServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:7010".to_string(),
            allowed_origins: Vec::new(),
            allowed_hosts: Vec::new(),
            session_capabilities: golden_protocol::UiSessionCapabilities::ALL,
            tick_interval: Duration::from_millis(16),
            value_flush_interval: DEFAULT_WS_VALUE_FLUSH_INTERVAL,
            frontend_assets: &[],
            preferences: None,
        }
    }
}

struct ServerState<T: ProjectLifecycle> {
    frontend_security: Arc<FrontendSecurity>,
    sessions: Arc<SessionRegistry>,
    request_access: Arc<RequestAccessPolicy>,
    runtime: ProductionRuntime<T>,
    read_model: Arc<UiReadModel>,
    ws_hub: WsHubHandle,
    snapshot_encoding: SnapshotEncodingService,
    frontend_assets: &'static [UiAsset],
    preferences: Option<UiPreferencesConfig>,
}

impl<T: ProjectLifecycle> Clone for ServerState<T> {
    fn clone(&self) -> Self {
        Self {
            frontend_security: self.frontend_security.clone(),
            sessions: self.sessions.clone(),
            request_access: self.request_access.clone(),
            runtime: self.runtime.clone(),
            read_model: self.read_model.clone(),
            ws_hub: self.ws_hub.clone(),
            snapshot_encoding: self.snapshot_encoding.clone(),
            frontend_assets: self.frontend_assets,
            preferences: self.preferences.clone(),
        }
    }
}

fn ui_intent_kind(intent: &UiEditIntent) -> &'static str {
    match intent {
        UiEditIntent::BeginEdit { .. } => "begin_edit",
        UiEditIntent::EndEdit { .. } => "end_edit",
        UiEditIntent::SetParam { .. } => "set_param",
        UiEditIntent::SetTextParamSmart { .. } => "set_text_param_smart",
        UiEditIntent::SetParamControlState { .. } => "set_param_control_state",
        UiEditIntent::SetParamConstraints { .. } => "set_param_constraints",
        UiEditIntent::MoveNode { .. } => "move",
        UiEditIntent::MoveDashboardWidget { .. } => "move_dashboard_widget",
        UiEditIntent::ResizeDashboardWidget { .. } => "resize_dashboard_widget",
        UiEditIntent::RemoveNode { .. } => "remove",
        UiEditIntent::RemoveNodes { .. } => "remove_nodes",
        UiEditIntent::CreateUserItem { .. } => "create_user_item",
        UiEditIntent::CreateDashboardContainerWidget { .. } => "create_dashboard_container_widget",
        UiEditIntent::CreateDashboardNodeWidget { .. } => "create_dashboard_node_widget",
        UiEditIntent::CreateDashboardGenericWidget { .. } => "create_dashboard_generic_widget",
        UiEditIntent::BindDashboardNodeWidgetTarget { .. } => "bind_dashboard_node_widget_target",
        UiEditIntent::BindDashboardGenericWidgetTarget { .. } => "bind_dashboard_generic_widget_target",
        UiEditIntent::WrapDashboardWidgetInContainer { .. } => "wrap_dashboard_widget_in_container",
        UiEditIntent::DuplicateNode { .. } => "duplicate",
        UiEditIntent::DuplicateNodes { .. } => "duplicate_nodes",
        UiEditIntent::FitAnimationCurvePath { .. } => "fit_animation_curve_path",
        UiEditIntent::PatchMeta { .. } => "rename",
        UiEditIntent::EnsureUserContextScope { .. } => "ensure_user_context_scope",
        UiEditIntent::RemoveUserContextScope { .. } => "remove_user_context_scope",
        UiEditIntent::UpsertUserContextEntry { .. } => "upsert_user_context_entry",
        UiEditIntent::RemoveUserContextEntry { .. } => "remove_user_context_entry",
        UiEditIntent::SendNodeEvent { .. } => "send_node_event",
        UiEditIntent::InvokeAppOperation { .. } => "invoke_app_operation",
        UiEditIntent::ReevaluateGraph => "reevaluate_graph",
        UiEditIntent::ClearLogs => "clear_logs",
        UiEditIntent::SetLogMaxEntries { .. } => "set_log_max_entries",
        UiEditIntent::Undo => "undo",
        UiEditIntent::Redo => "redo",
    }
}

struct UiIntentTiming {
    kind: &'static str,
    lock_wait_ms: u128,
    apply_ms: u128,
    event_collect_ms: u128,
    events: usize,
    requires_resync: bool,
    total_ms: u128,
}

fn ui_intent_timing_from_application(kind: &'static str, result: &AppliedUiTransaction) -> UiIntentTiming {
    UiIntentTiming {
        kind,
        lock_wait_ms: result.timing.lock_wait.as_millis(),
        apply_ms: result.timing.apply.as_millis(),
        event_collect_ms: result.timing.event_collect.as_millis(),
        events: result.events.events.len(),
        requires_resync: result.events.events.iter().any(
            |event| matches!(&event.kind, UiEventKind::Custom { topic, .. } if topic == "__transport.resync_required"),
        ),
        total_ms: result.timing.total.as_millis(),
    }
}

fn log_ui_intent_timing(channel: &str, timing: &UiIntentTiming) {
    eprintln!(
        "[{channel}] intent kind={} lock_wait_ms={} apply_ms={} event_collect_ms={} events={} requires_resync={} total_ms={}",
        timing.kind,
        timing.lock_wait_ms,
        timing.apply_ms,
        timing.event_collect_ms,
        timing.events,
        timing.requires_resync,
        timing.total_ms
    );
}

fn save_preferences_after_change<T: ProjectLifecycle>(
    runtime: &ProductionRuntime<T>,
    preferences: Option<&UiPreferencesConfig>,
    preferences_changed: bool,
) {
    if !preferences_changed {
        return;
    }
    let Some(preferences) = preferences else {
        return;
    };
    if let Err(err) = project_host::save_preferences(runtime, preferences) {
        eprintln!("[preferences] failed to save after edit: {err}");
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
    cmd_tx: SyncSender<WsHubCommand>,
    readiness: Arc<UiSessionReadiness>,
}

struct WsHubContext<T: ProjectLifecycle> {
    runtime: ProductionRuntime<T>,
    read_model: Arc<UiReadModel>,
    preferences: Option<UiPreferencesConfig>,
    session_id: String,
    readiness: Arc<UiSessionReadiness>,
    snapshot_encoding: SnapshotEncodingService,
}

#[derive(Default)]
struct UiSessionReadiness {
    active_websocket_clients: AtomicU64,
    active_subscribed_websocket_clients: AtomicU64,
}

impl UiSessionReadiness {
    fn update(&self, clients: &HashMap<u64, WsClientState>) {
        self.active_websocket_clients
            .store(clients.len() as u64, Ordering::Release);
        self.active_subscribed_websocket_clients.store(
            clients
                .values()
                .filter(|client| !client.subscriptions.is_empty())
                .count() as u64,
            Ordering::Release,
        );
    }

    fn snapshot(&self, read_model: &UiReadModel) -> UiReadinessDto {
        UiReadinessDto {
            version: 1,
            backend_ready: true,
            engine_read_model_ready: true,
            active_websocket_clients: self.active_websocket_clients.load(Ordering::Acquire),
            active_subscribed_websocket_clients: self.active_subscribed_websocket_clients.load(Ordering::Acquire),
            read_model_revision: read_model.current_revision(),
        }
    }
}

struct HttpRequest {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

struct WsClientState {
    outbound: Arc<WsOutboundQueue>,
    subscriptions: HashMap<String, WsSubscriptionState>,
    connection: UiClientLease,
}

impl Drop for WsClientState {
    fn drop(&mut self) {
        self.connection.close();
    }
}

struct WsSubscriptionState {
    interest: UiInterest,
    project_generation: ProjectGeneration,
    cursor: Option<EngineTime>,
    last_runtime_stats: Option<UiRuntimeStatsDto>,
    pending_value_events: PendingValueEvents,
    awaiting_resync: bool,
}

impl WsSubscriptionState {
    fn queue_value_events(&mut self, from: Option<EngineTime>, events: Vec<UiEventDto>) {
        self.pending_value_events.queue(from, events);
    }

    fn take_value_batch(&mut self) -> Option<UiEventBatch> {
        self.pending_value_events.take_batch()
    }

    fn clear_pending_value_events(&mut self) {
        self.pending_value_events.clear();
    }
}

struct WsEventOrigin {
    project_generation: ProjectGeneration,
    client_id: u64,
    include_self_events: bool,
}

enum WsHubCommand {
    RegisterClient {
        client_id: u64,
        outbound: Arc<WsOutboundQueue>,
        connection: UiClientLease,
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
        interest: UiInterest,
        from: Option<UiReplayCursor>,
    },
    Unsubscribe {
        client_id: u64,
        subscription_id: String,
    },
    Snapshot {
        client_id: u64,
        request_id: String,
        scope: UiSubscriptionScope,
    },
    Replay {
        client_id: u64,
        request_id: String,
        scope: UiSubscriptionScope,
        from: Option<UiReplayCursor>,
    },
    Intent {
        client_id: u64,
        request_id: String,
        intent: Box<UiEditIntent>,
        include_self_events: bool,
    },
    IntentBatch {
        client_id: u64,
        request_id: String,
        intents: Vec<UiEditIntent>,
        include_self_events: bool,
    },
}

// The queue is bounded by both entry count and retained serialized bytes. Keeping messages inline
// avoids an extra heap allocation on every high-rate value/preview update.
#[derive(Clone)]
#[allow(clippy::large_enum_variant)]
enum WsOutbound {
    Message(WsServerMessage),
    Snapshot {
        request_id: String,
        snapshot: SnapshotEncodingResult,
    },
    Ping(Vec<u8>),
    Close,
}

enum UiWebSocketPoll {
    Message(Message),
    Closed,
    Idle,
}

/// Prepares an engine and runs it through the built-in HTTP and WebSocket host.
pub fn run_with_ui_server_config<T: ProjectLifecycle + 'static>(
    engine: Engine<T>,
    config: UiServerConfig,
) -> std::io::Result<()> {
    run_with_ui_server_config_and_ready(engine, config, |_| {})
}

/// Prepares the authoritative runtime and exposes its facade once the listener is bound.
/// The callback only wires host services; it must not block or start another engine.
pub fn run_with_ui_server_config_and_ready<T: ProjectLifecycle + 'static>(
    mut engine: Engine<T>,
    config: UiServerConfig,
    ready: impl FnOnce(ProductionRuntime<T>),
) -> std::io::Result<()> {
    project_host::load_preferences_into_engine(&mut engine, config.preferences.as_ref()).map_err(Error::other)?;
    startup::prepare_engine_for_host_startup(&mut engine)?;
    apply_preferences_runtime_limits(&mut engine);
    run_ui_server_with_ready(engine, config, ready)
}

/// Runs the built-in HTTP and WebSocket host through the production application facade.
pub fn run_ui_server<T: ProjectLifecycle + 'static>(engine: Engine<T>, config: UiServerConfig) -> std::io::Result<()> {
    run_ui_server_with_ready(engine, config, |_| {})
}

fn run_ui_server_with_ready<T: ProjectLifecycle + 'static>(
    engine: Engine<T>,
    config: UiServerConfig,
    ready: impl FnOnce(ProductionRuntime<T>),
) -> std::io::Result<()> {
    let frontend_security = Arc::new(FrontendSecurity::prepare(config.frontend_assets)?);
    let request_access = Arc::new(RequestAccessPolicy::new(
        &config.allowed_origins,
        &config.allowed_hosts,
    )?);
    let project_file = UiProjectFileSpec::from_project_file_spec(T::project_file_spec(), None);
    let runtime = ProductionRuntime::new(engine, project_file).map_err(Error::other)?;
    if let Some(preferences) = config.preferences.as_ref() {
        project_host::save_preferences(&runtime, preferences).map_err(Error::other)?;
    }
    let read_model = runtime.read_model();
    let snapshot_encoding = SnapshotEncodingService::spawn()?;
    spawn_runtime_loop(runtime.clone(), read_model.clone());
    let ws_hub = spawn_ws_hub(
        runtime.clone(),
        read_model.clone(),
        config.tick_interval.max(WS_RETRY_INTERVAL),
        config.value_flush_interval.max(Duration::from_millis(1)),
        config.preferences.clone(),
        make_server_session_id(),
        snapshot_encoding.clone(),
    );

    let listener = TcpListener::bind(&config.bind_addr)?;
    ready(runtime.clone());
    println!(
        "UI host listening on http://{} (bundled_frontend={})",
        config.bind_addr,
        !config.frontend_assets.is_empty()
    );

    let state = ServerState {
        frontend_security,
        sessions: Arc::new(SessionRegistry::new(config.session_capabilities)),
        request_access,
        runtime,
        read_model,
        ws_hub,
        snapshot_encoding,
        frontend_assets: config.frontend_assets,
        preferences: config.preferences,
    };

    let connection_limiter = Arc::new(ConnectionLimiter::new(MAX_CONNECTIONS));
    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let Some(connection_permit) = connection_limiter.try_acquire() else {
                    reject_overloaded_connection(&mut stream);
                    continue;
                };
                let state = state.clone();
                let spawn_result = thread::Builder::new()
                    .name("golden-ui-connection".to_string())
                    .spawn(move || {
                        let _connection_permit = connection_permit;
                        if let Err(err) = handle_connection(&mut stream, &state) {
                            eprintln!("ui server request failed: {err}");
                        }
                    });
                if let Err(err) = spawn_result {
                    eprintln!("failed to spawn UI connection worker: {err}");
                }
            }
            Err(err) => {
                eprintln!("ui server accept failed: {err}");
            }
        }
    }

    Ok(())
}

fn reject_overloaded_connection(stream: &mut TcpStream) {
    let _ = stream.set_write_timeout(Some(Duration::from_millis(100)));
    let _ = write_json_error(
        &mut HttpResponse::new(stream),
        "503 Service Unavailable",
        "UI host connection capacity is exhausted; retry later",
    );
    let _ = stream.shutdown(Shutdown::Both);
}

fn spawn_runtime_loop<T: ProjectLifecycle + 'static>(runtime: ProductionRuntime<T>, read_model: Arc<UiReadModel>) {
    thread::spawn(move || {
        let mut pacer = RuntimeLoopPacer::new();
        let mut stats_window_start = Instant::now();
        let mut stats_window_ticks = 0u64;

        loop {
            let tick = pacer.begin_tick();

            let tick_interval = match runtime.run_tick(tick.elapsed) {
                Ok(result) => result.next_interval,
                Err(err) => {
                    report_runtime_tick_failure(&err);
                    Duration::from_millis(1)
                }
            };

            stats_window_ticks = stats_window_ticks.saturating_add(1);
            let stats_elapsed = stats_window_start.elapsed();
            if stats_elapsed >= ENGINE_STATS_INTERVAL {
                let elapsed_secs = stats_elapsed.as_secs_f64();
                if elapsed_secs > 0.0 {
                    let metrics = runtime.runtime_metrics();
                    read_model.set_runtime_stats(UiRuntimeStatsDto {
                        engine_hz: stats_window_ticks as f64 / elapsed_secs,
                        generation_id: metrics.generation_id,
                        control_queue_depth: metrics.control_queue_depth,
                        control_queue_peak: metrics.control_queue_peak,
                        control_received: metrics.control_received,
                        control_applied: metrics.control_applied,
                        control_rejected: metrics.control_rejected,
                        control_wait_ns: metrics.control_wait_ns,
                        control_apply_ns: metrics.control_apply_ns,
                        compilation_applied: metrics.compilation_applied,
                        compilation_rejected: metrics.compilation_rejected,
                        sparse_batches: metrics.sparse_batches,
                        dense_batches: metrics.dense_batches,
                        work_units: metrics.work_units,
                        effects_committed: metrics.effects_committed,
                        effects_suppressed: metrics.effects_suppressed,
                    });
                }
                stats_window_start = Instant::now();
                stats_window_ticks = 0;
            }

            pacer.wait_for_next_tick(tick.started_at, tick_interval);
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
