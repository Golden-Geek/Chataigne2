//! Production-backed implementation of the stable application facade contracts.
//!
//! This adapter deliberately keeps the current engine authoritative while preventing hosts and
//! transports from owning or locking it directly. It is the application seam through which
//! runtime planes can be selected independently.

mod graph_editing;
mod project_persistence;
mod project_replacement;

pub use graph_editing::GraphEditError;
use graph_editing::{graph_revision_result, transaction_acknowledgement_result};
use project_persistence::ProjectSaveFaultHook;
pub use project_persistence::{ProjectPersistenceStatus, ProjectSaveRequest, ProjectSaveResult};
#[cfg(test)]
pub(crate) use project_persistence::{ProjectSaveFaultCallback, ProjectSaveStage};
#[cfg(test)]
pub(crate) use project_replacement::ProjectReplacementFaultCallback;
use project_replacement::ProjectReplacementFaultHook;
pub use project_replacement::{
    ProjectReplacement, ProjectReplacementResult, ProjectReplacementStage, ProjectRuntimeStatus,
};

use std::collections::HashSet;
use std::convert::Infallible;
use std::sync::Arc;
#[cfg(test)]
use std::sync::Mutex;
use std::sync::atomic::AtomicU64;
#[cfg(test)]
use std::sync::atomic::AtomicUsize;
#[cfg(test)]
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use golden_application::{GraphEditing, HostLifecycle, Observation, Persistence, ProjectTransactions, RuntimeValues};
use golden_persistence::PersistenceCoordinator;
use golden_runtime::{ControlActor, RuntimeMetrics, RuntimeMetricsSnapshot};

use crate::app::{
    ProjectGeneration, ProjectLifecycle, apply_preferences_runtime_limits, live_preferences_root,
    prepare_engine_for_runtime, prepare_engine_for_runtime_recovering, shutdown_engine_for_runtime,
    to_sparse_preferences_json_pretty,
};
use crate::contexts::UiUserContextCandidatesDto;
use crate::engine::{Engine, EngineRuntimeError, EngineTime, ProjectLoadRecoveryReport};
use crate::node::{Node, NodeId};
use crate::parameter::ParamValue;
pub use crate::runtime_center::ProductionInputPort;
use crate::runtime_center::ProductionState;
use crate::script::{ScriptUiConfig, ScriptUiState};
use crate::ui_read_model::{UiEventCapture, UiReadModel};
use crate::ui_sync::{
    UiAck, UiAckStatus, UiEditIntent, UiEventBatch, UiEventKind, UiGraphOp, UiHistoryState, UiParamControlInfoDto,
    UiProjectFileSpec, UiReferenceTargetsDto, UiSnapshot, UiSubscriptionScope,
};

/// Timing captured around one production-backed UI transaction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ApplicationTransactionTiming {
    /// Time spent waiting for the current control-plane adapter.
    pub lock_wait: Duration,
    /// Time spent applying the authoritative transaction.
    pub apply: Duration,
    /// Time spent collecting and publishing the immutable observation delta.
    pub event_collect: Duration,
    /// End-to-end time through capture, excluding caller serialization.
    pub total: Duration,
}

/// Result of one UI transaction applied through the production facade.
#[derive(Clone, Debug)]
pub struct AppliedUiTransaction {
    /// Authoritative transaction acknowledgement.
    pub acknowledgement: UiAck,
    /// Immutable observation delta published in the same ordered control-actor turn as the mutation.
    pub events: UiEventBatch,
    /// Whether the authoritative transaction changed the persisted Preferences subtree.
    pub preferences_changed: bool,
    /// Adapter timing for performance and parity evidence.
    pub timing: ApplicationTransactionTiming,
}

/// Result of an ordered transaction batch applied under one control-plane lock.
#[derive(Clone, Debug)]
pub struct AppliedUiTransactionBatch {
    /// Per-transaction results in request order.
    pub transactions: Vec<AppliedUiTransaction>,
    /// Whether any transaction in the batch changed the persisted Preferences subtree.
    pub preferences_changed: bool,
    /// Time spent waiting for the one batch lock acquisition.
    pub lock_wait: Duration,
}

fn preferences_subtree_node_ids<T: Node>(engine: &Engine<T>) -> HashSet<NodeId> {
    live_preferences_root(engine)
        .map(|root| engine.collect_subtree_node_ids(root).into_iter().collect())
        .unwrap_or_default()
}

fn graph_op_changes_preferences(op: &UiGraphOp, preferences: &HashSet<NodeId>) -> bool {
    match op {
        UiGraphOp::NodeCreated { snapshot, parent, .. } => {
            preferences.contains(&snapshot.node_id)
                || parent.as_ref().is_some_and(|parent| preferences.contains(parent))
        }
        UiGraphOp::SubtreeInserted {
            root, parent, nodes, ..
        } => {
            preferences.contains(root)
                || preferences.contains(parent)
                || nodes.iter().any(|node| preferences.contains(&node.node_id))
        }
        UiGraphOp::SubtreeRemoved {
            root,
            removed_ids,
            parent_after,
        } => {
            preferences.contains(root)
                || removed_ids.iter().any(|node| preferences.contains(node))
                || parent_after
                    .as_ref()
                    .is_some_and(|patch| preferences.contains(&patch.parent))
        }
        UiGraphOp::NodeMoved {
            node,
            old_parent,
            new_parent,
            old_parent_after,
            new_parent_after,
        } => {
            preferences.contains(node)
                || old_parent.as_ref().is_some_and(|parent| preferences.contains(parent))
                || new_parent.as_ref().is_some_and(|parent| preferences.contains(parent))
                || old_parent_after
                    .as_ref()
                    .is_some_and(|patch| preferences.contains(&patch.parent))
                || new_parent_after
                    .as_ref()
                    .is_some_and(|patch| preferences.contains(&patch.parent))
        }
        UiGraphOp::ChildrenReordered { parent, .. } => preferences.contains(parent),
        UiGraphOp::NodeMetaPatched { node, .. } => preferences.contains(node),
        UiGraphOp::ParamPatched { node, param, .. } => preferences.contains(node) || preferences.contains(param),
        UiGraphOp::HistoryPatched { .. } | UiGraphOp::LoggerPatched { .. } => false,
    }
}

fn event_batch_changes_preferences(batch: &UiEventBatch, preferences: &HashSet<NodeId>) -> bool {
    if preferences.is_empty() {
        return false;
    }

    batch.events.iter().any(|event| match &event.kind {
        UiEventKind::GraphTransaction { transaction } => transaction
            .ops
            .iter()
            .any(|op| graph_op_changes_preferences(op, preferences)),
        UiEventKind::ParamChanged { param, .. }
        | UiEventKind::ParamControlChanged { param, .. }
        | UiEventKind::ParamConstraintsChanged { param, .. } => preferences.contains(param),
        UiEventKind::ChildAdded { parent, child, .. } | UiEventKind::ChildRemoved { parent, child } => {
            preferences.contains(parent) || preferences.contains(child)
        }
        UiEventKind::ChildReplaced { parent, old, new, .. } => {
            preferences.contains(parent) || preferences.contains(old) || preferences.contains(new)
        }
        UiEventKind::ChildMoved {
            child,
            old_parent,
            new_parent,
            ..
        } => preferences.contains(child) || preferences.contains(old_parent) || preferences.contains(new_parent),
        UiEventKind::ChildReordered { parent, .. } => preferences.contains(parent),
        UiEventKind::NodeCreated { node, .. }
        | UiEventKind::NodeDeleted { node }
        | UiEventKind::MetaChanged { node, .. } => preferences.contains(node),
        UiEventKind::Custom { .. } => false,
    })
}

fn event_batch_has_structural_graph_changes(batch: &UiEventBatch) -> bool {
    batch.events.iter().any(|event| match &event.kind {
        UiEventKind::GraphTransaction { transaction } => transaction.ops.iter().any(|op| {
            matches!(
                op,
                UiGraphOp::NodeCreated { .. }
                    | UiGraphOp::SubtreeInserted { .. }
                    | UiGraphOp::SubtreeRemoved { .. }
                    | UiGraphOp::NodeMoved { .. }
            )
        }),
        UiEventKind::ChildAdded { .. }
        | UiEventKind::ChildRemoved { .. }
        | UiEventKind::ChildReplaced { .. }
        | UiEventKind::ChildMoved { .. }
        | UiEventKind::NodeCreated { .. }
        | UiEventKind::NodeDeleted { .. } => true,
        UiEventKind::ChildReordered { .. }
        | UiEventKind::MetaChanged { .. }
        | UiEventKind::ParamChanged { .. }
        | UiEventKind::ParamControlChanged { .. }
        | UiEventKind::ParamConstraintsChanged { .. }
        | UiEventKind::Custom { .. } => false,
    })
}

#[cfg(test)]
pub(crate) type ReadModelPublicationCallback = Arc<dyn Fn() + Send + Sync + 'static>;

#[derive(Clone, Default)]
struct ReadModelPublicationHook {
    #[cfg(test)]
    callback: Arc<Mutex<Option<ReadModelPublicationCallback>>>,
}

impl ReadModelPublicationHook {
    #[inline]
    fn invoke(&self) {
        #[cfg(test)]
        {
            let callback = self
                .callback
                .lock()
                .expect("read-model publication hook poisoned")
                .clone();
            if let Some(callback) = callback {
                callback();
            }
        }
    }

    #[cfg(test)]
    fn set(&self, callback: Option<ReadModelPublicationCallback>) {
        *self.callback.lock().expect("read-model publication hook poisoned") = callback;
    }
}

fn publish_event_capture(
    read_model: &UiReadModel,
    publication_hook: &ReadModelPublicationHook,
    capture: UiEventCapture,
) -> UiEventBatch {
    publication_hook.invoke();
    read_model.apply_event_capture(capture)
}

/// Result of one runtime tick through the production facade.
#[derive(Clone, Debug)]
pub struct ApplicationTickResult {
    /// Observation delta emitted by the tick.
    pub events: UiEventBatch,
    /// Current host loop cap after applying project preferences.
    pub next_interval: Duration,
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
    read_model_publication_hook: ReadModelPublicationHook,
    input_port: ProductionInputPort,
    next_project_generation: AtomicU64,
    latest_requested_project_generation: Arc<AtomicU64>,
    project_replacement_fault_hook: ProjectReplacementFaultHook,
    persistence_coordinator: PersistenceCoordinator,
    project_save_fault_hook: ProjectSaveFaultHook,
    #[cfg(test)]
    preferences_subtree_collection_count: Arc<AtomicUsize>,
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
        let project_was_saved = project_file.current_path.is_some();
        let read_model = Arc::new(UiReadModel::from_engine(&engine, project_file));
        let metrics = Arc::new(RuntimeMetrics::default());
        let (state, input_port) = ProductionState::new(engine, metrics.clone(), project_was_saved)
            .expect("the initial production runtime generation must compile");
        let control = ControlActor::spawn_with_metrics("golden-control", state, metrics)
            .expect("the production control-plane actor must start");
        Self {
            inner: Arc::new(ProductionRuntimeInner {
                control,
                read_model,
                read_model_publication_hook: ReadModelPublicationHook::default(),
                input_port,
                next_project_generation: AtomicU64::new(ProjectGeneration::INITIAL.get() + 1),
                latest_requested_project_generation: Arc::new(AtomicU64::new(ProjectGeneration::INITIAL.get())),
                project_replacement_fault_hook: ProjectReplacementFaultHook::default(),
                persistence_coordinator: PersistenceCoordinator::new(ProjectGeneration::INITIAL.get(), 4),
                project_save_fault_hook: ProjectSaveFaultHook::default(),
                #[cfg(test)]
                preferences_subtree_collection_count: Arc::new(AtomicUsize::new(0)),
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
        let publication_hook = self.inner.read_model_publication_hook.clone();
        #[cfg(test)]
        let preferences_subtree_collection_count = self.inner.preferences_subtree_collection_count.clone();
        let receipt = self
            .inner
            .control
            .call(move |state| {
                if let Some(error) = state.project_pause_error().map(str::to_string) {
                    return intents
                        .into_iter()
                        .map(|_| {
                            let before = state.engine.ui_event_log().last().map(|event| event.time);
                            let capture = read_model.collect_event_batch(&state.engine, before);
                            let events = publish_event_capture(&read_model, &publication_hook, capture);
                            (
                                rejected_ack(
                                    &state.engine,
                                    "project_runtime_paused",
                                    format!("project runtime is paused: {error}"),
                                ),
                                events,
                                false,
                                ApplicationTransactionTiming::default(),
                            )
                        })
                        .collect();
                }
                let mut failed = false;
                let mut opened_edit_session: Option<String> = None;
                let mut pending = Vec::with_capacity(intents.len());
                let mut runtime_compile_requested = false;
                #[cfg(test)]
                preferences_subtree_collection_count.fetch_add(1, Ordering::Relaxed);
                let mut preferences_nodes = preferences_subtree_node_ids(&state.engine);

                for intent in intents {
                    let intent_started = Instant::now();
                    let requires_runtime_compile = ui_intent_requires_runtime_compile(&intent);
                    let is_matching_end_edit = opened_edit_session.as_ref().is_some_and(
                        |active_id| matches!(&intent, UiEditIntent::EndEdit { client_edit_id } if client_edit_id == active_id),
                    );
                    if failed && stop_after_failure && !is_matching_end_edit {
                        let capture = read_model.collect_event_batch(
                            &state.engine,
                            state.engine.ui_event_log().last().map(|event| event.time),
                        );
                        let events = publish_event_capture(&read_model, &publication_hook, capture);
                        pending.push((
                            skipped_after_failed_batch_ack(&state.engine),
                            events,
                            false,
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

                    let before_event_time = state.engine.ui_event_log().last().map(|event| event.time);
                    let apply_started = Instant::now();
                    let acknowledgement =
                        apply_ui_intent_to_engine(&mut state.engine, intent, ui_client_instance_id.as_deref());
                    let apply = apply_started.elapsed();
                    let event_collect_started = Instant::now();
                    let capture = read_model.collect_event_batch(&state.engine, before_event_time);
                    let preferences_changed = if event_batch_has_structural_graph_changes(capture.batch()) {
                        #[cfg(test)]
                        preferences_subtree_collection_count.fetch_add(1, Ordering::Relaxed);
                        let updated_preferences_nodes = preferences_subtree_node_ids(&state.engine);
                        let changed = event_batch_changes_preferences(capture.batch(), &preferences_nodes)
                            || event_batch_changes_preferences(capture.batch(), &updated_preferences_nodes);
                        preferences_nodes = updated_preferences_nodes;
                        changed
                    } else {
                        event_batch_changes_preferences(capture.batch(), &preferences_nodes)
                    };
                    let events = publish_event_capture(&read_model, &publication_hook, capture);
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
                        events,
                        preferences_changed,
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
            .map(|(index, (acknowledgement, events, preferences_changed, mut timing))| {
                if index == 0 {
                    timing.lock_wait = lock_wait;
                }
                timing.total = timing
                    .total
                    .saturating_add(if index == 0 { lock_wait } else { Duration::ZERO });
                AppliedUiTransaction {
                    acknowledgement,
                    events,
                    preferences_changed,
                    timing,
                }
            })
            .collect::<Vec<_>>();
        let preferences_changed = transactions.iter().any(|transaction| transaction.preferences_changed);

        AppliedUiTransactionBatch {
            transactions,
            preferences_changed,
            lock_wait,
        }
    }

    #[cfg(test)]
    pub(crate) fn reset_preferences_subtree_collection_count(&self) {
        self.inner
            .preferences_subtree_collection_count
            .store(0, Ordering::Relaxed);
    }

    #[cfg(test)]
    pub(crate) fn preferences_subtree_collection_count(&self) -> usize {
        self.inner.preferences_subtree_collection_count.load(Ordering::Relaxed)
    }

    #[cfg(test)]
    pub(crate) fn set_read_model_publication_hook(&self, callback: Option<ReadModelPublicationCallback>) {
        self.inner.read_model_publication_hook.set(callback);
    }

    #[cfg(test)]
    pub(crate) fn set_project_replacement_fault_hook(&self, callback: Option<ProjectReplacementFaultCallback>) {
        self.inner.project_replacement_fault_hook.set(callback);
    }

    #[cfg(test)]
    pub(crate) fn set_project_save_fault_hook(&self, callback: Option<ProjectSaveFaultCallback>) {
        self.inner.project_save_fault_hook.set(callback);
    }

    /// Cancels one client's active edit session and publishes resulting events.
    pub fn cancel_ui_edit_session(&self, ui_client_instance_id: &str) -> UiEventBatch {
        let ui_client_instance_id = ui_client_instance_id.to_owned();
        let read_model = self.inner.read_model.clone();
        let publication_hook = self.inner.read_model_publication_hook.clone();
        self.call_engine(move |engine| {
            let before = engine.ui_event_log().last().map(|event| event.time);
            let _ = engine.cancel_active_ui_edit_session_for_client(&ui_client_instance_id);
            let capture = read_model.collect_event_batch(engine, before);
            publish_event_capture(&read_model, &publication_hook, capture)
        })
    }

    /// Runs one authoritative engine tick and publishes its observation delta.
    pub fn run_tick(&self, elapsed: Duration) -> Result<ApplicationTickResult, EngineRuntimeError> {
        let read_model = self.inner.read_model.clone();
        let publication_hook = self.inner.read_model_publication_hook.clone();
        let (events, next_interval) = self
            .inner
            .control
            .call(move |state| {
                let before = state.engine.ui_event_log().last().map(|event| event.time);
                state.run_tick(elapsed)?;
                let engine = &mut state.engine;
                let capture = read_model.collect_event_batch(engine, before);
                apply_preferences_runtime_limits(engine);
                let next_interval = engine.runtime_limits().loop_cap_interval().max(Duration::from_nanos(1));
                let events = publish_event_capture(&read_model, &publication_hook, capture);
                Ok::<_, EngineRuntimeError>((events, next_interval))
            })
            .expect("production control actor disconnected")
            .output?;
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

    #[cfg(test)]
    pub(crate) fn set_project_file(&self, project_file: UiProjectFileSpec) {
        let read_model = self.inner.read_model.clone();
        self.inner
            .control
            .call(move |_state| read_model.set_project_file(project_file))
            .expect("production control actor disconnected");
    }

    fn apply_engine_mutation<R>(&self, mutation: impl FnOnce(&mut Engine<T>) -> R + Send + 'static) -> (R, UiEventBatch)
    where
        R: Send + 'static,
    {
        let read_model = self.inner.read_model.clone();
        let publication_hook = self.inner.read_model_publication_hook.clone();
        self.call_engine(move |engine| {
            let before = engine.ui_event_log().last().map(|event| event.time);
            let result = mutation(engine);
            let capture = read_model.collect_event_batch(engine, before);
            let events = publish_event_capture(&read_model, &publication_hook, capture);
            (result, events)
        })
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
    type Error = GraphEditError;

    fn apply_transaction(&self, transaction: Self::Transaction) -> Result<Self::Receipt, Self::Error> {
        transaction_acknowledgement_result(self.apply_ui_transaction(transaction, None).acknowledgement)
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
    type Error = GraphEditError;

    fn apply_graph_edit(&self, edit: Self::Edit) -> Result<Self::Revision, Self::Error> {
        graph_revision_result(self.apply_ui_transaction(edit, None).acknowledgement)
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
    type SaveResult = ProjectSaveResult;
    type Error = String;

    fn load(&self, request: Self::LoadRequest) -> Result<Self::LoadResult, Self::Error> {
        self.replace_project(request)
    }

    fn save(&self, request: Self::SaveRequest) -> Result<Self::SaveResult, Self::Error> {
        self.save_project(request)
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

pub(crate) fn apply_ui_intent_to_engine<T: ProjectLifecycle>(
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
        } => match engine.duplicate_subtree_with_initial_params(
            source,
            new_parent,
            new_prev_sibling,
            initial_params
                .into_iter()
                .map(|initial_param| (initial_param.decl_id, initial_param.value))
                .collect(),
            |node| node.project_encode_data(),
            |node_type, data, meta| T::project_decode_node(node_type, data, meta),
        ) {
            Ok(_) => applied_ack_since(engine, before_event_time),
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
                Ok(_) => applied_ack_since(engine, before_event_time),
                Err(error) => rejected_ack(engine, "duplicate_nodes_failed", error.to_string()),
            }
        }
        other => engine.apply_ui_intent_from_client(other, ui_client_instance_id),
    }
}

fn applied_ack_since<T: Node>(engine: &Engine<T>, previous_event_time: Option<EngineTime>) -> UiAck {
    let start = engine.ui_event_log_start_index(previous_event_time);
    let events = &engine.ui_event_log()[start..];

    UiAck {
        success: true,
        status: UiAckStatus::Applied,
        error_code: None,
        error_message: None,
        earliest_event_time: events.first().map(|event| event.time),
        latest_event_time: events.last().map(|event| event.time),
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
        latest_event_time: None,
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
        latest_event_time: None,
        history: engine.ui_history_state(),
    }
}
