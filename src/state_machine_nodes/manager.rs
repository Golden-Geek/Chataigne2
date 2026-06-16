use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use chataigne_state_machine::{
    ANodeOutputPreviewSampleDto, ContextKeyDto, DefaultProcessorContextProvider,
    Processor, ProcessorDebugCapture, ProcessorLaneSummaryDto,
    ProcessorLifecycleEvent, ProcessorLifecyclePolicy, ProcessorRuntime,
    StateMachineProtocolBundle, processor_output_preview_samples,
    alchemist::{node_registry, value_type_registry},
};
use golden_alchemist::{
    ANodeId, AlchemistFormula, CompiledAlchemistFormula, EvaluationCtx,
    DebugValueSample, OutputPreviewStatus, RuntimeInputSnapshot, RuntimeIntent,
    RuntimeRegistries, RuntimeValue, SocketId, TriggerValue,
    SurfaceItemId,
};
use golden_core::{
    engine::NodeExecutionRule,
    log,
    node,
    node::{
        Node, NodeCreationContext, NodeId, NodeMetaPatch, NodeUserPermissions,
        NodeUuid, UserContainerRules, UserCreatableItem,
    },
    parameter::ParamValue,
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

pub(crate) const STATE_ITEM_KIND: &str = "state";
const STATE_NODE_TYPE: &str = "state";
const FORMULA_LIBRARY_NODE_TYPE: &str = "alchemist_formula_library";
const FORMULA_NODE_TYPE: &str = "alchemist_formula";
const ANODE_NODE_TYPE: &str = "alchemist_anode";
const PROCESSOR_MANAGER_DECL_ID: &str = "processors";
const PROCESSOR_NODE_TYPE: &str = "state_processor";
const PROCESSOR_FOLDER_NODE_TYPE: &str = "state_processor_folder";
const PROCESSOR_PROPERTIES_DECL_ID: &str = "properties";
const STATE_MACHINE_RUNTIME_HZ: u32 = 60;
const STATE_MACHINE_RUNTIME_PREVIEW_TOPIC: &str = "chataigne.state_machine.runtime_preview";
const STATE_MACHINE_PREVIEW_CHANGED_MIN_TICKS: u64 = 1;
const STATE_MACHINE_PREVIEW_KEEPALIVE_TICKS: u64 = 60;
const STATE_MACHINE_LOG_MIN_TICKS: u64 = 30;
const STATE_MACHINE_RUNTIME_WARNING_ID: &str = "state_machine_runtime";

struct RuntimeProcessor {
    processor: Processor,
    runtime: ProcessorRuntime,
    formula_node: NodeId,
    evaluated_once: bool,
}

struct RuntimeLogRecord {
    value: String,
    tick: u64,
}

struct RuntimeFormulaDefaultPreview {
    processor: Processor,
    runtime: ProcessorRuntime,
}

struct PendingTriggerInput {
    formula: NodeUuid,
    anode: NodeUuid,
    socket: SocketId,
    value: TriggerValue,
}

struct FormulaInputValueParam {
    formula: NodeUuid,
    anode: NodeUuid,
    socket: SocketId,
    is_trigger: bool,
}

#[derive(Default)]
struct StateMachineRuntimeCache {
    dirty: bool,
    dirty_formula_values: HashSet<NodeUuid>,
    dirty_processor_overrides: HashSet<NodeId>,
    pending_trigger_inputs: Vec<PendingTriggerInput>,
    next_trigger_edge_id: u64,
    processors: HashMap<NodeId, RuntimeProcessor>,
    formula_default_previews: HashMap<String, RuntimeFormulaDefaultPreview>,
    last_preview_signature: Option<String>,
    last_preview_tick: Option<u64>,
    last_log_values: HashMap<String, RuntimeLogRecord>,
}

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
        self.runtime_cache.dirty = true;
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            self.runtime_cache.dirty = true;
            return;
        };
        ctx.add_event_listener_subtree(self.id(), snapshot.root(), 1);
        for library in formula_libraries(&snapshot) {
            ctx.add_event_listener_subtree(self.id(), library, u32::MAX);
        }
        self.runtime_cache.dirty = true;
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.run_processors(ctx);
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        true
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::periodic(STATE_MACHINE_RUNTIME_HZ)
    }

    fn on_child_added(&mut self, ctx: &mut ProcessCtx, _parent: golden_core::node::NodeId, _child: golden_core::node::NodeId) {
        self.runtime_cache.dirty = true;
        crate::app::state_machine_nodes_transition::reconcile_state_networks(ctx, None, None, None);
    }

    fn on_child_removed(
        &mut self,
        ctx: &mut ProcessCtx,
        _parent: golden_core::node::NodeId,
        _child: golden_core::node::NodeId,
    ) {
        self.runtime_cache.dirty = true;
        crate::app::state_machine_nodes_transition::reconcile_state_networks(ctx, None, None, None);
    }

    fn on_node_created(&mut self, _ctx: &mut ProcessCtx, _node: NodeId) {
        self.runtime_cache.dirty = true;
    }

    fn on_node_deleted(&mut self, _ctx: &mut ProcessCtx, _node: NodeId) {
        self.runtime_cache.dirty = true;
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        if self.mark_processor_override_dirty(ctx, param) {
            return;
        }
        if self.mark_formula_input_value_dirty(ctx, param) {
            return;
        }
        self.runtime_cache.dirty = true;
    }

    fn on_meta_changed(&mut self, _ctx: &mut ProcessCtx, _node: NodeId, _patch: NodeMetaPatch) {
        self.runtime_cache.dirty = true;
    }

    fn child_event_interest_depth(&self, _event: &golden_core::events::Event) -> u32 {
        u32::MAX
    }
}

impl StateMachineManager {
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
        self.runtime_cache.dirty = true;
        self.runtime_cache.dirty_formula_values.insert(input.formula);
        if input.is_trigger {
            let edge_id = self.runtime_cache.next_trigger_edge_id;
            self.runtime_cache.next_trigger_edge_id =
                self.runtime_cache.next_trigger_edge_id.wrapping_add(1);
            self.runtime_cache.pending_trigger_inputs.push(PendingTriggerInput {
                formula: input.formula,
                anode: input.anode,
                socket: input.socket,
                value: TriggerValue::fired(edge_id, ctx.time.tick),
            });
        }
        true
    }

    fn run_processors(&mut self, ctx: &mut ProcessCtx) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot.as_ref();
        let mut formulas = collect_formulas(snapshot);
        let pending_trigger_inputs = std::mem::take(&mut self.runtime_cache.pending_trigger_inputs);
        apply_pending_trigger_inputs(&mut formulas, pending_trigger_inputs);
        let active_states = active_state_nodes(snapshot, self.id());
        let cache_rebuilt = self.runtime_cache.dirty;

        if self.runtime_cache.dirty {
            self.rebuild_runtime_cache(ctx, snapshot, &formulas);
        } else {
            self.refresh_dirty_processor_overrides(snapshot, &formulas);
        }

        let value_types = value_type_registry();
        let registries = RuntimeRegistries {
            value_types: &value_types,
        };
        let inputs = RuntimeInputSnapshot::default();
        let eval_ctx = EvaluationCtx {
            logical_tick: ctx.time.tick,
            delta_time: ctx.delta_time,
            events: &[],
            inputs: &inputs,
            registries: &registries,
        };
        let provider = DefaultProcessorContextProvider;
        let capture = ProcessorDebugCapture::All {
            history_len: usize::MAX,
        };
        let mut output_preview = Vec::new();
        let mut previewed_formula_defaults = HashSet::new();
        let mut processor_lanes = Vec::new();
        let mut evaluated_any = false;

        for state in active_states {
            let Some(processor_manager) =
                snapshot.find_child_by_decl_id(state, PROCESSOR_MANAGER_DECL_ID)
            else {
                continue;
            };
            for processor_node in processor_nodes(snapshot, processor_manager) {
                let Some(runtime_processor) = self.runtime_cache.processors.get_mut(&processor_node)
                else {
                    continue;
                };
                if runtime_processor.evaluated_once
                    && !processor_needs_continuous_evaluation(&runtime_processor.runtime)
                {
                    continue;
                }
                let compiled_formula = runtime_processor
                    .runtime
                    .compiled
                    .as_ref()
                    .map(Arc::clone);
                let formula_id = compiled_formula
                    .as_ref()
                    .map(|compiled| compiled.formula_ref.id.clone());
                runtime_processor.runtime.apply_lifecycle(
                    &runtime_processor.processor,
                    ProcessorLifecycleEvent::ProjectStart,
                );
                let lanes = runtime_processor
                    .runtime
                    .evaluate_processor_with_context_provider_and_send_capture(
                        &runtime_processor.processor,
                        &eval_ctx,
                        &provider,
                        &capture,
                    );
                evaluated_any = true;
                runtime_processor.evaluated_once = true;
                let anode_nodes = formula_anode_node_ids(snapshot, runtime_processor.formula_node);
                for diagnostic in &runtime_processor.runtime.diagnostics {
                    if should_emit_runtime_log(
                        &mut self.runtime_cache.last_log_values,
                        ctx.time.tick,
                        format!("processor:{processor_node:?}:compile"),
                        diagnostic.message.as_str(),
                    ) {
                        log!(
                            origin = processor_node;
                            format!("Processor diagnostic: {}", diagnostic.message)
                        );
                    }
                }
                if let Some(formula_id) = formula_id.as_ref() {
                    output_preview.extend(processor_output_preview_samples(
                        runtime_processor.processor.id,
                        formula_id,
                        lanes.clone(),
                    ));
                    if previewed_formula_defaults.insert(formula_id.to_string()) {
                        let formula_uuid = snapshot
                            .node(runtime_processor.formula_node)
                            .map(|node| node.uuid);
                        if let (Some(compiled_formula), Some(formula)) = (
                            compiled_formula.as_ref().map(Arc::clone),
                            formula_uuid.and_then(|uuid| formulas.get(&uuid)),
                        )
                        {
                            output_preview.extend(formula_default_output_preview_samples(
                                &mut self.runtime_cache.formula_default_previews,
                                compiled_formula,
                                formula,
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
                            format!("processor:{processor_node:?}:runtime"),
                            diagnostic.message.as_str(),
                        ) {
                            log!(
                                origin = processor_node;
                                format!("Processor runtime diagnostic: {}", diagnostic.message)
                            );
                        }
                    }
                    for intent in &lane.output.intents {
                        if intent.kind.as_ref() != "debug.log" {
                            continue;
                        }
                        let (origin, message) = format_debug_log_intent(
                            snapshot,
                            runtime_processor.formula_node,
                            processor_node,
                            lane.context_key.as_ref(),
                            &anode_nodes,
                            intent,
                        );
                        log!(
                            origin = origin;
                            format!("{}", message)
                        );
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
                            runtime_processor.formula_node,
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
        if evaluated_any || cache_rebuilt {
            self.publish_output_preview(ctx, output_preview, processor_lanes);
        }
    }

    fn publish_output_preview(
        &mut self,
        ctx: &mut ProcessCtx,
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
            processors: Vec::new(),
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
            let Some(formula_uuid) = child_reference_uuid(snapshot, processor_node, "formula") else {
                continue;
            };
            let Some(formula_node) = snapshot.node_id_by_uuid(formula_uuid) else {
                continue;
            };
            let Some(formula) = formulas.get(&formula_uuid) else {
                continue;
            };
            let Some(processor) = processor_from_snapshot(snapshot, processor_node, formula) else {
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
            let compiled = if self
                .runtime_cache
                .dirty_formula_values
                .contains(&formula_uuid)
            {
                runtime.compile_preserving_compatible_lanes(&processor, formula, &compile_ctx)
            } else {
                runtime.compile(&processor, formula, &compile_ctx)
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
                    formula_node,
                    evaluated_once: false,
                },
            );
        }
        self.runtime_cache.processors = next_processors;
        self.runtime_cache.dirty_processor_overrides.clear();
        self.runtime_cache.dirty_formula_values.clear();
        self.runtime_cache.formula_default_previews.clear();
        self.runtime_cache.last_preview_signature = None;
        self.runtime_cache.dirty = false;
    }

    fn refresh_dirty_processor_overrides(
        &mut self,
        snapshot: &ProcessTreeSnapshot,
        formulas: &HashMap<NodeUuid, AlchemistFormula>,
    ) {
        let dirty_processors = std::mem::take(&mut self.runtime_cache.dirty_processor_overrides);
        for processor_node in dirty_processors {
            let Some(runtime_processor) = self.runtime_cache.processors.get_mut(&processor_node)
            else {
                continue;
            };
            let Some(formula_uuid) = child_reference_uuid(snapshot, processor_node, "formula")
            else {
                self.runtime_cache.dirty = true;
                continue;
            };
            let Some(formula) = formulas.get(&formula_uuid) else {
                self.runtime_cache.dirty = true;
                continue;
            };
            let Some(processor) = processor_from_snapshot(snapshot, processor_node, formula) else {
                self.runtime_cache.dirty = true;
                continue;
            };
            runtime_processor.processor = processor;
            runtime_processor.evaluated_once = false;
        }
    }
}

#[cfg(test)]
mod manager_tests;

fn collect_formulas(snapshot: &ProcessTreeSnapshot) -> HashMap<NodeUuid, AlchemistFormula> {
    let mut formulas = HashMap::new();
    for library in formula_libraries(snapshot) {
        collect_formulas_in_subtree(snapshot, library, &mut formulas);
    }
    formulas
}

fn apply_pending_trigger_inputs(
    formulas: &mut HashMap<NodeUuid, AlchemistFormula>,
    pending: Vec<PendingTriggerInput>,
) {
    for input in pending {
        let Some(formula) = formulas.get_mut(&input.formula) else {
            continue;
        };
        let anode_id = ANodeId::from_uuid(input.anode.0);
        let Some(anode) = formula.graph.nodes.get_mut(&anode_id) else {
            continue;
        };
        anode
            .input_defaults
            .insert(input.socket, RuntimeValue::Trigger(input.value));
    }
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

fn collect_formulas_in_subtree(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    formulas: &mut HashMap<NodeUuid, AlchemistFormula>,
) {
    for child in snapshot.child_ids(parent) {
        let Some(node) = snapshot.node(child) else {
            continue;
        };
        if node.node_type == FORMULA_NODE_TYPE {
            if let Ok(formula) =
                crate::app::state_machine_nodes_formula::formula_from_snapshot(snapshot, child)
            {
                formulas.insert(node.uuid, formula);
            }
        } else {
            collect_formulas_in_subtree(snapshot, child, formulas);
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
                .is_some_and(|node| node.node_type == STATE_NODE_TYPE)
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
        match node.node_type.as_str() {
            PROCESSOR_NODE_TYPE => processors.push(child),
            PROCESSOR_FOLDER_NODE_TYPE => processors.extend(processor_nodes(snapshot, child)),
            _ => {}
        }
    }
    processors
}

fn processor_for_override_change(
    snapshot: &ProcessTreeSnapshot,
    changed_node: NodeId,
) -> Option<NodeId> {
    let mut current = Some(changed_node);
    let mut properties_root = None;
    while let Some(node_id) = current {
        let node = snapshot.node(node_id)?;
        if node.node_type == PROCESSOR_NODE_TYPE {
            let processor_properties =
                snapshot.find_child_by_decl_id(node_id, PROCESSOR_PROPERTIES_DECL_ID)?;
            return (properties_root == Some(processor_properties)).then_some(node_id);
        }
        if node_matches_decl_id(node.decl_id.as_str(), PROCESSOR_PROPERTIES_DECL_ID) {
            properties_root = Some(node_id);
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
    Some(processor)
}

fn formula_default_output_preview_samples(
    cache: &mut HashMap<String, RuntimeFormulaDefaultPreview>,
    compiled: Arc<CompiledAlchemistFormula>,
    formula: &AlchemistFormula,
    ctx: &EvaluationCtx<'_>,
    provider: &DefaultProcessorContextProvider,
    capture: &ProcessorDebugCapture,
) -> Vec<chataigne_state_machine::ANodeOutputPreviewSample> {
    let key = formula.id.to_string();
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
            .compile_from_shared_formula(&entry.processor, formula, compiled)
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

fn output_preview_signature(samples: &[chataigne_state_machine::ANodeOutputPreviewSample]) -> String {
    let mut parts = samples
        .iter()
        .map(|sample| {
            format!(
                "{}:{}:{:?}:{}:{}:{}",
                sample.formula_id,
                sample
                    .processor_id
                    .map(|processor_id| processor_id.to_string())
                    .unwrap_or_default(),
                sample.context_key,
                sample.author_node_id,
                sample.output_socket,
                preview_value_signature(&sample.value)
            )
        })
        .collect::<Vec<_>>();
    parts.sort();
    parts.join("|")
}

fn preview_value_signature(value: &RuntimeValue) -> String {
    match value {
        RuntimeValue::Trigger(trigger) => format!(
            "trigger:{}:{}:{}",
            trigger.fired, trigger.edge_id, trigger.logical_tick
        ),
        other => format!("{other:?}"),
    }
}

fn should_emit_runtime_log(
    last_values: &mut HashMap<String, RuntimeLogRecord>,
    tick: u64,
    key: String,
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

fn formula_anode_node_ids(
    snapshot: &ProcessTreeSnapshot,
    formula_node: NodeId,
) -> HashMap<ANodeId, NodeId> {
    snapshot
        .child_ids(formula_node)
        .into_iter()
        .filter_map(|child| {
            let node = snapshot.node(child)?;
            (node.node_type == "alchemist_anode").then(|| (ANodeId::from_uuid(node.uuid.0), child))
        })
        .collect()
}

fn anode_logs_runtime_value(snapshot: &ProcessTreeSnapshot, anode_node: NodeId) -> bool {
    let anode_type = snapshot
        .node(anode_node)
        .and_then(|node| tagged_value(&node.tags, "alchemist.anode.type:"))
        .unwrap_or_default();
    matches!(anode_type, "debug_value")
}

fn format_debug_log_intent(
    snapshot: &ProcessTreeSnapshot,
    formula_node: NodeId,
    processor_node: NodeId,
    context_key: Option<&golden_alchemist::ContextKey>,
    anode_nodes: &HashMap<ANodeId, NodeId>,
    intent: &RuntimeIntent,
) -> (NodeId, String) {
    let context = runtime_processing_context_label(snapshot, formula_node, processor_node, context_key);
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
    formula_node: NodeId,
    processor_node: NodeId,
    context_key: Option<&golden_alchemist::ContextKey>,
    anode_node: NodeId,
    sample: &DebugValueSample,
) -> String {
    let context = runtime_processing_context_label(snapshot, formula_node, processor_node, context_key);
    let node_label = snapshot_node_label(snapshot, anode_node, "ANode");
    let output_label = anode_output_label(snapshot, anode_node, &sample.output_socket);
    let value = runtime_value_label(&sample.value);
    format!("{context} | {node_label} / {output_label} = {value}")
}

fn runtime_processing_context_label(
    snapshot: &ProcessTreeSnapshot,
    formula_node: NodeId,
    processor_node: NodeId,
    context_key: Option<&golden_alchemist::ContextKey>,
) -> String {
    format!(
        "Formula: {} / Processor: {} / Lane: {}",
        snapshot_node_label(snapshot, formula_node, "Formula"),
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
    let Some(properties) = snapshot.find_child_by_decl_id(processor_node, PROCESSOR_PROPERTIES_DECL_ID)
    else {
        return;
    };
    collect_processor_property_overrides(snapshot, properties, processor);
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
        ParamValue::Color(r, g, b, a) => Some(RuntimeValue::Color(golden_alchemist::ColorValue {
            red: *r as f32,
            green: *g as f32,
            blue: *b as f32,
            alpha: *a as f32,
        })),
        _ => None,
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
