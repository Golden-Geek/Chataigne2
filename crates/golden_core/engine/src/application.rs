//! Production-backed implementation of the stable application facade contracts.
//!
//! This adapter deliberately keeps the current engine authoritative while preventing hosts and
//! transports from owning or locking it directly. It is the application seam through which
//! runtime planes can be selected independently.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::{Duration, Instant};

use golden_application::{GraphEditing, HostLifecycle, Observation, Persistence, ProjectTransactions, RuntimeValues};
use golden_runtime::{ControlActor, RuntimeMetrics, RuntimeMetricsSnapshot};

use crate::app::{
    ProjectLifecycle, apply_preferences_runtime_limits, prepare_engine_for_runtime,
    prepare_engine_for_runtime_recovering, shutdown_engine_for_runtime, to_sparse_preferences_json_pretty,
    to_sparse_project_json_pretty_with_ui_state,
};
use crate::contexts::UiUserContextCandidatesDto;
use crate::engine::{Engine, EngineRuntimeError, EngineTime, ProjectLoadRecoveryReport};
use crate::node::{Node, NodeId};
use crate::parameter::ParamValue;
pub use crate::runtime_center::ProductionInputPort;
use crate::runtime_center::ProductionState;
use crate::script::{ScriptUiConfig, ScriptUiState};
use crate::ui_read_model::{UiReadModel, UiReadModelReplaceReason};
use crate::ui_sync::{
    UiAck, UiAckStatus, UiEditIntent, UiEventBatch, UiHistoryState, UiParamControlInfoDto, UiProjectFileSpec,
    UiReferenceTargetsDto, UiSnapshot, UiSubscriptionScope,
};

/// Timing captured around one production-backed UI transaction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ApplicationTransactionTiming {
    /// Time spent waiting for the current control-plane adapter.
    pub lock_wait: Duration,
    /// Time spent applying the authoritative transaction.
    pub apply: Duration,
    /// Time spent collecting the immutable observation delta.
    pub event_collect: Duration,
    /// End-to-end time through capture, excluding caller serialization.
    pub total: Duration,
}

/// Result of one UI transaction applied through the production facade.
#[derive(Clone, Debug)]
pub struct AppliedUiTransaction {
    /// Authoritative transaction acknowledgement.
    pub acknowledgement: UiAck,
    /// Immutable observation delta published after releasing the engine adapter lock.
    pub events: UiEventBatch,
    /// Adapter timing for performance and parity evidence.
    pub timing: ApplicationTransactionTiming,
}

/// Result of an ordered transaction batch applied under one control-plane lock.
#[derive(Clone, Debug)]
pub struct AppliedUiTransactionBatch {
    /// Per-transaction results in request order.
    pub transactions: Vec<AppliedUiTransaction>,
    /// Time spent waiting for the one batch lock acquisition.
    pub lock_wait: Duration,
}

/// Result of one runtime tick through the production facade.
#[derive(Clone, Debug)]
pub struct ApplicationTickResult {
    /// Observation delta emitted by the tick.
    pub events: UiEventBatch,
    /// Current host loop cap after applying project preferences.
    pub next_interval: Duration,
}

/// Serialized project snapshot plus capture metadata.
#[derive(Clone, Debug)]
pub struct EncodedProjectDocument {
    /// Pretty JSON project document.
    pub json: String,
    /// Number of live nodes represented by the document.
    pub node_count: usize,
    /// Time spent waiting for the production adapter lock.
    pub lock_wait: Duration,
    /// Time spent encoding the document.
    pub serialize: Duration,
}

/// Request to serialize the live project.
#[derive(Clone, Debug, Default)]
pub struct ProjectSaveRequest {
    /// Optional project-owned UI state stored in the persistence envelope.
    pub ui_state: Option<serde_json::Value>,
}

/// Request to replace the live project with a decoded engine.
pub struct ProjectReplacement<T: ProjectLifecycle> {
    /// Decoded replacement engine.
    pub engine: Engine<T>,
    /// Host-owned project-file metadata for the new observation snapshot.
    pub project_file: UiProjectFileSpec,
    /// Stable replacement reason used by resynchronization diagnostics.
    pub reason: String,
    /// Whether recoverable runtime-startup failures may be retained in the report.
    pub recover: bool,
}

/// Timing and recovery evidence from replacing the live engine.
#[derive(Clone, Debug, Default)]
pub struct ProjectReplacementResult {
    /// Recoverable load/startup problems.
    pub recovery: ProjectLoadRecoveryReport,
    /// Number of nodes in the replacement project.
    pub node_count: usize,
    /// Time spent waiting for the production adapter lock.
    pub lock_wait: Duration,
    /// Time spent shutting down the previous project.
    pub shutdown: Duration,
    /// Time spent dropping the previous project.
    pub drop_previous: Duration,
    /// Time spent preparing the replacement for runtime use.
    pub prepare: Duration,
    /// Total replacement time.
    pub total: Duration,
}

/// Startup policy for the production host-lifecycle adapter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuntimeStartRequest {
    /// Continue after recoverable node-ready failures and report them.
    pub recover: bool,
}

struct ProductionRuntimeInner<T: ProjectLifecycle> {
    control: ControlActor<ProductionState<T>>,
    read_model: Arc<UiReadModel>,
    input_port: ProductionInputPort,
}

/// Current production engine connected through stable application-facing operations.
///
/// Clones share one actor-owned authoritative project and one immutable observation projection.
/// Hosts and transports can only submit typed operations; no engine lock is exposed or acquired on
/// their threads.
pub struct ProductionRuntime<T: ProjectLifecycle> {
    inner: Arc<ProductionRuntimeInner<T>>,
}

impl<T: ProjectLifecycle> Clone for ProductionRuntime<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T: ProjectLifecycle> ProductionRuntime<T> {
    /// Wraps an already-created engine and seeds its immutable observation projection.
    pub fn new(engine: Engine<T>, project_file: UiProjectFileSpec) -> Self {
        let read_model = Arc::new(UiReadModel::from_engine(&engine, project_file));
        let metrics = Arc::new(RuntimeMetrics::default());
        let (state, input_port) = ProductionState::new(engine, metrics.clone())
            .expect("the initial production runtime generation must compile");
        let control = ControlActor::spawn_with_metrics("golden-control", state, metrics)
            .expect("the production control-plane actor must start");
        Self {
            inner: Arc::new(ProductionRuntimeInner {
                control,
                read_model,
                input_port,
            }),
        }
    }

    /// Returns the immutable observation projection used by transport read paths.
    pub fn read_model(&self) -> Arc<UiReadModel> {
        self.inner.read_model.clone()
    }

    /// Returns lock-free control/runtime metrics for diagnostics projection.
    pub fn runtime_metrics(&self) -> RuntimeMetricsSnapshot {
        self.inner.control.metrics().snapshot()
    }

    /// Returns the shared metrics source used by runtime-plane adapters.
    pub fn runtime_metrics_source(&self) -> Arc<RuntimeMetrics> {
        self.inner.control.metrics()
    }

    /// Returns the generation-aware dense input adapter for module and I/O producers.
    pub fn input_port(&self) -> ProductionInputPort {
        self.inner.input_port.clone()
    }

    /// Applies one UI transaction and publishes its captured observation delta.
    pub fn apply_ui_transaction(
        &self,
        intent: UiEditIntent,
        ui_client_instance_id: Option<&str>,
    ) -> AppliedUiTransaction {
        self.apply_ui_transaction_batch(vec![intent], ui_client_instance_id, true)
            .transactions
            .into_iter()
            .next()
            .expect("single transaction batch always returns one result")
    }

    /// Applies ordered UI transactions under one control-plane lock.
    ///
    /// When `stop_after_failure` is true, later transactions receive deterministic skipped
    /// acknowledgements and are not applied.
    pub fn apply_ui_transaction_batch(
        &self,
        intents: Vec<UiEditIntent>,
        ui_client_instance_id: Option<&str>,
        stop_after_failure: bool,
    ) -> AppliedUiTransactionBatch {
        let ui_client_instance_id = ui_client_instance_id.map(str::to_owned);
        let read_model = self.inner.read_model.clone();
        let receipt = self
            .inner
            .control
            .call(move |state| {
                let engine = &mut state.engine;
                let mut failed = false;
                let mut opened_edit_session: Option<String> = None;
                let mut pending = Vec::with_capacity(intents.len());
                let mut runtime_compile_requested = false;

                for intent in intents {
                    let intent_started = Instant::now();
                    let requires_runtime_compile = ui_intent_requires_runtime_compile(&intent);
                    let is_matching_end_edit = opened_edit_session.as_ref().is_some_and(
                        |active_id| matches!(&intent, UiEditIntent::EndEdit { client_edit_id } if client_edit_id == active_id),
                    );
                    if failed && stop_after_failure && !is_matching_end_edit {
                        pending.push((
                            skipped_after_failed_batch_ack(engine),
                            read_model.collect_event_batch(
                                engine,
                                engine.ui_event_log().last().map(|event| event.time),
                            ),
                            ApplicationTransactionTiming {
                                lock_wait: Duration::ZERO,
                                total: intent_started.elapsed(),
                                ..Default::default()
                            },
                        ));
                        continue;
                    }

                    let begin_edit_id = match &intent {
                        UiEditIntent::BeginEdit { client_edit_id, .. } => Some(client_edit_id.clone()),
                        _ => None,
                    };
                    let end_edit_id = match &intent {
                        UiEditIntent::EndEdit { client_edit_id } => Some(client_edit_id.clone()),
                        _ => None,
                    };

                    let before_event_time = engine.ui_event_log().last().map(|event| event.time);
                    let apply_started = Instant::now();
                    let acknowledgement =
                        apply_ui_intent_to_engine(engine, intent, ui_client_instance_id.as_deref());
                    let apply = apply_started.elapsed();
                    let event_collect_started = Instant::now();
                    let capture = read_model.collect_event_batch(engine, before_event_time);
                    let event_collect = event_collect_started.elapsed();
                    if acknowledgement.success {
                        if let Some(client_edit_id) = begin_edit_id {
                            opened_edit_session = Some(client_edit_id);
                        }
                        if end_edit_id.as_ref() == opened_edit_session.as_ref() {
                            opened_edit_session = None;
                        }
                    }
                    failed |= !acknowledgement.success;
                    runtime_compile_requested |= acknowledgement.success && requires_runtime_compile;
                    pending.push((
                        acknowledgement,
                        capture,
                        ApplicationTransactionTiming {
                            lock_wait: Duration::ZERO,
                            apply,
                            event_collect,
                            total: intent_started.elapsed(),
                        },
                    ));
                }
                if runtime_compile_requested {
                    state.request_compilation("ui.graph");
                }
                pending
            })
            .expect("production control actor disconnected");
        let lock_wait = receipt.queue_wait;
        let pending = receipt.output;

        let transactions = pending
            .into_iter()
            .enumerate()
            .map(|(index, (acknowledgement, capture, mut timing))| {
                if index == 0 {
                    timing.lock_wait = lock_wait;
                }
                timing.total = timing
                    .total
                    .saturating_add(if index == 0 { lock_wait } else { Duration::ZERO });
                let events = self.inner.read_model.apply_event_capture(capture);
                AppliedUiTransaction {
                    acknowledgement,
                    events,
                    timing,
                }
            })
            .collect();

        AppliedUiTransactionBatch {
            transactions,
            lock_wait,
        }
    }

    /// Cancels one client's active edit session and publishes resulting events.
    pub fn cancel_ui_edit_session(&self, ui_client_instance_id: &str) -> UiEventBatch {
        let ui_client_instance_id = ui_client_instance_id.to_owned();
        let read_model = self.inner.read_model.clone();
        let capture = self.call_engine(move |engine| {
            let before = engine.ui_event_log().last().map(|event| event.time);
            let _ = engine.cancel_active_ui_edit_session_for_client(&ui_client_instance_id);
            read_model.collect_event_batch(engine, before)
        });
        self.inner.read_model.apply_event_capture(capture)
    }

    /// Runs one authoritative engine tick and publishes its observation delta.
    pub fn run_tick(&self, elapsed: Duration) -> Result<ApplicationTickResult, EngineRuntimeError> {
        let read_model = self.inner.read_model.clone();
        let (capture, next_interval) = self
            .inner
            .control
            .call(move |state| {
                let before = state.engine.ui_event_log().last().map(|event| event.time);
                state.run_tick(elapsed)?;
                let engine = &mut state.engine;
                let capture = read_model.collect_event_batch(engine, before);
                apply_preferences_runtime_limits(engine);
                let next_interval = engine.runtime_limits().loop_cap_interval().max(Duration::from_nanos(1));
                Ok::<_, EngineRuntimeError>((capture, next_interval))
            })
            .expect("production control actor disconnected")
            .output?;
        let events = self.inner.read_model.apply_event_capture(capture);
        Ok(ApplicationTickResult { events, next_interval })
    }

    /// Returns current history state without exposing the live engine.
    pub fn history_state(&self) -> UiHistoryState {
        self.call_engine(|engine| engine.ui_history_state())
    }

    /// Returns reference-picker targets for one parameter.
    pub fn reference_targets(&self, param: NodeId) -> UiReferenceTargetsDto {
        self.call_engine(move |engine| engine.ui_reference_targets_for_param(param))
    }

    /// Returns lexical context candidates for one parameter.
    pub fn context_candidates(&self, param: NodeId) -> UiUserContextCandidatesDto {
        self.call_engine(move |engine| engine.ui_context_candidates_for_param(param))
    }

    /// Returns control-mode information for one parameter.
    pub fn param_control_info(&self, param: NodeId) -> Result<UiParamControlInfoDto, String> {
        self.call_engine(move |engine| engine.ui_param_control_info(param))
    }

    /// Returns current script runtime state.
    pub fn script_state(&self, node: NodeId) -> Result<ScriptUiState, String> {
        self.call_engine(move |engine| engine.ui_script_state(node))
    }

    /// Replaces script configuration and publishes resulting observation events.
    pub fn set_script_config(
        &self,
        node: NodeId,
        config: ScriptUiConfig,
        force_reload: bool,
    ) -> (Result<(), String>, UiEventBatch) {
        self.apply_engine_mutation(move |engine| engine.ui_set_script_config(node, config, force_reload))
    }

    /// Requests script reload and publishes resulting observation events.
    pub fn reload_script(&self, node: NodeId) -> (Result<(), String>, UiEventBatch) {
        self.apply_engine_mutation(move |engine| engine.ui_reload_script(node))
    }

    /// Applies project-derived runtime limits after preferences change.
    pub fn refresh_runtime_limits(&self) {
        self.call_engine(apply_preferences_runtime_limits);
    }

    /// Serializes the Preferences subtree through the authoritative sparse codec.
    pub fn encode_preferences(&self) -> Result<Option<String>, String> {
        self.call_engine(|engine| to_sparse_preferences_json_pretty(engine).map_err(|error| error.to_string()))
    }

    /// Serializes the live project through the authoritative sparse codec.
    pub fn encode_project(&self, request: ProjectSaveRequest) -> Result<EncodedProjectDocument, String> {
        let receipt = self
            .inner
            .control
            .call(move |state| {
                let engine = &mut state.engine;
                let node_count = engine.nodes.iter().count();
                let serialize_started = Instant::now();
                let json = to_sparse_project_json_pretty_with_ui_state(engine, request.ui_state)
                    .map_err(|error| error.to_string())?;
                Ok::<_, String>((json, node_count, serialize_started.elapsed()))
            })
            .map_err(|error| error.to_string())?;
        let (json, node_count, serialize) = receipt.output?;
        Ok(EncodedProjectDocument {
            json,
            node_count,
            lock_wait: receipt.queue_wait,
            serialize,
        })
    }

    /// Replaces the live project after the caller decodes and configures it.
    pub fn replace_project(&self, request: ProjectReplacement<T>) -> Result<ProjectReplacementResult, String> {
        let started = Instant::now();
        let read_model = self.inner.read_model.clone();
        let receipt = self
            .inner
            .control
            .call(move |state| {
                let engine = &mut state.engine;
                let node_count = request.engine.nodes.iter().count();
                let shutdown_started = Instant::now();
                shutdown_engine_for_runtime(engine);
                let shutdown = shutdown_started.elapsed();

                let drop_started = Instant::now();
                let previous = std::mem::replace(engine, request.engine);
                drop(previous);
                let drop_previous = drop_started.elapsed();

                let prepare_started = Instant::now();
                let recovery = if request.recover {
                    prepare_engine_for_runtime_recovering(engine)
                } else {
                    prepare_engine_for_runtime(engine).map_err(|error| error.to_string())?;
                    ProjectLoadRecoveryReport::default()
                };
                let prepare = prepare_started.elapsed();
                state.recompile_blocking("project.replace")?;
                let engine = &mut state.engine;
                engine.clear_ui_event_log();
                engine.push_ui_custom_event(
                    "__transport.resync_required",
                    None,
                    serde_json::json!({ "reason": request.reason }),
                );
                read_model.replace_from_engine(engine, request.project_file, UiReadModelReplaceReason::ProjectReplaced);
                read_model.publish_engine_events_since(engine, None);
                Ok::<_, String>((recovery, node_count, shutdown, drop_previous, prepare))
            })
            .map_err(|error| error.to_string())?;
        let (recovery, node_count, shutdown, drop_previous, prepare) = receipt.output?;
        Ok(ProjectReplacementResult {
            recovery,
            node_count,
            lock_wait: receipt.queue_wait,
            shutdown,
            drop_previous,
            prepare,
            total: started.elapsed(),
        })
    }

    /// Updates host-owned project-file metadata in the immutable observation model.
    pub fn set_project_file(&self, project_file: UiProjectFileSpec) {
        self.inner.read_model.set_project_file(project_file);
    }

    fn apply_engine_mutation<R>(&self, mutation: impl FnOnce(&mut Engine<T>) -> R + Send + 'static) -> (R, UiEventBatch)
    where
        R: Send + 'static,
    {
        let read_model = self.inner.read_model.clone();
        let (result, capture) = self.call_engine(move |engine| {
            let before = engine.ui_event_log().last().map(|event| event.time);
            let result = mutation(engine);
            let capture = read_model.collect_event_batch(engine, before);
            (result, capture)
        });
        (result, self.inner.read_model.apply_event_capture(capture))
    }

    fn call_engine<R>(&self, operation: impl FnOnce(&mut Engine<T>) -> R + Send + 'static) -> R
    where
        R: Send + 'static,
    {
        self.inner
            .control
            .call(move |state| operation(&mut state.engine))
            .expect("production control actor disconnected")
            .output
    }
}

impl<T: ProjectLifecycle> ProjectTransactions for ProductionRuntime<T> {
    type Transaction = UiEditIntent;
    type Receipt = UiAck;
    type Error = Infallible;

    fn apply_transaction(&self, transaction: Self::Transaction) -> Result<Self::Receipt, Self::Error> {
        Ok(self.apply_ui_transaction(transaction, None).acknowledgement)
    }

    fn undo(&self) -> Result<Self::Receipt, Self::Error> {
        self.apply_transaction(UiEditIntent::Undo)
    }

    fn redo(&self) -> Result<Self::Receipt, Self::Error> {
        self.apply_transaction(UiEditIntent::Redo)
    }
}

impl<T: ProjectLifecycle> GraphEditing for ProductionRuntime<T> {
    type Edit = UiEditIntent;
    type Revision = UiHistoryState;
    type Error = Infallible;

    fn apply_graph_edit(&self, edit: Self::Edit) -> Result<Self::Revision, Self::Error> {
        let _ = self.apply_ui_transaction(edit, None);
        Ok(self.history_state())
    }
}

impl<T: ProjectLifecycle> RuntimeValues for ProductionRuntime<T> {
    type Key = NodeId;
    type Value = ParamValue;
    type Error = String;

    fn read_value(&self, key: &Self::Key) -> Result<Option<Self::Value>, Self::Error> {
        let key = *key;
        Ok(self.call_engine(move |engine| {
            engine
                .nodes
                .get(key)
                .and_then(Node::engine_param_snapshot)
                .map(|snapshot| snapshot.value)
        }))
    }

    fn publish_input(&self, key: Self::Key, value: Self::Value, source_time_ns: u64) -> Result<(), Self::Error> {
        self.inner.input_port.publish(key, value, source_time_ns)
    }
}

impl<T: ProjectLifecycle> Observation for ProductionRuntime<T> {
    type SnapshotRequest = UiSubscriptionScope;
    type Snapshot = UiSnapshot;
    type DeltaRequest = (Option<EngineTime>, UiSubscriptionScope);
    type Delta = UiEventBatch;
    type Error = Infallible;

    fn snapshot(&self, request: Self::SnapshotRequest) -> Result<Self::Snapshot, Self::Error> {
        Ok(self.inner.read_model.snapshot_for_scope(request))
    }

    fn changes(&self, request: Self::DeltaRequest) -> Result<Self::Delta, Self::Error> {
        Ok(self.inner.read_model.replay(request.0, request.1))
    }
}

impl<T: ProjectLifecycle> Persistence for ProductionRuntime<T> {
    type LoadRequest = ProjectReplacement<T>;
    type LoadResult = ProjectReplacementResult;
    type SaveRequest = ProjectSaveRequest;
    type SaveResult = EncodedProjectDocument;
    type Error = String;

    fn load(&self, request: Self::LoadRequest) -> Result<Self::LoadResult, Self::Error> {
        self.replace_project(request)
    }

    fn save(&self, request: Self::SaveRequest) -> Result<Self::SaveResult, Self::Error> {
        self.encode_project(request)
    }
}

impl<T: ProjectLifecycle> HostLifecycle for ProductionRuntime<T> {
    type StartRequest = RuntimeStartRequest;
    type StartResult = ProjectLoadRecoveryReport;
    type StopRequest = ();
    type StopResult = ();
    type Error = String;

    fn start(&self, request: Self::StartRequest) -> Result<Self::StartResult, Self::Error> {
        self.inner
            .control
            .call(move |state| {
                let recovery = if request.recover {
                    prepare_engine_for_runtime_recovering(&mut state.engine)
                } else {
                    prepare_engine_for_runtime(&mut state.engine).map_err(|error| error.to_string())?;
                    ProjectLoadRecoveryReport::default()
                };
                apply_preferences_runtime_limits(&mut state.engine);
                state.recompile_blocking("runtime.start")?;
                Ok(recovery)
            })
            .map_err(|error| error.to_string())?
            .output
    }

    fn stop(&self, _request: Self::StopRequest) -> Result<Self::StopResult, Self::Error> {
        self.call_engine(shutdown_engine_for_runtime);
        Ok(())
    }
}

fn ui_intent_requires_runtime_compile(intent: &UiEditIntent) -> bool {
    !matches!(
        intent,
        UiEditIntent::BeginEdit { .. }
            | UiEditIntent::EndEdit { .. }
            | UiEditIntent::SetParam { .. }
            | UiEditIntent::SetTextParamSmart { .. }
            | UiEditIntent::SendNodeEvent { .. }
            | UiEditIntent::ReevaluateGraph
            | UiEditIntent::ClearLogs
            | UiEditIntent::SetLogMaxEntries { .. }
    )
}

fn apply_ui_intent_to_engine<T: ProjectLifecycle>(
    engine: &mut Engine<T>,
    intent: UiEditIntent,
    ui_client_instance_id: Option<&str>,
) -> UiAck {
    let before_event_time = engine.ui_event_log().last().map(|event| event.time);

    match intent {
        UiEditIntent::DuplicateNode {
            source,
            new_parent,
            new_prev_sibling,
            initial_params,
        } => match engine.duplicate_subtree_with(
            source,
            new_parent,
            new_prev_sibling,
            None,
            |node| node.project_encode_data(),
            |node_type, data, meta| T::project_decode_node(node_type, data, meta),
        ) {
            Ok(duplicated_root) => {
                match engine.ui_apply_initial_params_to_node(duplicated_root, initial_params, "DuplicateNode") {
                    Ok(()) => applied_ack(engine, earliest_ui_event_time_since(engine, before_event_time)),
                    Err(error) => rejected_ack(engine, "duplicate_node_failed", error.to_string()),
                }
            }
            Err(error) => rejected_ack(engine, "duplicate_node_failed", error.to_string()),
        },
        UiEditIntent::DuplicateNodes {
            nodes,
            created_items,
            dependent_items,
        } => {
            match engine.ui_apply_duplicate_nodes_with_dependent_user_items(
                nodes,
                created_items,
                dependent_items,
                |node| node.project_encode_data(),
                |node_type, data, meta| T::project_decode_node(node_type, data, meta),
            ) {
                Ok(_) => applied_ack(engine, earliest_ui_event_time_since(engine, before_event_time)),
                Err(error) => rejected_ack(engine, "duplicate_nodes_failed", error.to_string()),
            }
        }
        other => engine.apply_ui_intent_from_client(other, ui_client_instance_id),
    }
}

fn earliest_ui_event_time_since<T: Node>(
    engine: &Engine<T>,
    previous_event_time: Option<EngineTime>,
) -> Option<EngineTime> {
    engine
        .ui_event_log()
        .iter()
        .find(|event| previous_event_time.is_none_or(|previous| event.time > previous))
        .map(|event| event.time)
}

fn applied_ack<T: Node>(engine: &Engine<T>, earliest_event_time: Option<EngineTime>) -> UiAck {
    UiAck {
        success: true,
        status: UiAckStatus::Applied,
        error_code: None,
        error_message: None,
        earliest_event_time,
        history: engine.ui_history_state(),
    }
}

fn rejected_ack<T: Node>(engine: &Engine<T>, code: &str, message: String) -> UiAck {
    UiAck {
        success: false,
        status: UiAckStatus::Rejected,
        error_code: Some(code.to_string()),
        error_message: Some(message),
        earliest_event_time: None,
        history: engine.ui_history_state(),
    }
}

fn skipped_after_failed_batch_ack<T: Node>(engine: &Engine<T>) -> UiAck {
    UiAck {
        success: false,
        status: UiAckStatus::Rejected,
        error_code: Some("intent_batch_cancelled".to_string()),
        error_message: Some("intent batch stopped after a previous failure".to_string()),
        earliest_event_time: None,
        history: engine.ui_history_state(),
    }
}
