use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, LazyLock},
};

use chataigne_state_machine::{
    ANodeOutputPreviewSample, ANodeOutputPreviewSampleDto, ContextKeyDto,
    DefaultProcessorContextProvider, Processor, ProcessorDebugCapture,
    ProcessorFormulaUiState, ProcessorId, ProcessorLaneSummaryDto, ProcessorUiDto,
    ProcessorLifecycleEvent, ProcessorLifecyclePolicy, ProcessorRuntime,
    StateMachineProtocolBundle, processor_output_preview_samples,
    ValueLaneKey, ValueSet, ValueSetEntry,
    alchemist::{CONDITIONS_MANAGER_TYPE, INPUTS_MANAGER_TYPE, node_registry, value_type_registry},
};
use golden_alchemist::{
    ANodeId, AlchemistFormula, CompiledAlchemistFormula, ContextKey, EvaluationCtx,
    DebugValueSample, FormulaCompileKey, FormulaRef, ManagedItemId, ManagedItemInstance,
    ManagedItemUiState, ManagedRegionInstance, OutputPreviewStatus, RuntimeInputSnapshot,
    RuntimeIntent, RuntimeRegistries, RuntimeValue, SignatureCtx, SocketId, StableRef,
    SurfaceItemId, TriggerValue, ValueTypeId, compile_graph, formula_input_value_ref,
};
use golden_core::{
    engine::NodeExecutionRule,
    events::{Event, EventFrame, EventKind},
    log,
    node,
    node::{
        Node, NodeCreationContext, NodeId, NodeMetaPatch, NodeUserPermissions,
        NodeUuid, UserContainerRules, UserCreatableItem,
    },
    parameter::ParamValue,
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::state_machine_nodes_formula::{
    anode_from_snapshot, constraint_value_type, formula_from_snapshot, local_signature_bindings,
    param_to_runtime_value as formula_param_to_runtime_value, runtime_value_to_param,
    ANODE_NODE_TYPE, FORMULA_EXTERNAL_READ_ONLY_TAG,
};
use crate::app::state_machine_nodes_processor::{
    processor_managed_region_decl_id, FormulaCatalog, FormulaSourceRef,
    PROCESSOR_FORMULA_SOURCE_DECL_ID, PROCESSOR_MANAGED_REGION_DECL_PREFIX,
    PROCESSOR_MANAGED_REGIONS_DECL_ID,
};

pub(crate) const STATE_ITEM_KIND: &str = "state";
const STATE_NODE_TYPE: &str = "state";
const FORMULA_LIBRARY_NODE_TYPE: &str = "alchemist_formula_library";
const FORMULA_NODE_TYPE: &str = "alchemist_formula";
const PROCESSOR_MANAGER_DECL_ID: &str = "processors";
const PROCESSOR_NODE_TYPE: &str = "state_processor";
const PROCESSOR_FOLDER_NODE_TYPE: &str = "state_processor_folder";
const STATE_MACHINE_RUNTIME_HZ: u32 = 60;
const STATE_MACHINE_RUNTIME_PREVIEW_TOPIC: &str = "chataigne.state_machine.runtime_preview";
const STATE_MACHINE_PREVIEW_CHANGED_MIN_TICKS: u64 = 1;
const CONDITION_PULSE_HOLD_TICKS: u64 = 6;
const STATE_MACHINE_PREVIEW_KEEPALIVE_TICKS: u64 = 60;
const STATE_MACHINE_LOG_MIN_TICKS: u64 = 30;
const RUNTIME_OUTPUT_PREVIEW_HISTORY_LEN: usize = 64;
const STATE_MACHINE_RUNTIME_WARNING_ID: &str = "state_machine_runtime";
const CONDITION_MANAGER_NODE_TYPE: &str = "sm_condition_manager";
const INPUT_VALUE_CONDITION_NODE_TYPE: &str = "sm_input_value_condition";
const CONDITION_GROUP_NODE_TYPE: &str = "sm_condition_group";
const INPUTS_MANAGER_NODE_TYPE: &str = "sm_inputs_manager";
const INPUT_SOURCE_NODE_TYPE: &str = "sm_input_source";

struct RuntimeProcessor {
    processor: Processor,
    runtime: ProcessorRuntime,
    formula: AlchemistFormula,
    formula_node: Option<NodeId>,
    formula_ui: ProcessorFormulaUiState,
    formula_source_key: String,
}

struct RuntimeLogRecord {
    value: String,
    tick: u64,
}

struct RuntimeFormulaDefaultPreview {
    processor: Processor,
    runtime: ProcessorRuntime,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum RuntimeLogKind {
    Compile,
    Runtime,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct RuntimeLogKey {
    processor_node: NodeId,
    kind: RuntimeLogKind,
}

impl RuntimeLogKey {
    fn processor_compile(processor_node: NodeId) -> Self {
        Self {
            processor_node,
            kind: RuntimeLogKind::Compile,
        }
    }

    fn processor_runtime(processor_node: NodeId) -> Self {
        Self {
            processor_node,
            kind: RuntimeLogKind::Runtime,
        }
    }
}

#[derive(Clone)]
struct FormulaInputValueParam {
    formula: NodeUuid,
    anode: NodeUuid,
    socket: SocketId,
    is_trigger: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ConditionValidity {
    current: bool,
    settled: bool,
}

impl ConditionValidity {
    fn steady(valid: bool) -> Self {
        Self {
            current: valid,
            settled: valid,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct StateMachineRuntimePerfStats {
    pub runtime_cache_rebuilds: u64,
    pub formula_materializations: u64,
    pub formula_compiles: u64,
    pub debug_samples_captured: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct OutputPreviewSampleKey {
    formula_id: golden_alchemist::FormulaId,
    processor_id: Option<ProcessorId>,
    context_key: Option<ContextKey>,
    author_node_id: ANodeId,
    output_socket: SocketId,
}

impl OutputPreviewSampleKey {
    fn from_sample(sample: &ANodeOutputPreviewSample) -> Self {
        Self {
            formula_id: sample.formula_id.clone(),
            processor_id: sample.processor_id,
            context_key: sample.context_key.clone(),
            author_node_id: sample.author_node_id,
            output_socket: sample.output_socket.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct OutputPreviewSignature(Vec<OutputPreviewSignaturePart>);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct OutputPreviewSignaturePart {
    key: OutputPreviewSampleKey,
    status: OutputPreviewStatusKey,
    value: RuntimeValueSignature,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum OutputPreviewStatusKey {
    Live,
    DefaultPreview,
    Stale,
    Error,
    Suppressed,
    Unavailable,
}

impl From<OutputPreviewStatus> for OutputPreviewStatusKey {
    fn from(value: OutputPreviewStatus) -> Self {
        match value {
            OutputPreviewStatus::Live => Self::Live,
            OutputPreviewStatus::DefaultPreview => Self::DefaultPreview,
            OutputPreviewStatus::Stale => Self::Stale,
            OutputPreviewStatus::Error => Self::Error,
            OutputPreviewStatus::Suppressed => Self::Suppressed,
            OutputPreviewStatus::Unavailable => Self::Unavailable,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum RuntimeValueSignature {
    Unit,
    Bool(bool),
    Trigger {
        fired: bool,
        edge_id: u64,
        logical_tick: u64,
    },
    Int(i64),
    Float(u64),
    String(Arc<str>),
    Vec2([u64; 2]),
    Vec3([u64; 3]),
    Color([u32; 4]),
    Duration(std::time::Duration),
    Array(Vec<RuntimeValueSignature>),
    Ref {
        value_type: ValueTypeId,
        stable_id: Arc<str>,
    },
    Extension {
        value_type: ValueTypeId,
        payload: Vec<u8>,
    },
}

impl RuntimeValueSignature {
    fn from_value(value: &RuntimeValue) -> Self {
        match value {
            RuntimeValue::Unit => Self::Unit,
            RuntimeValue::Bool(value) => Self::Bool(*value),
            RuntimeValue::Trigger(trigger) => Self::Trigger {
                fired: trigger.fired,
                edge_id: trigger.edge_id,
                logical_tick: trigger.logical_tick,
            },
            RuntimeValue::Int(value) => Self::Int(*value),
            RuntimeValue::Float(value) => Self::Float(value.to_bits()),
            RuntimeValue::String(value) => Self::String(Arc::clone(value)),
            RuntimeValue::Vec2(value) => Self::Vec2(value.map(f64::to_bits)),
            RuntimeValue::Vec3(value) => Self::Vec3(value.map(f64::to_bits)),
            RuntimeValue::Color(value) => Self::Color([
                value.red.to_bits(),
                value.green.to_bits(),
                value.blue.to_bits(),
                value.alpha.to_bits(),
            ]),
            RuntimeValue::Duration(value) => Self::Duration(*value),
            RuntimeValue::Array(values) => {
                Self::Array(values.iter().map(Self::from_value).collect())
            }
            RuntimeValue::Ref(value) => Self::Ref {
                value_type: value.value_type.clone(),
                stable_id: Arc::clone(&value.stable_id),
            },
            RuntimeValue::Extension(value) => Self::Extension {
                value_type: value.value_type.clone(),
                payload: value.payload.to_vec(),
            },
        }
    }
}

#[derive(Default)]
struct StateMachineRuntimeCache {
    topology_dirty: bool,
    structure_dirty: HashSet<NodeUuid>,
    dirty_formula_values: HashSet<NodeUuid>,
    dirty_processor_overrides: HashSet<NodeId>,
    dirty_input_source_params: HashSet<NodeUuid>,
    formulas: HashMap<NodeUuid, AlchemistFormula>,
    formula_input_values: HashMap<StableRef, RuntimeValue>,
    compiled_formulas: HashMap<FormulaCompileKey, Arc<CompiledAlchemistFormula>>,
    source_listener_params: HashSet<NodeId>,
    source_listener_param_uuids: HashMap<NodeId, NodeUuid>,
    input_manager_signal_ticks: HashMap<String, u64>,
    condition_manager_values: HashMap<String, RuntimeValue>,
    condition_manager_valid_states: HashMap<String, bool>,
    input_value_condition_inner_valid_states: HashMap<NodeUuid, bool>,
    transient_condition_valid_resets: HashMap<NodeId, u64>,
    next_trigger_edge_id: u64,
    processors: HashMap<NodeId, RuntimeProcessor>,
    formula_default_previews: HashMap<golden_alchemist::FormulaId, RuntimeFormulaDefaultPreview>,
    output_preview_snapshot: HashMap<OutputPreviewSampleKey, ANodeOutputPreviewSample>,
    last_preview_signature: Option<OutputPreviewSignature>,
    last_preview_tick: Option<u64>,
    last_log_values: HashMap<RuntimeLogKey, RuntimeLogRecord>,
    perf_stats: StateMachineRuntimePerfStats,
}

static RUNTIME_OUTPUT_PREVIEWS_ENABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var_os("CHATAIGNE_RUNTIME_OUTPUT_PREVIEWS").is_some_and(|value| {
        let value = value.to_string_lossy();
        !matches!(value.trim().to_ascii_lowercase().as_str(), "" | "0" | "false" | "off")
    })
});

#[node("state_machine_manager", label = "State Machine")]
pub struct StateMachineManager {
    #[state(default = StateMachineRuntimeCache::default())]
    runtime_cache: StateMachineRuntimeCache,
}

#[node("state_machine_manager", from_struct)]
impl Node for StateMachineManager {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[STATE_ITEM_KIND]))
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        item_kind == STATE_ITEM_KIND && crate::app::declared_user_item_type_matches(item_type, STATE_ITEM_KIND)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        crate::app::declared_user_creatable_items(STATE_ITEM_KIND)
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        crate::app::create_declared_user_item(node_type, STATE_ITEM_KIND)
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        let mut permissions = NodeUserPermissions::all();
        permissions.can_remove_and_duplicate = false;
        self.node_data_mut().meta.user_permissions = permissions;
        self.runtime_cache.topology_dirty = true;
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            self.runtime_cache.topology_dirty = true;
            return;
        };
        ctx.add_event_listener_subtree(self.id(), snapshot.root(), 1);
        for library in formula_libraries(&snapshot) {
            ctx.add_event_listener_subtree(self.id(), library, u32::MAX);
        }
        self.runtime_cache.topology_dirty = true;
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.run_processors(ctx);
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        self.runtime_cache.topology_dirty
            || !self.runtime_cache.structure_dirty.is_empty()
            || !self.runtime_cache.dirty_processor_overrides.is_empty()
            || !self.runtime_cache.dirty_input_source_params.is_empty()
            || !self.runtime_cache.dirty_formula_values.is_empty()
            || self
                .runtime_cache
                .processors
                .values()
                .any(|processor| processor_needs_continuous_evaluation(&processor.runtime))
    }

    fn inbox_requires_tree_snapshot(&self, events: &EventFrame) -> bool {
        events.iter().any(|event| match &event.kind {
            EventKind::ParamChanged { param, .. } => {
                !self.runtime_cache.source_listener_param_uuids.contains_key(param)
            }
            EventKind::Custom(_) => false,
            _ => true,
        })
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::periodic(STATE_MACHINE_RUNTIME_HZ)
    }

    fn on_child_added(&mut self, ctx: &mut ProcessCtx, parent: golden_core::node::NodeId, child: golden_core::node::NodeId) {
        self.mark_runtime_structure_dirty(ctx, child);
        self.mark_runtime_structure_dirty(ctx, parent);
        crate::app::state_machine_nodes_transition::reconcile_state_networks(ctx, None, None, None);
    }

    fn on_child_removed(
        &mut self,
        ctx: &mut ProcessCtx,
        parent: golden_core::node::NodeId,
        child: golden_core::node::NodeId,
    ) {
        self.mark_runtime_structure_dirty(ctx, child);
        self.mark_runtime_structure_dirty(ctx, parent);
        crate::app::state_machine_nodes_transition::reconcile_state_networks(ctx, None, None, None);
    }

    fn on_node_created(&mut self, ctx: &mut ProcessCtx, node: NodeId) {
        self.mark_runtime_structure_dirty(ctx, node);
    }

    fn on_node_deleted(&mut self, ctx: &mut ProcessCtx, node: NodeId) {
        self.mark_runtime_structure_dirty(ctx, node);
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        let source_signal_dirty = self.mark_input_source_param_dirty(ctx, param);
        if source_signal_dirty {
            return;
        }
        if self.mark_processor_override_dirty(ctx, param) {
            return;
        }
        if self.mark_formula_input_value_dirty(ctx, param) {
            return;
        }
    }

    fn on_meta_changed(&mut self, ctx: &mut ProcessCtx, node: NodeId, _patch: NodeMetaPatch) {
        self.mark_runtime_structure_dirty(ctx, node);
    }

    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        u32::MAX
    }
}

impl StateMachineManager {
    #[cfg(test)]
    pub(crate) fn runtime_perf_stats(&self) -> StateMachineRuntimePerfStats {
        self.runtime_cache.perf_stats
    }

    #[cfg(test)]
    pub(crate) fn runtime_topology_dirty(&self) -> bool {
        self.runtime_cache.topology_dirty
    }

    fn mark_runtime_structure_dirty(&mut self, ctx: &mut ProcessCtx, node: NodeId) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            self.runtime_cache.topology_dirty = true;
            return;
        };
        match runtime_invalidation_for_node(snapshot.as_ref(), self.id(), node) {
            RuntimeInvalidation::Formula(formula) => {
                self.runtime_cache.structure_dirty.insert(formula);
                self.runtime_cache.dirty_formula_values.remove(&formula);
            }
            RuntimeInvalidation::Processor(processor) => {
                self.runtime_cache.dirty_processor_overrides.insert(processor);
            }
            RuntimeInvalidation::Topology => {
                self.runtime_cache.topology_dirty = true;
            }
            RuntimeInvalidation::Ignore => {}
        }
    }

    fn mark_input_source_param_dirty(&mut self, ctx: &mut ProcessCtx, param: NodeId) -> bool {
        if let Some(uuid) = self.runtime_cache.source_listener_param_uuids.get(&param).copied() {
            self.runtime_cache.dirty_input_source_params.insert(uuid);
            return true;
        }
        if !self.runtime_cache.source_listener_params.contains(&param) {
            return false;
        }
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return false;
        };
        if let Some(node) = snapshot.node(param) {
            self.runtime_cache.dirty_input_source_params.insert(node.uuid);
        }
        true
    }

    fn mark_processor_override_dirty(&mut self, ctx: &mut ProcessCtx, param: NodeId) -> bool {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return false;
        };
        let Some(processor_node) = processor_for_override_change(snapshot.as_ref(), param) else {
            return false;
        };
        self.runtime_cache
            .dirty_processor_overrides
            .insert(processor_node);
        true
    }

    fn mark_formula_input_value_dirty(&mut self, ctx: &mut ProcessCtx, param: NodeId) -> bool {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return false;
        };
        let Some(input) = formula_input_value_param(snapshot.as_ref(), param) else {
            return false;
        };
        self.runtime_cache.dirty_formula_values.insert(input.formula);
        let reference = formula_input_value_ref(
            golden_alchemist::AlchemistGraphId::from_uuid(input.formula.0),
            ANodeId::from_uuid(input.anode.0),
            &input.socket,
        );
        if input.is_trigger {
            let edge_id = self.runtime_cache.next_trigger_edge_id;
            self.runtime_cache.next_trigger_edge_id =
                self.runtime_cache.next_trigger_edge_id.wrapping_add(1);
            self.runtime_cache.formula_input_values.insert(
                reference,
                RuntimeValue::Trigger(TriggerValue::fired(edge_id, ctx.time.tick)),
            );
            return true;
        }
        let Some(value) = formula_input_runtime_value(
            snapshot.as_ref(),
            param,
            &input,
            &self.runtime_cache.formulas,
        ) else {
            self.runtime_cache.structure_dirty.insert(input.formula);
            return true;
        };
        self.runtime_cache.formula_input_values.insert(reference, value);
        true
    }

    fn refresh_formula_cache(&mut self, snapshot: &ProcessTreeSnapshot) {
        if self.runtime_cache.topology_dirty || self.runtime_cache.formulas.is_empty() {
            self.runtime_cache.formulas.clear();
            self.runtime_cache.compiled_formulas.clear();
            for library in formula_libraries(snapshot) {
                collect_formulas_in_subtree(
                    snapshot,
                    library,
                    &mut self.runtime_cache.formulas,
                    &mut self.runtime_cache.perf_stats,
                );
            }
            self.runtime_cache.structure_dirty.clear();
            return;
        }

        let dirty = std::mem::take(&mut self.runtime_cache.structure_dirty);
        for formula_uuid in dirty {
            if let Some(previous) = self.runtime_cache.formulas.remove(&formula_uuid) {
                self.runtime_cache
                    .compiled_formulas
                    .retain(|key, _| key.formula_id != previous.id);
            }
            let Some(formula_node) = snapshot.node_id_by_uuid(formula_uuid) else {
                continue;
            };
            let Ok(formula) = formula_from_snapshot(snapshot, formula_node) else {
                continue;
            };
            self.runtime_cache.perf_stats.formula_materializations += 1;
            self.runtime_cache
                .compiled_formulas
                .retain(|key, _| key.formula_id != formula.id);
            self.runtime_cache.formulas.insert(formula_uuid, formula);
        }
    }

    fn run_processors(&mut self, ctx: &mut ProcessCtx) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot.as_ref();
        let active_states = active_state_nodes(snapshot, self.id());
        let cache_rebuilt =
            self.runtime_cache.topology_dirty || !self.runtime_cache.structure_dirty.is_empty();
        let overrides_dirty = !self.runtime_cache.dirty_processor_overrides.is_empty();
        let dirty_input_source_params = self.runtime_cache.dirty_input_source_params.clone();
        let dirty_formula_values = self.runtime_cache.dirty_formula_values.clone();

        if cache_rebuilt {
            self.refresh_formula_cache(snapshot);
            let formulas = self.runtime_cache.formulas.clone();
            let catalog = FormulaCatalog::from_snapshot(snapshot);
            self.rebuild_runtime_cache(ctx, snapshot, &formulas, &catalog);
        } else if overrides_dirty {
            let formulas = self.runtime_cache.formulas.clone();
            let catalog = FormulaCatalog::from_snapshot(snapshot);
            self.refresh_dirty_processor_overrides(snapshot, &formulas, &catalog);
        }
        if cache_rebuilt || overrides_dirty {
            self.refresh_source_event_listeners(ctx, snapshot, &active_states);
        }

        let value_types = value_type_registry();
        let registries = RuntimeRegistries {
            value_types: &value_types,
        };
        let provider = DefaultProcessorContextProvider;
        let capture_output_previews = *RUNTIME_OUTPUT_PREVIEWS_ENABLED;
        let capture = if capture_output_previews {
            ProcessorDebugCapture::All {
                history_len: RUNTIME_OUTPUT_PREVIEW_HISTORY_LEN,
            }
        } else {
            ProcessorDebugCapture::Off
        };
        let mut output_preview = Vec::new();
        let mut previewed_formula_defaults = HashSet::new();
        let mut processor_lanes = Vec::new();
        let mut evaluated_any = false;
        reset_due_transient_condition_valid_params(
            ctx,
            snapshot,
            &mut self.runtime_cache.transient_condition_valid_resets,
        );

        for state in active_states {
            let Some(processor_manager) =
                snapshot.find_child_by_decl_id(state, PROCESSOR_MANAGER_DECL_ID)
            else {
                continue;
            };
            for processor_node in processor_nodes(snapshot, processor_manager) {
                let input_signal_dirty =
                    processor_has_dirty_input_source(snapshot, processor_node, &dirty_input_source_params);
                let condition_signal_dirty =
                    processor_has_dirty_condition_source(snapshot, processor_node, &dirty_input_source_params);
                let Some(runtime_processor) = self.runtime_cache.processors.get(&processor_node) else {
                    continue;
                };
                let formula_value_dirty =
                    processor_formula_node_uuid(snapshot, runtime_processor)
                        .is_some_and(|uuid| dirty_formula_values.contains(&uuid));
                let should_evaluate = processor_should_evaluate(
                    processor_needs_continuous_evaluation(&runtime_processor.runtime),
                    input_signal_dirty,
                    condition_signal_dirty,
                    formula_value_dirty,
                );
                if !should_evaluate {
                    continue;
                }
                let inputs = processor_runtime_inputs(
                    snapshot,
                    processor_node,
                    ctx.time.tick,
                    &dirty_input_source_params,
                    &self.runtime_cache.formula_input_values,
                    &mut self.runtime_cache.input_manager_signal_ticks,
                    &mut self.runtime_cache.condition_manager_values,
                    &mut self.runtime_cache.condition_manager_valid_states,
                    &mut self.runtime_cache.input_value_condition_inner_valid_states,
                    &mut self.runtime_cache.transient_condition_valid_resets,
                    &mut self.runtime_cache.next_trigger_edge_id,
                    ctx,
                );
                let Some(runtime_processor) = self.runtime_cache.processors.get_mut(&processor_node)
                else {
                    continue;
                };
                let compiled_formula = capture_output_previews
                    .then(|| runtime_processor.runtime.compiled.as_ref().map(Arc::clone))
                    .flatten();
                let formula_id = compiled_formula
                    .as_ref()
                    .map(|compiled| compiled.formula_ref.id.clone());
                runtime_processor.runtime.apply_lifecycle(
                    &runtime_processor.processor,
                    ProcessorLifecycleEvent::ProjectStart,
                );
                let eval_ctx = EvaluationCtx {
                    logical_tick: ctx.time.tick,
                    delta_time: ctx.delta_time,
                    events: &[],
                    inputs: &inputs,
                    registries: &registries,
                };
                let lanes = runtime_processor
                    .runtime
                    .evaluate_processor_with_context_provider_and_send_capture(
                        &runtime_processor.processor,
                        &eval_ctx,
                        &provider,
                        &capture,
                    );
                self.runtime_cache.perf_stats.debug_samples_captured += lanes
                    .iter()
                    .map(|lane| lane.output.debug_samples.len() as u64)
                    .sum::<u64>();
                evaluated_any = true;
                let anode_nodes = processor_anode_node_ids(
                    snapshot,
                    runtime_processor.formula_node,
                    processor_node,
                );
                for diagnostic in &runtime_processor.runtime.diagnostics {
                    if should_emit_runtime_log(
                        &mut self.runtime_cache.last_log_values,
                        ctx.time.tick,
                        RuntimeLogKey::processor_compile(processor_node),
                        diagnostic.message.as_str(),
                    ) {
                        log!(
                            origin = processor_node;
                            format!("Processor diagnostic: {}", diagnostic.message)
                        );
                    }
                }
                if let (true, Some(formula_id)) = (capture_output_previews, formula_id.as_ref()) {
                    output_preview.extend(processor_output_preview_samples(
                        runtime_processor.processor.id,
                        formula_id,
                        lanes.clone(),
                    ));
                    if previewed_formula_defaults.insert(formula_id.clone()) {
                        if let Some(compiled_formula) =
                            compiled_formula.as_ref().map(Arc::clone)
                        {
                            output_preview.extend(formula_default_output_preview_samples(
                                &mut self.runtime_cache.formula_default_previews,
                                compiled_formula,
                                &runtime_processor.formula,
                                &eval_ctx,
                                &provider,
                                &capture,
                            ));
                        }
                    }
                }
                for lane in &lanes {
                    processor_lanes.push(processor_lane_summary(
                        runtime_processor.processor.id,
                        lane,
                        ctx.time.tick,
                        processor_needs_continuous_evaluation(&runtime_processor.runtime),
                    ));
                    for diagnostic in &lane.output.diagnostics {
                        if should_emit_runtime_log(
                            &mut self.runtime_cache.last_log_values,
                            ctx.time.tick,
                            RuntimeLogKey::processor_runtime(processor_node),
                            diagnostic.message.as_str(),
                        ) {
                            log!(
                                origin = processor_node;
                                format!("Processor runtime diagnostic: {}", diagnostic.message)
                            );
                        }
                    }
                    for intent in &lane.output.intents {
                        let kind = intent.kind.as_ref();
                        if kind == "debug.log" {
                            let (origin, message) = format_debug_log_intent(
                                snapshot,
                                &runtime_processor.formula.label,
                                processor_node,
                                lane.context_key.as_ref(),
                                &anode_nodes,
                                intent,
                            );
                            log!(
                                origin = origin;
                                format!("{}", message)
                            );
                        } else if kind == chataigne_state_machine::COMMAND_INTENT_KIND {
                            dispatch_command_intent(ctx, snapshot, processor_node, intent);
                        }
                    }
                    for sample in &lane.output.debug_samples {
                        let Some(anode_node) = anode_nodes.get(&sample.author_node_id).copied()
                        else {
                            continue;
                        };
                        if !anode_logs_runtime_value(snapshot, anode_node) {
                            continue;
                        }
                        let message = format_debug_value_sample(
                            snapshot,
                            &runtime_processor.formula.label,
                            processor_node,
                            lane.context_key.as_ref(),
                            anode_node,
                            sample,
                        );
                        log!(
                            origin = anode_node;
                            format!("{}", message)
                        );
                    }
                }
            }
        }
        self.runtime_cache.dirty_input_source_params.clear();
        self.runtime_cache.dirty_formula_values.clear();
        let processors = processor_ui_dtos(&self.runtime_cache.processors);
        if evaluated_any || cache_rebuilt {
            let output_preview = if capture_output_previews {
                merge_output_preview_snapshot(
                    &mut self.runtime_cache.output_preview_snapshot,
                    output_preview,
                )
            } else {
                self.runtime_cache.output_preview_snapshot.clear();
                Vec::new()
            };
            self.publish_output_preview(ctx, processors, output_preview, processor_lanes);
        }
    }

    fn publish_output_preview(
        &mut self,
        ctx: &mut ProcessCtx,
        processors: Vec<ProcessorUiDto>,
        samples: Vec<chataigne_state_machine::ANodeOutputPreviewSample>,
        processor_lanes: Vec<ProcessorLaneSummaryDto>,
    ) {
        let signature = output_preview_signature(&samples);
        let changed = self
            .runtime_cache
            .last_preview_signature
            .as_ref()
            .is_none_or(|previous| previous != &signature);
        let elapsed = self
            .runtime_cache
            .last_preview_tick
            .map(|tick| ctx.time.tick.saturating_sub(tick));
        let should_publish = match elapsed {
            None => true,
            Some(elapsed) if changed => elapsed >= STATE_MACHINE_PREVIEW_CHANGED_MIN_TICKS,
            Some(elapsed) => elapsed >= STATE_MACHINE_PREVIEW_KEEPALIVE_TICKS,
        };
        if !should_publish {
            return;
        }
        let bundle = StateMachineProtocolBundle {
            statechart_deltas: Vec::new(),
            processors,
            diagnostics: Vec::new(),
            runtime_debug: Vec::new(),
            processor_lanes,
            preview_mode: None,
            output_preview: samples.iter().map(ANodeOutputPreviewSampleDto::from).collect(),
        };
        let _ = ctx.emit_custom_payload(STATE_MACHINE_RUNTIME_PREVIEW_TOPIC, None, &bundle);
        self.runtime_cache.last_preview_signature = Some(signature);
        self.runtime_cache.last_preview_tick = Some(ctx.time.tick);
    }

    fn rebuild_runtime_cache(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        formulas: &HashMap<NodeUuid, AlchemistFormula>,
        catalog: &FormulaCatalog,
    ) {
        let mut next_processors = HashMap::new();
        let value_types = value_type_registry();
        let nodes = node_registry();
        let compile_ctx = golden_alchemist::CompileCtx {
            value_types: &value_types,
            nodes: &nodes,
            properties: None,
        };
        let active_processors = active_processor_nodes(snapshot, self.id());
        for processor_node in active_processors {
            let Some((formula_node, formula, formula_ui, formula_source_key)) =
                processor_formula_from_snapshot(snapshot, processor_node, formulas, catalog)
            else {
                continue;
            };
            let Some(processor) = processor_from_snapshot(snapshot, processor_node, &formula) else {
                continue;
            };
            let mut runtime = self
                .runtime_cache
                .processors
                .remove(&processor_node)
                .map(|cached| cached.runtime)
                .unwrap_or_else(|| ProcessorRuntime::new(processor.id));
            if runtime.id != processor.id {
                runtime = ProcessorRuntime::new(processor.id);
            }
            let compiled = match self.shared_compiled_formula(&formula, &compile_ctx) {
                Ok(compiled_formula) => runtime
                    .compile_from_shared_formula_with_compile_ctx_preserving_compatible_lanes(
                        &processor,
                        &formula,
                        compiled_formula,
                        &compile_ctx,
                    ),
                Err(_) => compile_processor_runtime_for_cache_rebuild(
                    &mut runtime,
                    &processor,
                    &formula,
                    &compile_ctx,
                ),
            };
            if !compiled {
                let message = runtime
                    .diagnostics
                    .first()
                    .map(|diagnostic| diagnostic.message.as_str())
                    .unwrap_or("Processor formula failed to compile");
                ctx.set_node_warning_with(
                    processor_node,
                    Some(STATE_MACHINE_RUNTIME_WARNING_ID),
                    "Processor is not running",
                    Some(message),
                );
            } else {
                ctx.clear_node_warning(processor_node, Some(STATE_MACHINE_RUNTIME_WARNING_ID));
            }
            next_processors.insert(
                processor_node,
                RuntimeProcessor {
                    processor,
                    runtime,
                    formula,
                    formula_node,
                    formula_ui,
                    formula_source_key,
                },
            );
        }
        self.runtime_cache.processors = next_processors;
        self.runtime_cache.dirty_processor_overrides.clear();
        self.runtime_cache.dirty_formula_values.clear();
        self.runtime_cache.formula_default_previews.clear();
        self.runtime_cache.output_preview_snapshot.clear();
        self.runtime_cache.last_preview_signature = None;
        self.runtime_cache.topology_dirty = false;
        self.runtime_cache.structure_dirty.clear();
        self.runtime_cache.perf_stats.runtime_cache_rebuilds += 1;
    }

    fn shared_compiled_formula(
        &mut self,
        formula: &AlchemistFormula,
        ctx: &golden_alchemist::CompileCtx<'_>,
    ) -> Result<Arc<CompiledAlchemistFormula>, Vec<golden_alchemist::Diagnostic>> {
        let key = FormulaCompileKey::from_formula(formula, u64::from(formula.version), 0, 0);
        if let Some(compiled) = self.runtime_cache.compiled_formulas.get(&key) {
            return Ok(Arc::clone(compiled));
        }
        self.runtime_cache.perf_stats.formula_compiles += 1;
        let formula_ctx = golden_alchemist::CompileCtx {
            value_types: ctx.value_types,
            nodes: ctx.nodes,
            properties: Some(&formula.properties),
        };
        let result = compile_graph(&formula.graph, &formula_ctx);
        let diagnostics = result.diagnostics;
        let Some(compiled_graph) = result.compiled else {
            return Err(diagnostics);
        };
        let compiled = Arc::new(CompiledAlchemistFormula::new(
            FormulaRef {
                id: formula.id.clone(),
                version: formula.version,
            },
            compiled_graph,
            diagnostics,
        ));
        self.runtime_cache
            .compiled_formulas
            .insert(key, Arc::clone(&compiled));
        Ok(compiled)
    }

    fn refresh_dirty_processor_overrides(
        &mut self,
        snapshot: &ProcessTreeSnapshot,
        formulas: &HashMap<NodeUuid, AlchemistFormula>,
        catalog: &FormulaCatalog,
    ) {
        let dirty_processors = std::mem::take(&mut self.runtime_cache.dirty_processor_overrides);
        for processor_node in dirty_processors {
            let Some(runtime_processor) = self.runtime_cache.processors.get_mut(&processor_node)
            else {
                continue;
            };
            let Some((formula_node, formula, formula_ui, formula_source_key)) =
                processor_formula_from_snapshot(snapshot, processor_node, formulas, catalog)
            else {
                self.runtime_cache.topology_dirty = true;
                continue;
            };
            let Some(processor) = processor_from_snapshot(snapshot, processor_node, &formula) else {
                self.runtime_cache.topology_dirty = true;
                continue;
            };
            runtime_processor.processor = processor;
            runtime_processor.formula = formula;
            runtime_processor.formula_node = formula_node;
            runtime_processor.formula_ui = formula_ui;
            runtime_processor.formula_source_key = formula_source_key;
        }
    }

    fn refresh_source_event_listeners(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        active_states: &[NodeId],
    ) {
        let next_listeners = collect_source_listener_params(snapshot, active_states);
        let current_listeners = self.runtime_cache.source_listener_params.clone();

        for target in current_listeners.difference(&next_listeners).copied() {
            ctx.remove_event_listener(self.id(), target);
        }
        for target in next_listeners.difference(&current_listeners).copied() {
            ctx.add_event_listener(self.id(), target);
        }

        self.runtime_cache.source_listener_param_uuids = next_listeners
            .iter()
            .filter_map(|param| snapshot.node(*param).map(|node| (*param, node.uuid)))
            .collect();
        self.runtime_cache.source_listener_params = next_listeners;
    }
}

#[cfg(test)]
mod manager_tests;

fn merge_output_preview_snapshot(
    snapshot: &mut HashMap<OutputPreviewSampleKey, ANodeOutputPreviewSample>,
    samples: Vec<ANodeOutputPreviewSample>,
) -> Vec<ANodeOutputPreviewSample> {
    for sample in samples {
        let key = OutputPreviewSampleKey::from_sample(&sample);
        if snapshot
            .get(&key)
            .is_some_and(|current| sample.logical_tick < current.logical_tick)
        {
            continue;
        }
        snapshot.insert(key, sample);
    }

    let mut entries = snapshot.iter().collect::<Vec<_>>();
    entries.sort_by(|(left, _), (right, _)| left.cmp(right));
    entries
        .into_iter()
        .map(|(_, sample)| sample.clone())
        .collect()
}

fn formula_input_value_param(
    snapshot: &ProcessTreeSnapshot,
    param: NodeId,
) -> Option<FormulaInputValueParam> {
    let param_node = snapshot.node(param)?;
    let socket = input_value_socket_id(param_node.decl_id.as_str())?;
    let socket_node = param_node.parent?;
    let socket_snapshot = snapshot.node(socket_node)?;
    let expected_socket_decl_id = format!("inputs/{socket}");
    if !node_matches_decl_id(socket_snapshot.decl_id.as_str(), &expected_socket_decl_id) {
        return None;
    }
    let inputs_folder = socket_snapshot.parent?;
    let inputs_snapshot = snapshot.node(inputs_folder)?;
    if !node_matches_decl_id(inputs_snapshot.decl_id.as_str(), "inputs") {
        return None;
    }
    let anode = inputs_snapshot.parent?;
    let anode_snapshot = snapshot.node(anode)?;
    if anode_snapshot.node_type != ANODE_NODE_TYPE {
        return None;
    }
    let formula = anode_snapshot.parent?;
    let formula_snapshot = snapshot.node(formula)?;
    if formula_snapshot.node_type != FORMULA_NODE_TYPE {
        return None;
    }
    Some(FormulaInputValueParam {
        formula: formula_snapshot.uuid,
        anode: anode_snapshot.uuid,
        socket: SocketId::new(socket),
        is_trigger: matches!(param_node.param_value.as_ref(), Some(ParamValue::Trigger())),
    })
}

fn formula_input_runtime_value(
    snapshot: &ProcessTreeSnapshot,
    param: NodeId,
    input: &FormulaInputValueParam,
    formulas: &HashMap<NodeUuid, AlchemistFormula>,
) -> Option<RuntimeValue> {
    let param_value = snapshot.node(param)?.param_value.as_ref()?;
    let value_type = formula_input_socket_value_type(formulas.get(&input.formula)?, input)?;
    formula_param_to_runtime_value(param_value, &value_type).ok()
}

fn formula_input_socket_value_type(
    formula: &AlchemistFormula,
    input: &FormulaInputValueParam,
) -> Option<ValueTypeId> {
    let anode_id = ANodeId::from_uuid(input.anode.0);
    let instance = formula.graph.nodes.get(&anode_id)?;
    let value_types = value_type_registry();
    let nodes = node_registry();
    let declaration = nodes.get(&instance.type_id)?;
    let signature = declaration.signature(
        &SignatureCtx {
            value_types: &value_types,
            properties: Some(&formula.properties),
        },
        instance,
        &instance.type_bindings,
    );
    let bindings = local_signature_bindings(&signature, instance);
    let socket = signature
        .inputs
        .into_iter()
        .find(|candidate| candidate.id == input.socket)?;
    Some(constraint_value_type(&socket.constraint, &bindings))
}

fn input_value_socket_id(decl_id: &str) -> Option<&str> {
    decl_id
        .strip_prefix("inputs/")?
        .strip_suffix("/value")
        .filter(|socket| !socket.is_empty())
}

fn formula_libraries(snapshot: &ProcessTreeSnapshot) -> Vec<NodeId> {
    snapshot
        .child_ids(snapshot.root())
        .into_iter()
        .filter(|node| {
            snapshot.node(*node).is_some_and(|snapshot_node| {
                snapshot_node.node_type == FORMULA_LIBRARY_NODE_TYPE
            })
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RuntimeInvalidation {
    Formula(NodeUuid),
    Processor(NodeId),
    Topology,
    Ignore,
}

fn runtime_invalidation_for_node(
    snapshot: &ProcessTreeSnapshot,
    manager: NodeId,
    node_id: NodeId,
) -> RuntimeInvalidation {
    let mut current = Some(node_id);
    while let Some(candidate) = current {
        let Some(node) = snapshot.node(candidate) else {
            return RuntimeInvalidation::Topology;
        };
        if node.node_type == FORMULA_NODE_TYPE {
            return RuntimeInvalidation::Formula(node.uuid);
        }
        if node.node_type == PROCESSOR_NODE_TYPE {
            return if candidate == node_id {
                RuntimeInvalidation::Topology
            } else {
                RuntimeInvalidation::Processor(candidate)
            };
        }
        if candidate == manager || node.node_type == STATE_NODE_TYPE {
            return RuntimeInvalidation::Topology;
        }
        current = node.parent;
    }
    RuntimeInvalidation::Ignore
}

fn collect_formulas_in_subtree(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    formulas: &mut HashMap<NodeUuid, AlchemistFormula>,
    stats: &mut StateMachineRuntimePerfStats,
) {
    for child in snapshot.child_ids(parent) {
        let Some(node) = snapshot.node(child) else {
            continue;
        };
        if node.node_type == FORMULA_NODE_TYPE {
            if let Ok(formula) =
                crate::app::state_machine_nodes_formula::formula_from_snapshot(snapshot, child)
            {
                stats.formula_materializations += 1;
                formulas.insert(node.uuid, formula);
            }
        } else {
            collect_formulas_in_subtree(snapshot, child, formulas, stats);
        }
    }
}

fn active_state_nodes(snapshot: &ProcessTreeSnapshot, manager: NodeId) -> Vec<NodeId> {
    snapshot
        .child_ids(manager)
        .into_iter()
        .filter(|state| {
            snapshot
                .node(*state)
                .is_some_and(|node| node.node_type == STATE_NODE_TYPE && node.enabled)
                && child_param(snapshot, *state, "active")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false)
        })
        .collect()
}

fn active_processor_nodes(snapshot: &ProcessTreeSnapshot, manager: NodeId) -> HashSet<NodeId> {
    active_state_nodes(snapshot, manager)
        .into_iter()
        .filter_map(|state| snapshot.find_child_by_decl_id(state, PROCESSOR_MANAGER_DECL_ID))
        .flat_map(|processor_manager| processor_nodes(snapshot, processor_manager))
        .collect()
}

fn processor_nodes(snapshot: &ProcessTreeSnapshot, parent: NodeId) -> Vec<NodeId> {
    let mut processors = Vec::new();
    for child in snapshot.child_ids(parent) {
        let Some(node) = snapshot.node(child) else {
            continue;
        };
        if !node.enabled {
            continue;
        }
        match node.node_type.as_str() {
            PROCESSOR_NODE_TYPE => processors.push(child),
            PROCESSOR_FOLDER_NODE_TYPE => processors.extend(processor_nodes(snapshot, child)),
            _ => {}
        }
    }
    processors
}

fn collect_source_listener_params(
    snapshot: &ProcessTreeSnapshot,
    active_states: &[NodeId],
) -> HashSet<NodeId> {
    let mut listeners = HashSet::new();
    for state in active_states {
        let Some(processor_manager) =
            snapshot.find_child_by_decl_id(*state, PROCESSOR_MANAGER_DECL_ID)
        else {
            continue;
        };
        for processor in processor_nodes(snapshot, processor_manager) {
            // Property surfaces are mirrored flat at the processor's top level.
            collect_property_source_listener_params(snapshot, processor, &mut listeners);
        }
    }
    listeners
}

fn collect_property_source_listener_params(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    listeners: &mut HashSet<NodeId>,
) {
    for child in snapshot.child_ids(parent) {
        let Some(node) = snapshot.node(child) else {
            continue;
        };
        if !node.enabled {
            continue;
        }
        match node.node_type.as_str() {
            PROCESSOR_FOLDER_NODE_TYPE
            | INPUTS_MANAGER_NODE_TYPE
            | CONDITION_MANAGER_NODE_TYPE
            | CONDITION_GROUP_NODE_TYPE => {
                collect_property_source_listener_params(snapshot, child, listeners);
            }
            INPUT_SOURCE_NODE_TYPE | INPUT_VALUE_CONDITION_NODE_TYPE => {
                if let Some(source) = child_reference_uuid(snapshot, child, "source")
                    .and_then(|uuid| snapshot.node_id_by_uuid(uuid))
                {
                    listeners.insert(source);
                }
            }
            _ => {}
        }
    }
}

fn processor_for_override_change(
    snapshot: &ProcessTreeSnapshot,
    changed_node: NodeId,
) -> Option<NodeId> {
    let mut current = Some(changed_node);
    let mut surface_seen = false;
    while let Some(node_id) = current {
        let node = snapshot.node(node_id)?;
        if node.node_type == PROCESSOR_NODE_TYPE {
            // Property surfaces are mirrored flat at the processor's top level
            // with a `surface/` decl prefix. Treat the change as an override
            // only if it originated inside one of those surfaces.
            return surface_seen.then_some(node_id);
        }
        // A change inside the managed-regions subtree is a managed ANode edit,
        // not a processor property override.
        if node_matches_decl_id(node.decl_id.as_str(), PROCESSOR_MANAGED_REGIONS_DECL_ID) {
            return None;
        }
        if node.decl_id.starts_with("surface/") {
            surface_seen = true;
        }
        current = node.parent;
    }
    None
}

fn processor_from_snapshot(
    snapshot: &ProcessTreeSnapshot,
    processor_node: NodeId,
    formula: &AlchemistFormula,
) -> Option<Processor> {
    let node = snapshot.node(processor_node)?;
    let mut processor = Processor::from_formula(&node.label, formula);
    processor.id = chataigne_state_machine::ProcessorId::from_uuid(node.uuid.0);
    processor.lifecycle = ProcessorLifecyclePolicy::AlwaysActive;
    apply_processor_overrides(snapshot, processor_node, &mut processor);
    apply_processor_managed_regions(snapshot, processor_node, formula, &mut processor)?;
    Some(processor)
}

fn compile_processor_runtime_for_cache_rebuild(
    runtime: &mut ProcessorRuntime,
    processor: &Processor,
    formula: &AlchemistFormula,
    ctx: &golden_alchemist::CompileCtx<'_>,
) -> bool {
    runtime.compile_preserving_compatible_lanes(processor, formula, ctx)
}

fn formula_default_output_preview_samples(
    cache: &mut HashMap<golden_alchemist::FormulaId, RuntimeFormulaDefaultPreview>,
    compiled: Arc<CompiledAlchemistFormula>,
    formula: &AlchemistFormula,
    ctx: &EvaluationCtx<'_>,
    provider: &DefaultProcessorContextProvider,
    capture: &ProcessorDebugCapture,
) -> Vec<chataigne_state_machine::ANodeOutputPreviewSample> {
    let key = formula.id.clone();
    let entry = cache.entry(key).or_insert_with(|| {
        let mut processor =
            Processor::from_formula(format!("{} defaults", formula.label), formula);
        processor.lifecycle = ProcessorLifecyclePolicy::AlwaysActive;
        RuntimeFormulaDefaultPreview {
            runtime: ProcessorRuntime::new(processor.id),
            processor,
        }
    });
    let needs_compile = entry.runtime.compiled.as_ref().is_none_or(|current| {
        current.formula_ref.id != compiled.formula_ref.id
            || current.formula_ref.version != compiled.formula_ref.version
    });
    if needs_compile
        && !entry
            .runtime
            .compile_from_shared_formula_preserving_compatible_lanes(
                &entry.processor,
                formula,
                compiled,
            )
    {
        return Vec::new();
    }
    entry
        .runtime
        .apply_lifecycle(&entry.processor, ProcessorLifecycleEvent::ProjectStart);
    let lanes = entry.runtime.evaluate_processor_with_context_provider_and_send_capture(
        &entry.processor,
        ctx,
        provider,
        capture,
    );
    let mut samples = processor_output_preview_samples(entry.processor.id, &formula.id, lanes);
    for sample in &mut samples {
        sample.processor_id = None;
        sample.status = OutputPreviewStatus::DefaultPreview;
    }
    samples
}

fn processor_needs_continuous_evaluation(runtime: &ProcessorRuntime) -> bool {
    runtime
        .compiled
        .as_ref()
        .is_some_and(|compiled| compiled.analysis.has_always_process_nodes)
}

fn processor_ui_dtos(processors: &HashMap<NodeId, RuntimeProcessor>) -> Vec<ProcessorUiDto> {
    let mut dtos: Vec<_> = processors
        .values()
        .filter_map(|runtime_processor| {
            Some(ProcessorUiDto::from(
                &runtime_processor.processor.ui_model_with_formula_source(
                    &runtime_processor.formula,
                    runtime_processor.runtime.diagnostics.clone(),
                    runtime_processor.formula_ui,
                    Some(runtime_processor.formula_source_key.clone()),
                ),
            ))
        })
        .collect();
    dtos.sort_by(|left, right| left.label.cmp(&right.label).then_with(|| left.id.cmp(&right.id)));
    dtos
}

fn processor_lane_summary(
    processor_id: chataigne_state_machine::ProcessorId,
    lane: &chataigne_state_machine::ProcessorLaneOutput,
    tick: u64,
    has_memory: bool,
) -> ProcessorLaneSummaryDto {
    ProcessorLaneSummaryDto {
        processor_id: processor_id.to_string(),
        context_key: lane.context_key.as_ref().map(ContextKeyDto::from),
        label: context_key_label(lane.context_key.as_ref()),
        has_memory,
        last_tick: Some(tick),
        diagnostics_count: lane.output.diagnostics.len(),
    }
}

fn context_key_label(context_key: Option<&golden_alchemist::ContextKey>) -> String {
    let Some(context_key) = context_key else {
        return "Default lane".to_string();
    };
    if context_key.is_default_lane() {
        return "Default lane".to_string();
    }
    context_key
        .iter()
        .map(|part| part.item.as_str())
        .collect::<Vec<_>>()
        .join(" / ")
}

fn output_preview_signature(
    samples: &[chataigne_state_machine::ANodeOutputPreviewSample],
) -> OutputPreviewSignature {
    let mut parts = samples
        .iter()
        .map(|sample| OutputPreviewSignaturePart {
            key: OutputPreviewSampleKey::from_sample(sample),
            status: OutputPreviewStatusKey::from(sample.status),
            value: RuntimeValueSignature::from_value(&sample.value),
        })
        .collect::<Vec<_>>();
    parts.sort();
    OutputPreviewSignature(parts)
}

fn should_emit_runtime_log(
    last_values: &mut HashMap<RuntimeLogKey, RuntimeLogRecord>,
    tick: u64,
    key: RuntimeLogKey,
    value: &str,
) -> bool {
    if let Some(previous) = last_values.get(&key) {
        if previous.value == value {
            return false;
        }
        if tick.saturating_sub(previous.tick) < STATE_MACHINE_LOG_MIN_TICKS {
            return false;
        }
    }
    last_values.insert(
        key,
        RuntimeLogRecord {
            value: value.to_string(),
            tick,
        },
    );
    true
}

fn processor_anode_node_ids(
    snapshot: &ProcessTreeSnapshot,
    formula_node: Option<NodeId>,
    processor_node: NodeId,
) -> HashMap<ANodeId, NodeId> {
    let mut nodes = formula_node.map_or_else(HashMap::new, |formula_node| {
        snapshot
            .child_ids(formula_node)
            .into_iter()
            .filter_map(|child| {
                let node = snapshot.node(child)?;
                (node.node_type == ANODE_NODE_TYPE)
                    .then(|| (ANodeId::from_uuid(node.uuid.0), child))
            })
            .collect()
    });
    if let Some(regions_root) =
        snapshot.find_child_by_decl_id(processor_node, PROCESSOR_MANAGED_REGIONS_DECL_ID)
    {
        for region in snapshot.child_ids(regions_root) {
            let Some(region_node) = snapshot.node(region) else {
                continue;
            };
            if !region_node
                .decl_id
                .starts_with(PROCESSOR_MANAGED_REGION_DECL_PREFIX)
            {
                continue;
            }
            for child in snapshot.child_ids(region) {
                let Some(node) = snapshot.node(child) else {
                    continue;
                };
                if node.node_type == ANODE_NODE_TYPE {
                    nodes.insert(ANodeId::from_uuid(node.uuid.0), child);
                }
            }
        }
    }
    nodes
}

fn anode_logs_runtime_value(snapshot: &ProcessTreeSnapshot, anode_node: NodeId) -> bool {
    let anode_type = snapshot
        .node(anode_node)
        .and_then(|node| tagged_value(&node.tags, "alchemist.anode.type:"))
        .unwrap_or_default();
    matches!(anode_type, "debug_value")
}

/// Dispatches a `chataigne.command` runtime intent emitted by a triggered
/// output. The intent loop runs once per lane, so this is invoked once per lane
/// — each call emits its own execute event(s), which is what keeps multiplexed
/// outputs firing one command per lane instead of coalescing into one.
///
/// The intent's `target` points at the Outputs manager (via its `manager_id`
/// reference). We resolve that to the live manager — both directly by uuid and
/// through the processor surface (`surface/<uuid>`) that mirrors a formula
/// manager property — then fire each enabled command it contains. A target that
/// is itself a command is fired directly, and a plain-parameter target is set
/// (the legacy value-output path).
fn dispatch_command_intent(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    processor_node: NodeId,
    intent: &RuntimeIntent,
) {
    let Some(target) = intent.target.as_ref() else {
        return;
    };

    let mut targets: Vec<NodeId> = Vec::new();
    if let Some(direct) = resolve_stable_ref_node(snapshot, target) {
        targets.push(direct);
    }
    if let Some(surface) =
        find_descendant_by_decl_id(snapshot, processor_node, &format!("surface/{}", target.stable_id))
    {
        targets.push(surface);
    }

    let mut fired: HashSet<NodeId> = HashSet::new();
    let mut command_count = 0usize;
    let mut manager_with_children = false;
    for node in &targets {
        if snapshot
            .node(*node)
            .is_some_and(|node| node.node_type == crate::app::OutputsManager::NODE_TYPE)
            && !snapshot.child_ids(*node).is_empty()
        {
            manager_with_children = true;
        }
        command_count += execute_output_target(ctx, snapshot, *node, &intent.payload, &mut fired);
    }

    // Only warn when an Outputs manager actually holds items but none fired — an
    // empty branch (e.g. an unused "On False") is a normal, silent no-op.
    if command_count == 0 && manager_with_children {
        log!(
            origin = processor_node;
            format!("Output dispatch: no command fired for target '{}'", target.stable_id)
        );
    }
}

/// Fires a single output target: a command node directly, every enabled command
/// under an Outputs manager, or a plain parameter (legacy value output). Returns
/// the number of commands fired.
fn execute_output_target(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    node: NodeId,
    payload: &RuntimeValue,
    fired: &mut HashSet<NodeId>,
) -> usize {
    if !fired.insert(node) {
        return 0;
    }
    // A command fires itself; an Outputs manager / group fires through its own
    // scheduler (applying its Delay / Stagger / Cancel) — in both cases we just
    // deliver an execute event to the node and let it do the work.
    if node_is_command(snapshot, node)
        || crate::app::state_machine_nodes_managed_nodes::is_output_container(snapshot, node)
    {
        let _ = crate::app::module_command::emit_command_execute(ctx, node);
        return 1;
    }

    let mut command_count = 0usize;
    for child in snapshot.child_ids(node) {
        if snapshot.node(child).is_some_and(|child| child.enabled) && node_is_command(snapshot, child) {
            if fired.insert(child) {
                let _ = crate::app::module_command::emit_command_execute(ctx, child);
            }
            command_count += 1;
        }
    }
    if command_count > 0 {
        return command_count;
    }

    if let Ok(value) = runtime_value_to_param(payload) {
        set_output_target_param(ctx, snapshot, node, value);
    }
    0
}

fn set_output_target_param(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    node: NodeId,
    value: ParamValue,
) -> bool {
    let Some(current) = snapshot.node(node).and_then(|node| node.param_value.as_ref()) else {
        return false;
    };
    if !matches!(value, ParamValue::Trigger()) && current == &value {
        return false;
    }
    ctx.set_param(node, value);
    true
}

/// A command is a node whose type is a declared module- or generic-command item.
/// (Detecting by a `trigger` child is unreliable — triggers aren't persisted, so
/// a freshly-loaded command may not expose one until re-materialized.)
fn node_is_command(snapshot: &ProcessTreeSnapshot, node: NodeId) -> bool {
    snapshot.node(node).is_some_and(|node| {
        crate::app::declared_user_item_type_matches(
            &node.node_type,
            crate::app::module_command::MODULE_COMMAND_ITEM_KIND,
        ) || crate::app::declared_user_item_type_matches(
            &node.node_type,
            crate::app::state_machine_nodes_generic_commands::GENERIC_COMMAND_ITEM_KIND,
        )
    })
}

fn resolve_stable_ref_node(snapshot: &ProcessTreeSnapshot, target: &StableRef) -> Option<NodeId> {
    let uuid = target.stable_id.parse::<uuid::Uuid>().map(NodeUuid).ok()?;
    snapshot.node_id_by_uuid(uuid)
}

fn find_descendant_by_decl_id(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
) -> Option<NodeId> {
    for child in snapshot.child_ids(parent) {
        if snapshot.node(child).is_some_and(|child| child.decl_id == decl_id) {
            return Some(child);
        }
        if let Some(found) = find_descendant_by_decl_id(snapshot, child, decl_id) {
            return Some(found);
        }
    }
    None
}

fn format_debug_log_intent(
    snapshot: &ProcessTreeSnapshot,
    formula_label: &str,
    processor_node: NodeId,
    context_key: Option<&golden_alchemist::ContextKey>,
    anode_nodes: &HashMap<ANodeId, NodeId>,
    intent: &RuntimeIntent,
) -> (NodeId, String) {
    let context =
        runtime_processing_context_label(snapshot, formula_label, processor_node, context_key);
    let value = runtime_value_label(&intent.payload);
    let Some(anode_node) = intent.source_node.and_then(|node| anode_nodes.get(&node).copied()) else {
        return (processor_node, format!("{context} | {value}"));
    };
    let node_label = snapshot_node_label(snapshot, anode_node, "ANode");
    let detail = intent.source_socket.as_ref().map_or_else(
        || format!(": {value}"),
        |socket| format!(" / {} = {value}", anode_output_label(snapshot, anode_node, socket)),
    );
    (anode_node, format!("{context} | {node_label}{detail}"))
}

fn format_debug_value_sample(
    snapshot: &ProcessTreeSnapshot,
    formula_label: &str,
    processor_node: NodeId,
    context_key: Option<&golden_alchemist::ContextKey>,
    anode_node: NodeId,
    sample: &DebugValueSample,
) -> String {
    let context =
        runtime_processing_context_label(snapshot, formula_label, processor_node, context_key);
    let node_label = snapshot_node_label(snapshot, anode_node, "ANode");
    let output_label = anode_output_label(snapshot, anode_node, &sample.output_socket);
    let value = runtime_value_label(&sample.value);
    format!("{context} | {node_label} / {output_label} = {value}")
}

fn runtime_processing_context_label(
    snapshot: &ProcessTreeSnapshot,
    formula_label: &str,
    processor_node: NodeId,
    context_key: Option<&golden_alchemist::ContextKey>,
) -> String {
    format!(
        "Formula: {} / Processor: {} / Lane: {}",
        formula_label,
        snapshot_node_label(snapshot, processor_node, "Processor"),
        context_key_label(context_key)
    )
}

fn snapshot_node_label(snapshot: &ProcessTreeSnapshot, node: NodeId, fallback: &str) -> String {
    snapshot
        .node(node)
        .map(|node| node.label.trim())
        .filter(|label| !label.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn anode_output_label(
    snapshot: &ProcessTreeSnapshot,
    anode_node: NodeId,
    socket_id: &SocketId,
) -> String {
    let Some(outputs) = snapshot.find_child_by_decl_id(anode_node, "outputs") else {
        return socket_id.to_string();
    };
    for child in snapshot.child_ids(outputs) {
        let Some(node) = snapshot.node(child) else {
            continue;
        };
        if node.node_type != "alchemist_output_socket" {
            continue;
        }
        if socket_runtime_id(snapshot, child).as_deref() == Some(socket_id.as_str()) {
            return snapshot_node_label(snapshot, child, socket_id.as_str());
        }
    }
    socket_id.to_string()
}

fn socket_runtime_id(snapshot: &ProcessTreeSnapshot, socket_node: NodeId) -> Option<String> {
    for child in snapshot.child_ids(socket_node) {
        let Some(node) = snapshot.node(child) else {
            continue;
        };
        if !node.decl_id.ends_with("/socket_id") {
            continue;
        }
        if let Some(ParamValue::Str(value)) = node.param_value.as_ref() {
            return Some(value.to_string());
        }
    }
    None
}

fn tagged_value<'a>(tags: &'a [String], prefix: &str) -> Option<&'a str> {
    tags.iter().find_map(|tag| tag.strip_prefix(prefix))
}

fn apply_processor_overrides(
    snapshot: &ProcessTreeSnapshot,
    processor_node: NodeId,
    processor: &mut Processor,
) {
    // Property surfaces are mirrored flat at the processor's top level; the
    // collector skips non-surface children (formula reference, managed regions).
    collect_processor_property_overrides(snapshot, processor_node, processor);
}

fn processor_formula_source_ref(
    snapshot: &ProcessTreeSnapshot,
    processor_node: NodeId,
) -> Option<FormulaSourceRef> {
    if let Some(ParamValue::Str(source)) =
        child_param(snapshot, processor_node, PROCESSOR_FORMULA_SOURCE_DECL_ID)
            .filter(|value| matches!(value, ParamValue::Str(source) if !source.is_empty()))
    {
        if let Ok(source) = FormulaSourceRef::parse_processor_create_type(source) {
            return Some(source);
        }
    }
    child_reference_uuid(snapshot, processor_node, "formula")
        .map(FormulaSourceRef::project_uuid)
}

fn processor_formula_from_snapshot(
    snapshot: &ProcessTreeSnapshot,
    processor_node: NodeId,
    formulas: &HashMap<NodeUuid, AlchemistFormula>,
    _catalog: &FormulaCatalog,
) -> Option<(Option<NodeId>, AlchemistFormula, ProcessorFormulaUiState, String)> {
    let source = processor_formula_source_ref(snapshot, processor_node)?;
    let source_key = source.processor_create_type();
    let FormulaSourceRef::ProjectNode(reference) = source;
    let uuid = reference.uuid();
    let formula_node = snapshot.node_id_by_uuid(uuid)?;
    let formula_ui = snapshot
        .node(formula_node)
        .filter(|node| {
            node.tags
                .iter()
                .any(|tag| tag == FORMULA_EXTERNAL_READ_ONLY_TAG)
        })
        .map(|_| ProcessorFormulaUiState::builtin(true, false))
        .unwrap_or_else(ProcessorFormulaUiState::project);
    formulas
        .get(&uuid)
        .cloned()
        .map(|formula| (Some(formula_node), formula, formula_ui, source_key))
}

fn apply_processor_managed_regions(
    snapshot: &ProcessTreeSnapshot,
    processor_node: NodeId,
    formula: &AlchemistFormula,
    processor: &mut Processor,
) -> Option<()> {
    let Some(regions_root) =
        snapshot.find_child_by_decl_id(processor_node, PROCESSOR_MANAGED_REGIONS_DECL_ID)
    else {
        return Some(());
    };
    for definition in &formula.surface.managed_regions {
        let decl_id = processor_managed_region_decl_id(definition.id.as_str());
        let Some(region_node) =
            snapshot.find_child_by_decl_id(regions_root, &decl_id)
        else {
            continue;
        };
        let mut region = ManagedRegionInstance {
            region_id: definition.id.clone(),
            items: Vec::new(),
        };
        for child in snapshot.child_ids(region_node) {
            let child_node = snapshot.node(child)?;
            if child_node.node_type != ANODE_NODE_TYPE {
                continue;
            }
            let anode = anode_from_snapshot(snapshot, child).ok()?;
            region.items.push(ManagedItemInstance {
                id: ManagedItemId::from_uuid(child_node.uuid.0),
                anode,
                enabled: child_node.enabled,
                ui_state: ManagedItemUiState {
                    collapsed: child_node.presentation.collapsed,
                },
            });
        }
        processor
            .formula_instance
            .managed_regions
            .regions
            .insert(definition.id.clone(), region);
    }
    Some(())
}

fn collect_processor_property_overrides(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    processor: &mut Processor,
) {
    for child in snapshot.child_ids(parent) {
        let Some(node) = snapshot.node(child) else {
            continue;
        };
        if node.node_type == PROCESSOR_FOLDER_NODE_TYPE {
            collect_processor_property_overrides(snapshot, child, processor);
            continue;
        }
        let Some(value) = processor_override_value(snapshot, child) else {
            continue;
        };
        let Some(surface_id) = node
            .decl_id
            .strip_prefix("surface/")
            .map(SurfaceItemId::new)
        else {
            continue;
        };
        if let Some(runtime_value) = param_to_runtime_value(value) {
            processor
                .formula_instance
                .overrides
                .values
                .insert(surface_id, runtime_value);
        }
    }
}

fn processor_runtime_inputs(
    snapshot: &ProcessTreeSnapshot,
    processor_node: NodeId,
    logical_tick: u64,
    dirty_input_source_params: &HashSet<NodeUuid>,
    formula_input_values: &HashMap<StableRef, RuntimeValue>,
    input_manager_signal_ticks: &mut HashMap<String, u64>,
    condition_manager_values: &mut HashMap<String, RuntimeValue>,
    condition_manager_valid_states: &mut HashMap<String, bool>,
    input_value_condition_inner_valid_states: &mut HashMap<NodeUuid, bool>,
    transient_condition_valid_resets: &mut HashMap<NodeId, u64>,
    next_trigger_edge_id: &mut u64,
    ctx: &mut ProcessCtx,
) -> RuntimeInputSnapshot {
    let mut inputs = RuntimeInputSnapshot::default();
    for (reference, value) in formula_input_values {
        inputs.insert(reference.clone(), value.clone());
    }
    if let Some(processor_uuid) = snapshot.node(processor_node).map(|node| node.uuid) {
        collect_processor_runtime_inputs(
            snapshot,
            processor_uuid,
            processor_node,
            logical_tick,
            dirty_input_source_params,
            input_manager_signal_ticks,
            condition_manager_values,
            condition_manager_valid_states,
            input_value_condition_inner_valid_states,
            transient_condition_valid_resets,
            next_trigger_edge_id,
            ctx,
            &mut inputs,
        );
    }
    inputs
}

fn processor_formula_node_uuid(
    snapshot: &ProcessTreeSnapshot,
    runtime_processor: &RuntimeProcessor,
) -> Option<NodeUuid> {
    runtime_processor
        .formula_node
        .and_then(|node| snapshot.node(node).map(|node| node.uuid))
}

fn processor_should_evaluate(
    continuous: bool,
    input_signal_dirty: bool,
    condition_signal_dirty: bool,
    formula_value_dirty: bool,
) -> bool {
    continuous || input_signal_dirty || condition_signal_dirty || formula_value_dirty
}

fn processor_has_dirty_input_source(
    snapshot: &ProcessTreeSnapshot,
    processor_node: NodeId,
    dirty_input_source_params: &HashSet<NodeUuid>,
) -> bool {
    if dirty_input_source_params.is_empty() {
        return false;
    }
    property_tree_has_dirty_input_source(snapshot, processor_node, dirty_input_source_params)
}

fn processor_has_dirty_condition_source(
    snapshot: &ProcessTreeSnapshot,
    processor_node: NodeId,
    dirty_input_source_params: &HashSet<NodeUuid>,
) -> bool {
    if dirty_input_source_params.is_empty() {
        return false;
    }
    property_tree_has_dirty_condition_source(snapshot, processor_node, dirty_input_source_params)
}

fn property_tree_has_dirty_condition_source(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    dirty_input_source_params: &HashSet<NodeUuid>,
) -> bool {
    snapshot.child_ids(parent).into_iter().any(|child| {
        let Some(node) = snapshot.node(child) else {
            return false;
        };
        if !node.enabled {
            return false;
        }
        match node.node_type.as_str() {
            PROCESSOR_FOLDER_NODE_TYPE => {
                property_tree_has_dirty_condition_source(snapshot, child, dirty_input_source_params)
            }
            CONDITION_MANAGER_NODE_TYPE => {
                condition_tree_has_dirty_source(snapshot, child, dirty_input_source_params)
            }
            _ => false,
        }
    })
}

fn property_tree_has_dirty_input_source(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    dirty_input_source_params: &HashSet<NodeUuid>,
) -> bool {
    snapshot.child_ids(parent).into_iter().any(|child| {
        let Some(node) = snapshot.node(child) else {
            return false;
        };
        if !node.enabled {
            return false;
        }
        match node.node_type.as_str() {
            PROCESSOR_FOLDER_NODE_TYPE => {
                property_tree_has_dirty_input_source(snapshot, child, dirty_input_source_params)
            }
            INPUTS_MANAGER_NODE_TYPE => {
                input_manager_has_dirty_source(snapshot, child, dirty_input_source_params)
            }
            _ => false,
        }
    })
}

fn collect_processor_runtime_inputs(
    snapshot: &ProcessTreeSnapshot,
    processor_uuid: NodeUuid,
    parent: NodeId,
    logical_tick: u64,
    dirty_input_source_params: &HashSet<NodeUuid>,
    input_manager_signal_ticks: &mut HashMap<String, u64>,
    condition_manager_values: &mut HashMap<String, RuntimeValue>,
    condition_manager_valid_states: &mut HashMap<String, bool>,
    input_value_condition_inner_valid_states: &mut HashMap<NodeUuid, bool>,
    transient_condition_valid_resets: &mut HashMap<NodeId, u64>,
    next_trigger_edge_id: &mut u64,
    ctx: &mut ProcessCtx,
    inputs: &mut RuntimeInputSnapshot,
) {
    for child in snapshot.child_ids(parent) {
        let Some(node) = snapshot.node(child) else {
            continue;
        };
        if !node.enabled {
            continue;
        }
        if node.node_type == PROCESSOR_FOLDER_NODE_TYPE {
            collect_processor_runtime_inputs(
                snapshot,
                processor_uuid,
                child,
                logical_tick,
                dirty_input_source_params,
                input_manager_signal_ticks,
                condition_manager_values,
                condition_manager_valid_states,
                input_value_condition_inner_valid_states,
                transient_condition_valid_resets,
                next_trigger_edge_id,
                ctx,
                inputs,
            );
            continue;
        }
        match node.node_type.as_str() {
            INPUTS_MANAGER_NODE_TYPE => collect_input_manager_runtime_input(
                snapshot,
                processor_uuid,
                child,
                node.decl_id.as_str(),
                logical_tick,
                dirty_input_source_params,
                input_manager_signal_ticks,
                inputs,
            ),
            CONDITION_MANAGER_NODE_TYPE => collect_condition_manager_runtime_input(
                snapshot,
                processor_uuid,
                child,
                node.decl_id.as_str(),
                logical_tick,
                dirty_input_source_params,
                condition_manager_values,
                condition_manager_valid_states,
                input_value_condition_inner_valid_states,
                transient_condition_valid_resets,
                next_trigger_edge_id,
                ctx,
                inputs,
            ),
            _ => {}
        }
    }
}

fn collect_input_manager_runtime_input(
    snapshot: &ProcessTreeSnapshot,
    processor_uuid: NodeUuid,
    manager: NodeId,
    decl_id: &str,
    logical_tick: u64,
    dirty_input_source_params: &HashSet<NodeUuid>,
    input_manager_signal_ticks: &mut HashMap<String, u64>,
    inputs: &mut RuntimeInputSnapshot,
) {
    let Some(manager_uuid) = decl_id
        .strip_prefix("surface/")
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    let signal_key = format!("{}:{manager_uuid}", processor_uuid.0);
    if input_manager_has_dirty_source(snapshot, manager, dirty_input_source_params) {
        input_manager_signal_ticks.insert(signal_key.clone(), logical_tick);
    }
    let Some(signal_tick) = input_manager_signal_ticks.get(&signal_key).copied() else {
        return;
    };
    let value_set = processor_input_manager_value_set(snapshot, manager, signal_tick);
    if value_set.entries.is_empty() {
        input_manager_signal_ticks.remove(&signal_key);
        return;
    }
    let Ok(value) = value_set.to_runtime_value() else {
        return;
    };
    inputs.insert(
        StableRef::new(ValueTypeId::new(INPUTS_MANAGER_TYPE), manager_uuid.to_owned()),
        value,
    );
}

fn collect_condition_manager_runtime_input(
    snapshot: &ProcessTreeSnapshot,
    processor_uuid: NodeUuid,
    manager: NodeId,
    decl_id: &str,
    logical_tick: u64,
    dirty_input_source_params: &HashSet<NodeUuid>,
    condition_manager_values: &mut HashMap<String, RuntimeValue>,
    condition_manager_valid_states: &mut HashMap<String, bool>,
    input_value_condition_inner_valid_states: &mut HashMap<NodeUuid, bool>,
    transient_condition_valid_resets: &mut HashMap<NodeId, u64>,
    next_trigger_edge_id: &mut u64,
    ctx: &mut ProcessCtx,
    inputs: &mut RuntimeInputSnapshot,
) {
    let Some(manager_uuid) = decl_id
        .strip_prefix("surface/")
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    let signal_key = format!("{}:{manager_uuid}", processor_uuid.0);
    let previous = condition_manager_valid_states.get(&signal_key).copied();
    let mut current_value = None;
    let source_dirty =
        condition_tree_has_dirty_source(snapshot, manager, dirty_input_source_params);
    if previous.is_none() || source_dirty {
        let validity = condition_group_valid(
            snapshot,
            manager,
            ctx,
            dirty_input_source_params,
            input_value_condition_inner_valid_states,
            transient_condition_valid_resets,
        )
        .unwrap_or_else(|| ConditionValidity::steady(false));
        set_condition_validity_param(
            ctx,
            snapshot,
            manager,
            validity,
            transient_condition_valid_resets,
        );
        let previous = condition_manager_valid_states.insert(signal_key.clone(), validity.settled);

        if let Ok(value) = condition_manager_value_set(
            logical_tick,
            validity.settled,
            Some(validity.settled),
            next_trigger_edge_id,
        )
        .to_runtime_value()
        {
            condition_manager_values.insert(signal_key.clone(), value);
        }

        if previous != Some(validity.current) {
            let previous = condition_manager_edge_previous(
                previous,
                validity.current,
                source_dirty,
            );
            let value_set = condition_manager_value_set(
                logical_tick,
                validity.current,
                previous,
                next_trigger_edge_id,
            );
            if let Ok(value) = value_set.to_runtime_value() {
                current_value = Some(value);
            }
        }
    }
    let Some(value) =
        current_value.or_else(|| condition_manager_values.get(&signal_key).cloned())
    else {
        return;
    };
    inputs.insert(
        StableRef::new(ValueTypeId::new(CONDITIONS_MANAGER_TYPE), manager_uuid.to_owned()),
        value,
    );
}

fn condition_manager_edge_previous(
    previous: Option<bool>,
    current: bool,
    source_dirty: bool,
) -> Option<bool> {
    if source_dirty {
        previous
    } else {
        Some(current)
    }
}

fn condition_manager_value_set(
    logical_tick: u64,
    valid: bool,
    previous: Option<bool>,
    next_trigger_edge_id: &mut u64,
) -> ValueSet {
    let mut value_set = ValueSet::new(logical_tick);
    value_set.push(ValueSetEntry::new(
        ValueLaneKey::new("valid").expect("static ValueSet key must be valid"),
        "Valid",
        RuntimeValue::Bool(valid),
    ));
    let on_true = if previous != Some(valid) && valid {
        RuntimeValue::Trigger(next_trigger(next_trigger_edge_id, logical_tick))
    } else {
        RuntimeValue::Trigger(TriggerValue::default())
    };
    value_set.push(ValueSetEntry::new(
        ValueLaneKey::new("on_true").expect("static ValueSet key must be valid"),
        "On True",
        on_true,
    ));
    let on_false = if previous != Some(valid) && !valid {
        RuntimeValue::Trigger(next_trigger(next_trigger_edge_id, logical_tick))
    } else {
        RuntimeValue::Trigger(TriggerValue::default())
    };
    value_set.push(ValueSetEntry::new(
        ValueLaneKey::new("on_false").expect("static ValueSet key must be valid"),
        "On False",
        on_false,
    ));
    value_set
}

fn next_trigger(next_trigger_edge_id: &mut u64, logical_tick: u64) -> TriggerValue {
    let edge_id = *next_trigger_edge_id;
    *next_trigger_edge_id = (*next_trigger_edge_id).wrapping_add(1);
    TriggerValue::fired(edge_id, logical_tick)
}

fn condition_tree_has_dirty_source(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    dirty_input_source_params: &HashSet<NodeUuid>,
) -> bool {
    snapshot.child_ids(parent).into_iter().any(|child| {
        let Some(node) = snapshot.node(child) else {
            return false;
        };
        if node.param_value.is_some() && node.decl_id != "valid" && dirty_input_source_params.contains(&node.uuid) {
            return true;
        }
        if !node.enabled {
            return false;
        }
        match node.node_type.as_str() {
            INPUT_VALUE_CONDITION_NODE_TYPE => input_value_condition_has_dirty_source(
                snapshot,
                child,
                dirty_input_source_params,
            ),
            CONDITION_GROUP_NODE_TYPE | CONDITION_MANAGER_NODE_TYPE => {
                condition_tree_has_dirty_source(snapshot, child, dirty_input_source_params)
            }
            _ => false,
        }
    })
}

fn input_value_condition_has_dirty_source(
    snapshot: &ProcessTreeSnapshot,
    condition: NodeId,
    dirty_input_source_params: &HashSet<NodeUuid>,
) -> bool {
    child_reference_uuid(snapshot, condition, "source")
        .is_some_and(|source| dirty_input_source_params.contains(&source))
        || snapshot.child_ids(condition).into_iter().any(|child| {
            snapshot.node(child).is_some_and(|node| {
                node.param_value.is_some()
                    && node.decl_id != "valid"
                    && dirty_input_source_params.contains(&node.uuid)
            })
        })
}

fn condition_group_valid(
    snapshot: &ProcessTreeSnapshot,
    group: NodeId,
    ctx: &mut ProcessCtx,
    dirty_input_source_params: &HashSet<NodeUuid>,
    input_value_condition_inner_valid_states: &mut HashMap<NodeUuid, bool>,
    transient_condition_valid_resets: &mut HashMap<NodeId, u64>,
) -> Option<ConditionValidity> {
    let mut current_values = Vec::new();
    let mut settled_values = Vec::new();
    for child in snapshot.child_ids(group) {
        let node = snapshot.node(child)?;
        if node.param_value.is_some() {
            continue;
        }
        let valid = if node.enabled {
            match node.node_type.as_str() {
                INPUT_VALUE_CONDITION_NODE_TYPE => input_value_condition_valid(
                    snapshot,
                    child,
                    ctx,
                    dirty_input_source_params,
                    input_value_condition_inner_valid_states,
                    transient_condition_valid_resets,
                ),
                CONDITION_GROUP_NODE_TYPE => condition_group_valid(
                    snapshot,
                    child,
                    ctx,
                    dirty_input_source_params,
                    input_value_condition_inner_valid_states,
                    transient_condition_valid_resets,
                ),
                _ => None,
            }
        } else {
            None
        };
        if let Some(valid) = valid {
            current_values.push(valid.current);
            settled_values.push(valid.settled);
        }
    }
    Some(ConditionValidity {
        current: reduce_condition_values(snapshot, group, &current_values),
        settled: reduce_condition_values(snapshot, group, &settled_values),
    })
}

fn input_value_condition_valid(
    snapshot: &ProcessTreeSnapshot,
    condition: NodeId,
    ctx: &mut ProcessCtx,
    dirty_input_source_params: &HashSet<NodeUuid>,
    input_value_condition_inner_valid_states: &mut HashMap<NodeUuid, bool>,
    transient_condition_valid_resets: &mut HashMap<NodeId, u64>,
) -> Option<ConditionValidity> {
    let condition_uuid = snapshot.node(condition)?.uuid;
    let source_uuid = child_reference_uuid(snapshot, condition, "source")?;
    let source_id = snapshot.node_id_by_uuid(source_uuid)?;
    let source_value = snapshot.node(source_id)?.param_value.as_ref()?;
    let comparator = child_string(snapshot, condition, "comparator").unwrap_or_else(|| "equal".to_owned());
    let transient = input_value_condition_is_transient(source_value, comparator.as_str());
    let source_changed = dirty_input_source_params.contains(&source_uuid);
    let comparator_valid = if transient {
        source_changed
    } else {
        compare_condition_value(snapshot, condition, source_value, comparator.as_str())
    };
    let previous_inner_valid = input_value_condition_inner_valid_states
        .get(&condition_uuid)
        .copied()
        .unwrap_or(false);
    let toggle_mode = child_bool(snapshot, condition, "toggle_mode").unwrap_or(false);
    let current_valid = child_bool(snapshot, condition, "valid").unwrap_or(false);
    let validity = next_input_value_condition_validity(
        toggle_mode,
        current_valid,
        previous_inner_valid,
        comparator_valid,
        transient,
    );
    input_value_condition_inner_valid_states.insert(
        condition_uuid,
        if transient { false } else { comparator_valid },
    );
    set_condition_validity_param(
        ctx,
        snapshot,
        condition,
        validity,
        transient_condition_valid_resets,
    );
    Some(validity)
}

fn input_value_condition_is_transient(source_value: &ParamValue, comparator: &str) -> bool {
    matches!(source_value, ParamValue::Trigger()) || comparator == "value_changed"
}

fn next_input_value_condition_validity(
    toggle_mode: bool,
    current_valid: bool,
    previous_inner_valid: bool,
    comparator_valid: bool,
    transient: bool,
) -> ConditionValidity {
    if transient {
        return ConditionValidity {
            current: comparator_valid,
            settled: false,
        };
    }
    ConditionValidity::steady(next_input_value_condition_valid_state(
        toggle_mode,
        current_valid,
        previous_inner_valid,
        comparator_valid,
    ))
}

fn next_input_value_condition_valid_state(
    toggle_mode: bool,
    current_valid: bool,
    previous_inner_valid: bool,
    comparator_valid: bool,
) -> bool {
    if !toggle_mode {
        return comparator_valid;
    }
    if !previous_inner_valid && comparator_valid {
        !current_valid
    } else {
        current_valid
    }
}

fn set_condition_validity_param(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    condition: NodeId,
    validity: ConditionValidity,
    transient_condition_valid_resets: &mut HashMap<NodeId, u64>,
) {
    if validity.current != validity.settled {
        set_condition_valid_param(ctx, snapshot, condition, validity.current);
        transient_condition_valid_resets
            .insert(condition, ctx.time.tick.saturating_add(CONDITION_PULSE_HOLD_TICKS));
        return;
    }

    transient_condition_valid_resets.remove(&condition);
    set_condition_valid_param(ctx, snapshot, condition, validity.settled);
}

fn reset_due_transient_condition_valid_params(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    transient_condition_valid_resets: &mut HashMap<NodeId, u64>,
) {
    let now = ctx.time.tick;
    let due = transient_condition_valid_resets
        .iter()
        .filter_map(|(node, reset_tick)| (*reset_tick <= now).then_some(*node))
        .collect::<Vec<_>>();

    for node in due {
        transient_condition_valid_resets.remove(&node);
        set_condition_valid_param(ctx, snapshot, node, false);
    }
}

fn set_condition_valid_param(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    condition: NodeId,
    valid: bool,
) {
    let Some(valid_param) = snapshot.find_child_by_decl_id(condition, "valid") else {
        return;
    };
    let current = snapshot
        .node(valid_param)
        .and_then(|node| node.param_value.as_ref())
        .and_then(ParamValue::as_bool);
    if current != Some(valid) {
        ctx.set_param(valid_param, ParamValue::Bool(valid));
    }
}

fn reduce_condition_values(snapshot: &ProcessTreeSnapshot, group: NodeId, values: &[bool]) -> bool {
    if values.is_empty() {
        return false;
    }
    let valid_count = values.iter().filter(|value| **value).count();
    let required = child_param(snapshot, group, "operator_count")
        .and_then(param_numeric)
        .unwrap_or(1.0)
        .round()
        .max(0.0) as usize;
    match child_string(snapshot, group, "operator")
        .unwrap_or_else(|| "all".to_owned())
        .as_str()
    {
        "any" => valid_count > 0,
        "none" => valid_count == 0,
        "at_least" => valid_count >= required,
        "exactly" => valid_count == required,
        _ => valid_count == values.len(),
    }
}

fn compare_condition_value(
    snapshot: &ProcessTreeSnapshot,
    condition: NodeId,
    source_value: &ParamValue,
    comparator: &str,
) -> bool {
    let reference = child_param(snapshot, condition, "reference")
        .and_then(param_numeric)
        .unwrap_or(0.0);
    let reference_max = child_param(snapshot, condition, "reference_max")
        .and_then(param_numeric)
        .unwrap_or(1.0);
    let reference_string = child_string(snapshot, condition, "reference_string").unwrap_or_default();
    match comparator {
        "not_equal" => !param_values_equal(source_value, reference, reference_string.as_str()),
        "greater_than" => param_numeric(source_value).is_some_and(|value| value > reference),
        "greater_than_or_equal" => param_numeric(source_value).is_some_and(|value| value >= reference),
        "less_than" => param_numeric(source_value).is_some_and(|value| value < reference),
        "less_than_or_equal" => param_numeric(source_value).is_some_and(|value| value <= reference),
        "between" => param_numeric(source_value).is_some_and(|value| {
            let min = reference.min(reference_max);
            let max = reference.max(reference_max);
            value >= min && value <= max
        }),
        "outside" => param_numeric(source_value).is_some_and(|value| {
            let min = reference.min(reference_max);
            let max = reference.max(reference_max);
            value < min || value > max
        }),
        "is_true" => param_bool(source_value).unwrap_or(false),
        "is_false" => !param_bool(source_value).unwrap_or(true),
        "contains" => param_string(source_value).contains(reference_string.as_str()),
        "does_not_contain" => !param_string(source_value).contains(reference_string.as_str()),
        "starts_with" => param_string(source_value).starts_with(reference_string.as_str()),
        "ends_with" => param_string(source_value).ends_with(reference_string.as_str()),
        "regex_match" => regex::Regex::new(reference_string.as_str())
            .is_ok_and(|regex| regex.is_match(param_string(source_value).as_str())),
        "value_changed" => false,
        _ => param_values_equal(source_value, reference, reference_string.as_str()),
    }
}

fn param_values_equal(value: &ParamValue, reference: f64, reference_string: &str) -> bool {
    if let Some(number) = param_numeric(value) {
        return (number - reference).abs() <= f64::EPSILON;
    }
    param_string(value) == reference_string
}

fn param_numeric(value: &ParamValue) -> Option<f64> {
    match value {
        ParamValue::Bool(value) => Some(if *value { 1.0 } else { 0.0 }),
        ParamValue::Int(value) => Some(f64::from(*value)),
        ParamValue::Float(value) => Some(*value),
        ParamValue::CssValue(value) => Some(value.value),
        ParamValue::Str(value) | ParamValue::File(value) | ParamValue::Enum(value) => value.parse().ok(),
        _ => None,
    }
}

fn param_bool(value: &ParamValue) -> Option<bool> {
    match value {
        ParamValue::Bool(value) => Some(*value),
        ParamValue::Int(value) => Some(*value != 0),
        ParamValue::Float(value) => Some(value.abs() > f64::EPSILON),
        ParamValue::CssValue(value) => Some(value.value.abs() > f64::EPSILON),
        ParamValue::Str(value) | ParamValue::File(value) | ParamValue::Enum(value) => match value.as_str() {
            "true" | "on" | "1" => Some(true),
            "false" | "off" | "0" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn param_string(value: &ParamValue) -> String {
    match value {
        ParamValue::Bool(value) => value.to_string(),
        ParamValue::Int(value) => value.to_string(),
        ParamValue::Float(value) => value.to_string(),
        ParamValue::Str(value) | ParamValue::File(value) | ParamValue::Enum(value) => value.clone(),
        ParamValue::CssValue(value) => value.value.to_string(),
        ParamValue::Vec2(x, y) => format!("{x},{y}"),
        ParamValue::Vec3(x, y, z) => format!("{x},{y},{z}"),
        ParamValue::Color(r, g, b, a) => format!("{r},{g},{b},{a}"),
        ParamValue::Reference(reference) => reference.uuid().0.to_string(),
        ParamValue::Trigger() => String::new(),
    }
}

fn input_manager_has_dirty_source(
    snapshot: &ProcessTreeSnapshot,
    manager: NodeId,
    dirty_input_source_params: &HashSet<NodeUuid>,
) -> bool {
    snapshot.child_ids(manager).into_iter().any(|input| {
        snapshot.node(input).is_some_and(|node| node.enabled)
            && child_reference_uuid(snapshot, input, "source")
                .is_some_and(|source| dirty_input_source_params.contains(&source))
    })
}

fn processor_input_manager_value_set(
    snapshot: &ProcessTreeSnapshot,
    manager: NodeId,
    logical_tick: u64,
) -> ValueSet {
    let mut value_set = ValueSet::new(logical_tick);
    for input in snapshot.child_ids(manager) {
        let Some(input_node) = snapshot.node(input) else {
            continue;
        };
        if !input_node.enabled {
            continue;
        }
        let Some(source_uuid) = child_reference_uuid(snapshot, input, "source") else {
            continue;
        };
        let Some(source_id) = snapshot.node_id_by_uuid(source_uuid) else {
            continue;
        };
        let Some(source_node) = snapshot.node(source_id) else {
            continue;
        };
        let Some(value) = source_node.param_value.as_ref().and_then(param_to_runtime_value) else {
            continue;
        };
        let Ok(key) = ValueLaneKey::new(input_node.uuid.0.to_string()) else {
            continue;
        };
        value_set.push(
            ValueSetEntry::new(key, source_node.label.clone(), value).with_source(StableRef::new(
                ValueTypeId::new("parameter"),
                source_uuid.0.to_string(),
            )),
        );
    }
    value_set
}

fn processor_override_value<'a>(
    snapshot: &'a ProcessTreeSnapshot,
    property: NodeId,
) -> Option<&'a ParamValue> {
    snapshot
        .node(property)
        .and_then(|node| node.param_value.as_ref())
        .or_else(|| child_param(snapshot, property, "value"))
}

fn child_param<'a>(
    snapshot: &'a ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
) -> Option<&'a ParamValue> {
    snapshot
        .find_child_by_decl_id(parent, decl_id)
        .and_then(|param| snapshot.node(param))
        .and_then(|node| node.param_value.as_ref())
}

fn child_string(snapshot: &ProcessTreeSnapshot, parent: NodeId, decl_id: &str) -> Option<String> {
    match child_param(snapshot, parent, decl_id)? {
        ParamValue::Str(value) | ParamValue::Enum(value) => Some(value.clone()),
        _ => None,
    }
}

fn child_bool(snapshot: &ProcessTreeSnapshot, parent: NodeId, decl_id: &str) -> Option<bool> {
    child_param(snapshot, parent, decl_id).and_then(param_bool)
}

fn node_matches_decl_id(actual: &str, expected: &str) -> bool {
    actual == expected || actual.rsplit('/').next() == Some(expected)
}

fn child_reference_uuid(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
) -> Option<NodeUuid> {
    child_param(snapshot, parent, decl_id).and_then(|value| match value {
        ParamValue::Reference(reference) => Some(reference.uuid()),
        _ => None,
    })
}

fn param_to_runtime_value(value: &ParamValue) -> Option<RuntimeValue> {
    match value {
        ParamValue::Bool(value) => Some(RuntimeValue::Bool(*value)),
        ParamValue::Int(value) => Some(RuntimeValue::Int(i64::from(*value))),
        ParamValue::Float(value) => Some(RuntimeValue::Float(*value)),
        ParamValue::Str(value) => Some(RuntimeValue::String(value.clone().into())),
        ParamValue::File(value) => Some(RuntimeValue::String(value.clone().into())),
        ParamValue::Enum(value) => Some(RuntimeValue::String(value.clone().into())),
        ParamValue::CssValue(value) => Some(RuntimeValue::Float(value.value)),
        ParamValue::Vec2(x, y) => Some(RuntimeValue::Vec2([*x, *y])),
        ParamValue::Vec3(x, y, z) => Some(RuntimeValue::Vec3([*x, *y, *z])),
        ParamValue::Color(r, g, b, a) => Some(RuntimeValue::Color(golden_alchemist::ColorValue {
            red: *r as f32,
            green: *g as f32,
            blue: *b as f32,
            alpha: *a as f32,
        })),
        ParamValue::Reference(reference) => Some(RuntimeValue::String(reference.uuid().0.to_string().into())),
        ParamValue::Trigger() => Some(RuntimeValue::Trigger(TriggerValue::default())),
    }
}

fn runtime_value_label(value: &RuntimeValue) -> String {
    match value {
        RuntimeValue::Unit => "unit".to_string(),
        RuntimeValue::Bool(value) => value.to_string(),
        RuntimeValue::Int(value) => value.to_string(),
        RuntimeValue::Float(value) => format!("{value:.3}"),
        RuntimeValue::String(value) => value.to_string(),
        RuntimeValue::Trigger(value) => {
            if value.fired {
                "trigger fired".to_string()
            } else {
                "trigger idle".to_string()
            }
        }
        other => format!("{other:?}"),
    }
}
