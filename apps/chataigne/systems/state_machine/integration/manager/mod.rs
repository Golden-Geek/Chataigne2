use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    sync::{Arc, LazyLock},
};

use chataigne_state_machine::{
    ANodeOutputPreviewSample, ANodeOutputPreviewSampleDto, ContextKeyDto, ContextKeyPartDto,
    DefaultProcessorContextProvider, Processor, ProcessorBindingAnalysis, ProcessorDebugCapture,
    ProcessorContextPropertyBinding, ProcessorContextProvider, ProcessorFormulaUiState, ProcessorId,
    ProcessorLaneConditionPreviewDto, ProcessorLaneInspectionDto, ProcessorLaneParameterPreviewDto,
    ProcessorLaneSummaryDto, ProcessorUiDto, ProcessorLifecycleEvent, ProcessorLifecyclePolicy,
    ProcessorRuntime, StateMachineProtocolBundle, processor_output_preview_samples,
    ValueLaneKey, ValueSet, ValueSetEntry, lane_scoped_stable_ref,
    alchemist::{CONDITIONS_MANAGER_TYPE, INPUTS_MANAGER_TYPE, node_registry, value_type_registry},
};
use chataigne_alchemist::{
    ANodeId, AlchemistFormula, AlchemistGraphDomain, AxisSet, CompiledAlchemistFormula, ContextAxisId, ContextItemId,
    ContextKey, ContextKeyPart, ContextValuePath, EvaluationCtx, DebugValueSample, FormulaCompileKey, FormulaRef,
    ManagedItemId, ManagedItemInstance,
    ManagedItemUiState, ManagedRegionInstance, OutputPreviewStatus, RuntimeInputSnapshot,
    RuntimeIntent, RuntimeRegistries, SignatureCtx, SocketId, StableRef,
    SurfaceItemId, TriggerValue, ValueTypeId, compile_graph, formula_input_value_ref,
};

use crate::app::systems_alchemist_conditions::compiler::{
    CompiledManagerCondition, ConditionBinding, compile_manager_condition,
    param_to_condition_value,
};
use chataigne_condition::{ConditionEvaluationFrame, ConditionInputProvider, ConditionRuntime};
use golden_core::{
    contexts::{
        MultiplexContextLinkTarget, MultiplexTemplateToken, MultiplexTokenSelector,
        UserContextValueType, parse_multiplex_context_link_symbol,
        parse_multiplex_template_token,
    },
    engine::NodeExecutionRule,
    events::{Event, EventFrame, EventKind},
    log,
    node,
    node::{
        Node, NodeCreationContext, NodeId, NodeMetaPatch, NodeUserPermissions,
        NodeUuid, UserContainerRules, UserCreatableItem, USER_CONTEXT_MULTIPLEX_COUNT_DECL_ID,
        USER_CONTEXT_MULTIPLEX_NODE_TYPE, USER_CONTEXT_NODE_TYPE,
        user_context_multiplex_list_value_type,
    },
    parameter::{
        ParamValue, ParameterControlMode, ParameterControlSpec, ParameterControlState,
        coerce_param_value_for_target, project_param_value,
    },
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};
use golden_values::Value as RuntimeValue;

use crate::app::systems_alchemist_formula::{
    anode_from_snapshot, constraint_value_type, formula_from_snapshot, local_signature_bindings,
    param_to_runtime_value as formula_param_to_runtime_value, runtime_value_to_param,
    ANODE_NODE_TYPE, FORMULA_EXTERNAL_READ_ONLY_TAG,
};
use crate::app::systems_alchemist_processor::{
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

struct ProcessorRuntimeInputContext<'a> {
    snapshot: &'a ProcessTreeSnapshot,
    processor_node: NodeId,
    processor_id: ProcessorId,
    logical_tick: u64,
    dirty_input_source_params: &'a HashSet<NodeUuid>,
    formula_input_values: &'a HashMap<StableRef, RuntimeValue>,
    context_provider: &'a SnapshotProcessorContextProvider,
    force_processor_recompute: bool,
    input_manager_signal_ticks: &'a mut HashMap<String, u64>,
    condition_manager_values: &'a mut HashMap<String, RuntimeValue>,
    condition_manager_valid_states: &'a mut HashMap<String, bool>,
    condition_manager_axes: &'a mut HashMap<NodeId, AxisSet>,
    compiled_conditions: &'a mut HashMap<NodeId, CompiledManagerCondition>,
    condition_runtimes: &'a mut HashMap<String, ConditionRuntime>,
    settled_condition_runtimes: &'a mut HashMap<String, ConditionRuntime>,
    condition_observations: &'a mut HashMap<String, Vec<ProcessorLaneConditionPreviewDto>>,
    transient_condition_valid_resets: &'a mut HashMap<NodeId, u64>,
    next_trigger_edge_id: &'a mut u64,
    ctx: &'a mut ProcessCtx,
}

#[derive(Clone, Debug, Default)]
struct SnapshotProcessorContextProvider {
    processors: HashMap<ProcessorId, ProcessorContextRuntime>,
}

#[derive(Clone, Debug, Default)]
struct ProcessorContextRuntime {
    axes: Vec<ProcessorContextAxisRuntime>,
    lists: Vec<ProcessorContextListRuntime>,
}

#[derive(Clone, Debug)]
struct ProcessorContextAxisRuntime {
    axis: ContextAxisId,
    name: String,
    items: Vec<ContextItemId>,
}

#[derive(Clone, Debug)]
struct ProcessorContextListRuntime {
    axis: ContextAxisId,
    symbol: String,
    list_id: String,
    entries: Vec<ProcessorContextEntryRuntime>,
}

#[derive(Clone, Debug)]
struct ProcessorContextEntryRuntime {
    item: ContextItemId,
    value: RuntimeValue,
}

impl SnapshotProcessorContextProvider {
    fn from_snapshot(
        snapshot: &ProcessTreeSnapshot,
        processor_nodes: impl IntoIterator<Item = NodeId>,
    ) -> Self {
        let mut provider = Self::default();
        for processor_node in processor_nodes {
            let Some(processor_snapshot) = snapshot.node(processor_node) else {
                continue;
            };
            let processor_id = ProcessorId::from_uuid(processor_snapshot.uuid.0);
            let runtime = collect_processor_context_runtime(snapshot, processor_node);
            provider.processors.insert(processor_id, runtime);
        }
        provider
    }

    fn multiplex_link_for_symbol(
        &self,
        processor_id: ProcessorId,
        symbol: &str,
    ) -> Option<(ContextAxisId, ContextValuePath)> {
        let symbol = symbol.trim();
        if symbol.is_empty() {
            return None;
        }
        let runtime = self.processors.get(&processor_id)?;
        if let Some(target) = parse_multiplex_context_link_symbol(symbol) {
            return match target {
                MultiplexContextLinkTarget::Index { axis_id, zero_based } => runtime
                    .axes
                    .iter()
                    .find(|axis| axis.axis.as_str() == axis_id)
                    .map(|axis| {
                        let selector = if zero_based { "@index0" } else { "@index" };
                        (axis.axis.clone(), ContextValuePath::new([selector]))
                    }),
                MultiplexContextLinkTarget::List { axis_id, symbol } => runtime
                    .lists
                    .iter()
                    .find(|list| list.axis.as_str() == axis_id && list.symbol == symbol)
                    .map(|list| {
                        (
                            list.axis.clone(),
                            ContextValuePath::new([list.symbol.as_str()]),
                        )
                    }),
            };
        }
        runtime
            .lists
            .iter()
            .find(|list| list.symbol == symbol || list.list_id == symbol)
            .map(|list| {
                (
                    list.axis.clone(),
                    ContextValuePath::new([list.symbol.as_str()]),
                )
            })
    }

    fn template_axes(&self, processor_id: ProcessorId, template: &str) -> AxisSet {
        let Some(runtime) = self.processors.get(&processor_id) else {
            return AxisSet::new();
        };
        multiplex_template_tokens(template)
            .filter_map(|token| runtime.axis_for_template_token(&token))
            .map(|axis| axis.axis.clone())
            .collect()
    }

    fn context_key_dto(&self, processor_id: ProcessorId, key: &ContextKey) -> ContextKeyDto {
        let runtime = self.processors.get(&processor_id);
        ContextKeyDto {
            parts: key
                .iter()
                .map(|part| {
                    let axis = runtime.and_then(|runtime| {
                        runtime.axes.iter().find(|axis| axis.axis == part.axis)
                    });
                    let index = axis.and_then(|axis| {
                        axis.items.iter().position(|item| item == &part.item)
                    });
                    let axis_label = axis
                        .map(|axis| axis.name.trim())
                        .filter(|label| !label.is_empty())
                        .unwrap_or_else(|| part.axis.as_str())
                        .to_owned();
                    ContextKeyPartDto {
                        axis_id: part.axis.as_str().to_owned(),
                        axis_label,
                        item_id: part.item.as_str().to_owned(),
                        item_label: index
                            .map(|index| format!("#{}", index + 1))
                            .unwrap_or_else(|| part.item.as_str().to_owned()),
                        index: index.and_then(|index| u32::try_from(index).ok()),
                    }
                })
                .collect(),
        }
    }

    fn context_key_label(&self, processor_id: ProcessorId, key: &ContextKey) -> String {
        self.context_key_dto(processor_id, key)
            .parts
            .into_iter()
            .map(|part| format!("{} {}", part.axis_label, part.item_label))
            .collect::<Vec<_>>()
            .join(" × ")
    }

    fn resolve_template_token(
        &self,
        processor_id: ProcessorId,
        key: &ContextKey,
        token: &MultiplexTemplateToken,
    ) -> Option<String> {
        let runtime = self.processors.get(&processor_id)?;
        let axis = runtime.axis_for_template_token(token)?;
        let item = key
            .iter()
            .find(|part| part.axis == axis.axis)
            .map(|part| &part.item)?;
        match token {
            MultiplexTemplateToken::Index { zero_based, .. } => {
                let index = axis.items.iter().position(|candidate| candidate == item)?;
                Some((index + usize::from(!zero_based)).to_string())
            }
            MultiplexTemplateToken::List { list, .. } => {
                let value = runtime
                    .lists
                    .iter()
                    .find(|candidate| candidate.axis == axis.axis && candidate.symbol == *list)?
                    .entries
                    .iter()
                    .find(|entry| entry.item == *item)?
                    .value
                    .clone();
                let value = runtime_value_to_param(&value).ok()?;
                Some(value.as_str().unwrap_or_else(|| value.to_string()))
            }
        }
    }

    fn lane_count_for_axes(&self, processor_id: ProcessorId, axes: &AxisSet) -> usize {
        if axes.is_empty() {
            return 0;
        }
        self.iter_context_keys(processor_id, axes).count()
    }
}

impl ProcessorContextProvider for SnapshotProcessorContextProvider {
    fn available_axes(&self, processor_id: ProcessorId) -> AxisSet {
        let mut axes = AxisSet::new();
        if let Some(runtime) = self.processors.get(&processor_id) {
            axes.extend(runtime.axes.iter().map(|axis| axis.axis.clone()));
        }
        axes
    }

    fn iter_context_keys<'a>(
        &'a self,
        processor_id: ProcessorId,
        axes: &'a AxisSet,
    ) -> Box<dyn Iterator<Item = ContextKey> + 'a> {
        if axes.is_empty() {
            return Box::new(std::iter::once(ContextKey::default_lane()));
        }
        let Some(runtime) = self.processors.get(&processor_id) else {
            return Box::new(std::iter::empty());
        };

        let mut required_axes = Vec::<(&ContextAxisId, Vec<ContextItemId>)>::new();
        for axis in axes {
            let Some(runtime_axis) = runtime.axes.iter().find(|candidate| &candidate.axis == axis)
            else {
                return Box::new(std::iter::empty());
            };
            if runtime_axis.items.is_empty() {
                return Box::new(std::iter::empty());
            }
            required_axes.push((axis, runtime_axis.items.clone()));
        }

        let mut key_parts = vec![Vec::<ContextKeyPart>::new()];
        for (axis, items) in required_axes {
            let mut next = Vec::<Vec<ContextKeyPart>>::new();
            for prefix in &key_parts {
                for item in &items {
                    let mut parts = prefix.clone();
                    parts.push(ContextKeyPart::new(axis.clone(), item.clone()));
                    next.push(parts);
                }
            }
            key_parts = next;
        }

        Box::new(key_parts.into_iter().map(ContextKey::new))
    }

    fn resolve_context_value(
        &self,
        key: &ContextKey,
        axis: &ContextAxisId,
        path: &ContextValuePath,
    ) -> Option<RuntimeValue> {
        let item = key
            .iter()
            .find(|part| &part.axis == axis)
            .map(|part| &part.item)?;
        let selector = path.segments.first().map(|segment| segment.as_str());

        if matches!(selector, Some("@index" | "@index0")) {
            let runtime = self.processors.values().find(|runtime| {
                runtime.axes.iter().any(|candidate| &candidate.axis == axis)
            })?;
            let runtime_axis = runtime.axes.iter().find(|candidate| &candidate.axis == axis)?;
            let index = runtime_axis.items.iter().position(|candidate| candidate == item)?;
            let offset = usize::from(selector == Some("@index"));
            return Some(RuntimeValue::Int((index + offset) as i64));
        }

        self.processors
            .values()
            .flat_map(|runtime| runtime.lists.iter())
            .filter(|list| &list.axis == axis)
            .find(|list| {
                selector.is_none_or(|selector| {
                    list.symbol == selector || list.list_id == selector
                })
            })?
            .entries
            .iter()
            .find(|entry| &entry.item == item)
            .map(|entry| entry.value.clone())
    }
}

impl ProcessorContextRuntime {
    fn axis_for_template_token(
        &self,
        token: &MultiplexTemplateToken,
    ) -> Option<&ProcessorContextAxisRuntime> {
        let selector = match token {
            MultiplexTemplateToken::Index { multiplex, .. }
            | MultiplexTemplateToken::List { multiplex, .. } => multiplex,
        };
        match selector {
            MultiplexTokenSelector::First => self.axes.first(),
            MultiplexTokenSelector::Ordinal(ordinal) => self.axes.get(ordinal.saturating_sub(1)),
            MultiplexTokenSelector::Name(name) => self
                .axes
                .iter()
                .find(|axis| axis.name.eq_ignore_ascii_case(name)),
        }
    }
}

fn multiplex_template_tokens(template: &str) -> impl Iterator<Item = MultiplexTemplateToken> + '_ {
    template
        .split('{')
        .skip(1)
        .filter_map(|suffix| suffix.split_once('}').map(|(token, _)| token))
        .filter_map(parse_multiplex_template_token)
}

fn collect_processor_context_runtime(
    snapshot: &ProcessTreeSnapshot,
    processor_node: NodeId,
) -> ProcessorContextRuntime {
    let mut runtime = ProcessorContextRuntime::default();
    let mut cursor = Some(processor_node);
    while let Some(owner) = cursor {
        for child in snapshot.child_ids(owner) {
            let Some(child_snapshot) = snapshot.node(child) else {
                continue;
            };
            if child_snapshot.node_type == USER_CONTEXT_NODE_TYPE {
                collect_context_scope_multiplexes(snapshot, child, &mut runtime);
            }
        }
        cursor = snapshot.node(owner).and_then(|node| node.parent);
    }
    runtime
}

fn collect_context_scope_multiplexes(
    snapshot: &ProcessTreeSnapshot,
    scope_node: NodeId,
    runtime: &mut ProcessorContextRuntime,
) {
    let mut stack = snapshot.child_ids(scope_node);
    stack.reverse();
    while let Some(node_id) = stack.pop() {
        let Some(node) = snapshot.node(node_id) else {
            continue;
        };
        if node.node_type == USER_CONTEXT_NODE_TYPE {
            continue;
        }
        if node.node_type == USER_CONTEXT_MULTIPLEX_NODE_TYPE {
            collect_multiplex_lists(snapshot, node_id, runtime);
            continue;
        }
        let mut children = snapshot.child_ids(node_id);
        children.reverse();
        stack.extend(children);
    }
}

fn collect_multiplex_lists(
    snapshot: &ProcessTreeSnapshot,
    multiplex_node: NodeId,
    runtime: &mut ProcessorContextRuntime,
) {
    let Some(multiplex_snapshot) = snapshot.node(multiplex_node) else {
        return;
    };
    let axis = ContextAxisId::new(multiplex_snapshot.uuid.0.to_string());
    let mut pending_lists = Vec::<PendingProcessorContextList>::new();
    let mut canonical_items = Vec::<ContextItemId>::new();

    for list_node in snapshot.child_ids(multiplex_node) {
        let Some(list_snapshot) = snapshot.node(list_node) else {
            continue;
        };
        if list_snapshot
            .decl_id
            .eq_ignore_ascii_case(USER_CONTEXT_MULTIPLEX_COUNT_DECL_ID)
        {
            continue;
        }
        let Some(value_type_id) =
            user_context_multiplex_list_value_type(list_snapshot.node_type.as_str())
        else {
            continue;
        };
        let Some(value_type) = UserContextValueType::from_parameter_node_type(value_type_id) else {
            continue;
        };
        let symbol = list_snapshot.decl_id.trim().to_string();
        if symbol.is_empty() {
            continue;
        }

        let own_entries = collect_multiplex_list_entry_values(snapshot, list_node, value_type_id, value_type);
        if canonical_items.is_empty() {
            canonical_items.extend(
                own_entries
                    .iter()
                    .map(|entry| ContextItemId::new(entry.item_id.clone())),
            );
        }
        pending_lists.push(PendingProcessorContextList {
            symbol,
            list_id: list_snapshot.uuid.0.to_string(),
            entries: own_entries,
        });
    }

    if canonical_items.is_empty() {
        return;
    }
    if runtime.axes.iter().all(|existing| existing.axis != axis) {
        runtime.axes.push(ProcessorContextAxisRuntime {
            axis: axis.clone(),
            name: multiplex_snapshot.label.trim().to_owned(),
            items: canonical_items.clone(),
        });
    }

    for pending in pending_lists {
        let entries = pending
            .entries
            .into_iter()
            .enumerate()
            .filter_map(|(index, pending_entry)| {
                let value = pending_entry.value?;
                let item = canonical_items
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| ContextItemId::new(pending_entry.item_id));
                Some(ProcessorContextEntryRuntime { item, value })
            })
            .collect::<Vec<_>>();
        runtime.lists.push(ProcessorContextListRuntime {
            axis: axis.clone(),
            symbol: pending.symbol,
            list_id: pending.list_id,
            entries,
        });
    }
}

struct PendingProcessorContextList {
    symbol: String,
    list_id: String,
    entries: Vec<PendingProcessorContextEntry>,
}

struct PendingProcessorContextEntry {
    item_id: String,
    value: Option<RuntimeValue>,
}

fn collect_multiplex_list_entry_values(
    snapshot: &ProcessTreeSnapshot,
    list_node: NodeId,
    value_type_id: &str,
    value_type: UserContextValueType,
) -> Vec<PendingProcessorContextEntry> {
    let mut entries = Vec::new();
    for entry_node in snapshot.child_ids(list_node) {
        let Some(entry_snapshot) = snapshot.node(entry_node) else {
            continue;
        };
        if !entry_snapshot.is_parameter()
            || !entry_snapshot
                .node_type
                .eq_ignore_ascii_case(value_type_id)
        {
            continue;
        }
        let item_id = entry_snapshot.uuid.0.to_string();
        let value = entry_snapshot
            .param_value
            .as_ref()
            .and_then(|param_value| context_entry_runtime_value(snapshot, param_value, value_type));
        entries.push(PendingProcessorContextEntry { item_id, value });
    }
    entries
}

fn context_entry_runtime_value(
    snapshot: &ProcessTreeSnapshot,
    value: &ParamValue,
    value_type: UserContextValueType,
) -> Option<RuntimeValue> {
    if value_type == UserContextValueType::Reference {
        return reference_context_entry_runtime_value(snapshot, value);
    }
    runtime_value_type_for_context(value_type)
        .as_ref()
        .and_then(|runtime_type| formula_param_to_runtime_value(value, runtime_type).ok())
}

fn reference_context_entry_runtime_value(
    snapshot: &ProcessTreeSnapshot,
    value: &ParamValue,
) -> Option<RuntimeValue> {
    let ParamValue::Reference(reference) = value else {
        return None;
    };
    let target = snapshot.node_id_by_uuid(reference.uuid())?;
    let target_value = snapshot.node(target)?.param_value.as_ref()?;
    let projected_value = reference
        .projection()
        .and_then(|projection| project_param_value(target_value, projection));
    param_to_runtime_value(projected_value.as_ref().unwrap_or(target_value))
}

fn runtime_value_type_for_context(value_type: UserContextValueType) -> Option<ValueTypeId> {
    match value_type {
        UserContextValueType::Trigger => Some(ValueTypeId::new("trigger")),
        UserContextValueType::Int => Some(ValueTypeId::new("int")),
        UserContextValueType::Float => Some(ValueTypeId::new("float")),
        UserContextValueType::Str | UserContextValueType::File | UserContextValueType::Enum => {
            Some(ValueTypeId::new("string"))
        }
        UserContextValueType::Bool => Some(ValueTypeId::new("bool")),
        UserContextValueType::CssValue => Some(ValueTypeId::new("float")),
        UserContextValueType::Vec2 => Some(ValueTypeId::new("vec2")),
        UserContextValueType::Vec3 => Some(ValueTypeId::new("vec3")),
        UserContextValueType::Color => Some(ValueTypeId::new("color")),
        UserContextValueType::Reference => None,
    }
}

fn processor_binding_analysis(
    snapshot: &ProcessTreeSnapshot,
    processor_node: NodeId,
    processor_id: ProcessorId,
    provider: &SnapshotProcessorContextProvider,
) -> ProcessorBindingAnalysis {
    let mut analysis = ProcessorBindingAnalysis::default();
    collect_processor_context_link_axes(
        snapshot,
        processor_node,
        processor_id,
        provider,
        &mut analysis.input_axes,
    );
    analysis
}

fn collect_processor_context_link_axes(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    processor_id: ProcessorId,
    provider: &SnapshotProcessorContextProvider,
    axes: &mut AxisSet,
) {
    for child in snapshot.child_ids(parent) {
        let Some(node) = snapshot.node(child) else {
            continue;
        };
        if !node.enabled {
            continue;
        }
        if node.node_type == USER_CONTEXT_NODE_TYPE
            || node_matches_decl_id(node.decl_id.as_str(), PROCESSOR_MANAGED_REGIONS_DECL_ID)
        {
            continue;
        }
        if node.is_parameter() {
            axes.extend(context_control_multiplex_axes(
                snapshot,
                child,
                processor_id,
                provider,
            ));
        }
        if node.child_count > 0 {
            collect_processor_context_link_axes(snapshot, child, processor_id, provider, axes);
        }
    }
}

fn context_control_multiplex_axes(
    snapshot: &ProcessTreeSnapshot,
    param: NodeId,
    processor_id: ProcessorId,
    provider: &SnapshotProcessorContextProvider,
) -> AxisSet {
    let Some(control) = snapshot.node(param).and_then(|node| node.param_control.as_ref()) else {
        return AxisSet::new();
    };
    match (&control.mode, &control.spec) {
        (ParameterControlMode::ContextLink, ParameterControlSpec::ContextLink { symbol, .. }) => {
            provider
                .multiplex_link_for_symbol(processor_id, symbol)
                .map(|(axis, _)| AxisSet::from([axis]))
                .unwrap_or_default()
        }
        (ParameterControlMode::TemplateText, ParameterControlSpec::TemplateText { template }) => {
            provider.template_axes(processor_id, template)
        }
        _ => AxisSet::new(),
    }
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
    formula_id: chataigne_alchemist::FormulaId,
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
    logical_tick: u64,
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
    Color([u64; 4]),
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
    condition_manager_axes: HashMap<NodeId, AxisSet>,
    compiled_conditions: HashMap<NodeId, CompiledManagerCondition>,
    condition_runtimes: HashMap<String, ConditionRuntime>,
    settled_condition_runtimes: HashMap<String, ConditionRuntime>,
    condition_observations: HashMap<String, Vec<ProcessorLaneConditionPreviewDto>>,
    transient_condition_valid_resets: HashMap<NodeId, u64>,
    next_trigger_edge_id: u64,
    processors: HashMap<NodeId, RuntimeProcessor>,
    formula_default_previews: HashMap<chataigne_alchemist::FormulaId, RuntimeFormulaDefaultPreview>,
    output_preview_snapshot: HashMap<OutputPreviewSampleKey, ANodeOutputPreviewSample>,
    processor_lane_snapshot: HashMap<String, ProcessorLaneSummaryDto>,
    processor_lane_inspection_snapshot: HashMap<String, ProcessorLaneInspectionDto>,
    last_preview_signature: Option<OutputPreviewSignature>,
    last_preview_tick: Option<u64>,
    last_log_values: HashMap<RuntimeLogKey, RuntimeLogRecord>,
    perf_stats: StateMachineRuntimePerfStats,
}

fn runtime_output_previews_enabled(value: Option<&OsStr>) -> bool {
    value.is_none_or(|value| {
        let value = value.to_string_lossy();
        !matches!(value.trim().to_ascii_lowercase().as_str(), "" | "0" | "false" | "off")
    })
}

// Runtime graph feedback is product functionality. Headless/performance runs
// may explicitly opt out, but a normal app launch must feed the editor.
static RUNTIME_OUTPUT_PREVIEWS_ENABLED: LazyLock<bool> = LazyLock::new(|| {
    runtime_output_previews_enabled(std::env::var_os("CHATAIGNE_RUNTIME_OUTPUT_PREVIEWS").as_deref())
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
            || !self.runtime_cache.transient_condition_valid_resets.is_empty()
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
            .with_compiled_kernel("chataigne.runtime.state-machine")
    }

    fn on_child_added(&mut self, ctx: &mut ProcessCtx, parent: golden_core::node::NodeId, child: golden_core::node::NodeId) {
        self.mark_runtime_structure_dirty(ctx, child);
        self.mark_runtime_structure_dirty(ctx, parent);
        crate::app::systems_state_machine_transition::reconcile_state_networks(ctx, None, None, None);
    }

    fn on_child_removed(
        &mut self,
        ctx: &mut ProcessCtx,
        parent: golden_core::node::NodeId,
        child: golden_core::node::NodeId,
    ) {
        self.mark_runtime_structure_dirty(ctx, child);
        self.mark_runtime_structure_dirty(ctx, parent);
        crate::app::systems_state_machine_transition::reconcile_state_networks(ctx, None, None, None);
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
        self.mark_formula_input_value_dirty(ctx, param);
    }

    fn on_param_control_changed(
        &mut self,
        ctx: &mut ProcessCtx,
        param: NodeId,
        _old_state: ParameterControlState,
        _new_state: ParameterControlState,
    ) {
        if self.mark_processor_override_dirty(ctx, param) {
            return;
        }
        if self.mark_formula_input_value_dirty(ctx, param) {
            return;
        }
        self.mark_runtime_structure_dirty(ctx, param);
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
            chataigne_alchemist::AlchemistGraphId::from_uuid(input.formula.0),
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
        let active_processor_nodes = active_states
            .iter()
            .filter_map(|state| snapshot.find_child_by_decl_id(*state, PROCESSOR_MANAGER_DECL_ID))
            .flat_map(|processor_manager| processor_nodes(snapshot, processor_manager))
            .collect::<Vec<_>>();
        let cache_rebuilt =
            self.runtime_cache.topology_dirty || !self.runtime_cache.structure_dirty.is_empty();
        let overrides_dirty = !self.runtime_cache.dirty_processor_overrides.is_empty();
        let dirty_input_source_params = self.runtime_cache.dirty_input_source_params.clone();
        let dirty_formula_values = self.runtime_cache.dirty_formula_values.clone();
        let provider = SnapshotProcessorContextProvider::from_snapshot(
            snapshot,
            active_processor_nodes.iter().copied(),
        );

        if cache_rebuilt {
            self.refresh_formula_cache(snapshot);
            let formulas = self.runtime_cache.formulas.clone();
            let catalog = FormulaCatalog::from_snapshot(snapshot);
            self.rebuild_runtime_cache(ctx, snapshot, &formulas, &catalog, &provider);
        } else if overrides_dirty {
            let formulas = self.runtime_cache.formulas.clone();
            let catalog = FormulaCatalog::from_snapshot(snapshot);
            self.refresh_dirty_processor_overrides(snapshot, &formulas, &catalog, &provider);
        }
        if cache_rebuilt || overrides_dirty {
            self.refresh_source_event_listeners(ctx, snapshot, &active_states);
        }

        let value_types = value_type_registry();
        let registries = RuntimeRegistries {
            value_types: &value_types,
        };
        let default_provider = DefaultProcessorContextProvider;
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
        let mut processor_lane_inspections = Vec::new();
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
                let condition_signal_dirty = !dirty_input_source_params.is_empty();
                let Some(runtime_processor) = self.runtime_cache.processors.get(&processor_node) else {
                    continue;
                };
                let formula_value_dirty =
                    processor_formula_node_uuid(snapshot, runtime_processor)
                        .is_some_and(|uuid| dirty_formula_values.contains(&uuid));
                let should_evaluate = processor_should_evaluate(
                    processor_needs_continuous_evaluation(&runtime_processor.runtime),
                    cache_rebuilt || overrides_dirty,
                    input_signal_dirty,
                    condition_signal_dirty,
                    formula_value_dirty,
                );
                if !should_evaluate {
                    continue;
                }
                let mut input_context = ProcessorRuntimeInputContext {
                    snapshot,
                    processor_node,
                    processor_id: runtime_processor.processor.id,
                    logical_tick: ctx.time.tick,
                    dirty_input_source_params: &dirty_input_source_params,
                    formula_input_values: &self.runtime_cache.formula_input_values,
                    context_provider: &provider,
                    force_processor_recompute: cache_rebuilt || overrides_dirty,
                    input_manager_signal_ticks: &mut self
                        .runtime_cache
                        .input_manager_signal_ticks,
                    condition_manager_values: &mut self.runtime_cache.condition_manager_values,
                    condition_manager_valid_states: &mut self
                        .runtime_cache
                        .condition_manager_valid_states,
                    condition_manager_axes: &mut self.runtime_cache.condition_manager_axes,
                    compiled_conditions: &mut self.runtime_cache.compiled_conditions,
                    condition_runtimes: &mut self.runtime_cache.condition_runtimes,
                    settled_condition_runtimes: &mut self
                        .runtime_cache
                        .settled_condition_runtimes,
                    condition_observations: &mut self.runtime_cache.condition_observations,
                    transient_condition_valid_resets: &mut self
                        .runtime_cache
                        .transient_condition_valid_resets,
                    next_trigger_edge_id: &mut self.runtime_cache.next_trigger_edge_id,
                    ctx,
                };
                let inputs = processor_runtime_inputs(&mut input_context);
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
                    .evaluate_processor_with_context_provider_and_runtime_capture(
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
                                &default_provider,
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
                        &provider,
                    ));
                    processor_lane_inspections.push(processor_lane_inspection(
                        snapshot,
                        processor_node,
                        runtime_processor.processor.id,
                        lane,
                        &provider,
                        &self.runtime_cache.condition_observations,
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
                            dispatch_command_intent(
                                ctx,
                                snapshot,
                                processor_node,
                                runtime_processor.processor.id,
                                lane.context_key.as_ref(),
                                &provider,
                                intent,
                            );
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
        let processors = processor_ui_dtos(&self.runtime_cache.processors, &provider);
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
            if cache_rebuilt {
                self.runtime_cache.processor_lane_snapshot.clear();
                self.runtime_cache.processor_lane_inspection_snapshot.clear();
            }
            let active_processor_ids = processors
                .iter()
                .map(|processor| processor.id.as_str())
                .collect::<HashSet<_>>();
            self.runtime_cache
                .processor_lane_snapshot
                .retain(|_, lane| active_processor_ids.contains(lane.processor_id.as_str()));
            for lane in processor_lanes {
                self.runtime_cache
                    .processor_lane_snapshot
                    .insert(processor_lane_summary_key(&lane), lane);
            }
            let mut processor_lanes = self
                .runtime_cache
                .processor_lane_snapshot
                .values()
                .cloned()
                .collect::<Vec<_>>();
            processor_lanes.sort_by(|left, right| {
                left.processor_id
                    .cmp(&right.processor_id)
                    .then_with(|| left.label.cmp(&right.label))
            });
            self.runtime_cache
                .processor_lane_inspection_snapshot
                .retain(|_, inspection| active_processor_ids.contains(inspection.processor_id.as_str()));
            for inspection in processor_lane_inspections {
                self.runtime_cache.processor_lane_inspection_snapshot.insert(
                    processor_lane_inspection_key(&inspection),
                    inspection,
                );
            }
            let processor_lane_inspections = self
                .runtime_cache
                .processor_lane_inspection_snapshot
                .values()
                .cloned()
                .collect::<Vec<_>>();
            self.publish_output_preview(
                ctx,
                processors,
                output_preview,
                processor_lanes,
                processor_lane_inspections,
            );
        }
    }

    fn publish_output_preview(
        &mut self,
        ctx: &mut ProcessCtx,
        processors: Vec<ProcessorUiDto>,
        samples: Vec<chataigne_state_machine::ANodeOutputPreviewSample>,
        processor_lanes: Vec<ProcessorLaneSummaryDto>,
        processor_lane_inspections: Vec<ProcessorLaneInspectionDto>,
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
            processor_lane_inspections,
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
        context_provider: &SnapshotProcessorContextProvider,
    ) {
        let mut next_processors = HashMap::new();
        let value_types = value_type_registry();
        let nodes = node_registry();
        let compile_ctx = chataigne_alchemist::CompileCtx {
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
            let Some(mut processor) = processor_from_snapshot(snapshot, processor_node, &formula)
            else {
                continue;
            };
            apply_processor_context_property_bindings(
                snapshot,
                processor_node,
                processor.id,
                &mut processor,
                context_provider,
            );
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
            let bindings =
                processor_binding_analysis(snapshot, processor_node, processor.id, context_provider);
            runtime.rebuild_execution_plan(context_provider, &bindings);
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
        self.runtime_cache.condition_manager_axes.clear();
        self.runtime_cache.compiled_conditions.clear();
        self.runtime_cache.condition_observations.clear();
        self.runtime_cache.last_preview_signature = None;
        self.runtime_cache.topology_dirty = false;
        self.runtime_cache.structure_dirty.clear();
        self.runtime_cache.perf_stats.runtime_cache_rebuilds += 1;
    }

    fn shared_compiled_formula(
        &mut self,
        formula: &AlchemistFormula,
        ctx: &chataigne_alchemist::CompileCtx<'_>,
    ) -> Result<Arc<CompiledAlchemistFormula>, Vec<chataigne_alchemist::Diagnostic>> {
        let key = FormulaCompileKey::from_formula(formula, 0, 0);
        if let Some(compiled) = self.runtime_cache.compiled_formulas.get(&key) {
            return Ok(Arc::clone(compiled));
        }
        self.runtime_cache.perf_stats.formula_compiles += 1;
        let formula_ctx = chataigne_alchemist::CompileCtx {
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
        context_provider: &SnapshotProcessorContextProvider,
    ) {
        let dirty_processors = std::mem::take(&mut self.runtime_cache.dirty_processor_overrides);
        self.runtime_cache.compiled_conditions.clear();
        self.runtime_cache.condition_manager_axes.clear();
        self.runtime_cache.condition_observations.clear();
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
            let Some(mut processor) = processor_from_snapshot(snapshot, processor_node, &formula)
            else {
                self.runtime_cache.topology_dirty = true;
                continue;
            };
            apply_processor_context_property_bindings(
                snapshot,
                processor_node,
                processor.id,
                &mut processor,
                context_provider,
            );
            let bindings =
                processor_binding_analysis(snapshot, processor_node, processor.id, context_provider);
            runtime_processor
                .runtime
                .rebuild_execution_plan(context_provider, &bindings);
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
mod tests;

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
    entries.sort_by_key(|(left, _)| *left);
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
    let node = formula.graph.node(AlchemistGraphDomain::node_id(anode_id))?;
    let instance = node.data.to_instance(anode_id);
    let value_types = value_type_registry();
    let nodes = node_registry();
    let declaration = nodes.get(&instance.type_id)?;
    let signature = declaration.signature(
        &SignatureCtx {
            value_types: &value_types,
            properties: Some(&formula.properties),
        },
        &instance,
        &instance.type_bindings,
    );
    let bindings = local_signature_bindings(&signature, &instance);
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
                crate::app::systems_alchemist_formula::formula_from_snapshot(snapshot, child)
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
    ctx: &chataigne_alchemist::CompileCtx<'_>,
) -> bool {
    runtime.compile_preserving_compatible_lanes(processor, formula, ctx)
}

fn formula_default_output_preview_samples(
    cache: &mut HashMap<chataigne_alchemist::FormulaId, RuntimeFormulaDefaultPreview>,
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
    let lanes = entry.runtime.evaluate_processor_with_context_provider_and_runtime_capture(
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

fn processor_ui_dtos(
    processors: &HashMap<NodeId, RuntimeProcessor>,
    context_provider: &SnapshotProcessorContextProvider,
) -> Vec<ProcessorUiDto> {
    let mut dtos: Vec<_> = processors
        .values()
        .map(|runtime_processor| {
            let mut dto = ProcessorUiDto::from(
                &runtime_processor.processor.ui_model_with_formula_source(
                    &runtime_processor.formula,
                    runtime_processor.runtime.diagnostics.clone(),
                    runtime_processor.formula_ui,
                    Some(runtime_processor.formula_source_key.clone()),
                ),
            );
            dto.multiplex_lane_count = runtime_processor
                .runtime
                .plan
                .as_ref()
                .map(|plan| {
                    context_provider
                        .lane_count_for_axes(runtime_processor.processor.id, &plan.required_eval_axes)
                })
                .unwrap_or(0);
            dto
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
    context_provider: &SnapshotProcessorContextProvider,
) -> ProcessorLaneSummaryDto {
    let context_key = lane
        .context_key
        .as_ref()
        .map(|key| context_provider.context_key_dto(processor_id, key));
    let label = lane.context_key.as_ref().map_or_else(
        || "Default lane".to_owned(),
        |key| context_provider.context_key_label(processor_id, key),
    );
    ProcessorLaneSummaryDto {
        processor_id: processor_id.to_string(),
        context_key,
        label,
        has_memory,
        last_tick: Some(tick),
        diagnostics_count: lane.output.diagnostics.len(),
    }
}

fn processor_lane_summary_key(summary: &ProcessorLaneSummaryDto) -> String {
    format!(
        "{}:{}",
        summary.processor_id,
        context_key_dto_cache_id(summary.context_key.as_ref())
    )
}

fn processor_lane_inspection_key(inspection: &ProcessorLaneInspectionDto) -> String {
    format!(
        "{}:{}",
        inspection.processor_id,
        context_key_dto_cache_id(inspection.context_key.as_ref())
    )
}

fn context_key_dto_cache_id(context_key: Option<&ContextKeyDto>) -> String {
    context_key.map_or_else(
        || "__default__".to_owned(),
        |key| {
            key.parts
                .iter()
                .map(|part| format!("{}:{}", part.axis_id, part.item_id))
                .collect::<Vec<_>>()
                .join("|")
        },
    )
}

fn processor_lane_inspection(
    snapshot: &ProcessTreeSnapshot,
    processor_node: NodeId,
    processor_id: ProcessorId,
    lane: &chataigne_state_machine::ProcessorLaneOutput,
    context_provider: &SnapshotProcessorContextProvider,
    observations: &HashMap<String, Vec<ProcessorLaneConditionPreviewDto>>,
) -> ProcessorLaneInspectionDto {
    let resolver = lane.context_key.as_ref().map(|context_key| LaneParamResolver {
        processor_id,
        context_key,
        context_provider,
    });
    let mut parameter_values = Vec::new();
    collect_processor_lane_parameter_inspection(
        snapshot,
        processor_node,
        resolver.as_ref(),
        &mut parameter_values,
    );
    let condition_states = processor_lane_condition_observations(
        snapshot,
        processor_node,
        lane.context_key.as_ref(),
        observations,
    );
    ProcessorLaneInspectionDto {
        processor_id: processor_id.to_string(),
        context_key: lane
            .context_key
            .as_ref()
            .map(|key| context_provider.context_key_dto(processor_id, key)),
        parameter_values,
        condition_states,
    }
}

fn collect_processor_lane_parameter_inspection(
    snapshot: &ProcessTreeSnapshot,
    node_id: NodeId,
    resolver: Option<&LaneParamResolver<'_>>,
    parameter_values: &mut Vec<ProcessorLaneParameterPreviewDto>,
) {
    let Some(node) = snapshot.node(node_id) else {
        return;
    };
    if node.is_parameter() {
        if let (Some(resolver), Some(control)) = (resolver, node.param_control.as_ref()) {
            if matches!(
                control.mode,
                ParameterControlMode::ContextLink | ParameterControlMode::TemplateText
            ) {
                if let Some(value) = resolver
                    .param_value(snapshot, node_id)
                    .as_ref()
                    .and_then(param_to_runtime_value)
                {
                    parameter_values.push(ProcessorLaneParameterPreviewDto {
                        node_id: node.uuid.0.to_string(),
                        value: runtime_value_label(&value),
                    });
                }
            }
        }
        return;
    }

    for child in snapshot.child_ids(node_id) {
        collect_processor_lane_parameter_inspection(
            snapshot,
            child,
            resolver,
            parameter_values,
        );
    }
}

fn processor_lane_condition_observations(
    snapshot: &ProcessTreeSnapshot,
    processor_node: NodeId,
    context_key: Option<&ContextKey>,
    observations: &HashMap<String, Vec<ProcessorLaneConditionPreviewDto>>,
) -> Vec<ProcessorLaneConditionPreviewDto> {
    let Some(processor) = snapshot.node(processor_node) else {
        return Vec::new();
    };
    let prefix = format!("{}:", processor.uuid.0);
    let suffix = context_key.map(|key| format!(":{}", context_key_cache_id(key)));
    let mut result = observations
        .iter()
        .filter(|(key, _)| {
            let Some(manager_and_lane) = key.strip_prefix(&prefix) else {
                return false;
            };
            suffix.as_ref().map_or_else(
                || !manager_and_lane.contains(':'),
                |suffix| key.ends_with(suffix),
            )
        })
        .flat_map(|(_, values)| values.iter().cloned())
        .collect::<Vec<_>>();
    result.sort_by(|left, right| left.node_id.cmp(&right.node_id));
    result.dedup_by(|left, right| left.node_id == right.node_id);
    result
}

fn context_key_label(context_key: Option<&ContextKey>) -> String {
    context_key.map_or_else(
        || "Default lane".to_owned(),
        |key| {
            key.iter()
                .map(|part| part.item.as_str())
                .collect::<Vec<_>>()
                .join(" / ")
        },
    )
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
            logical_tick: sample.logical_tick,
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
    processor_id: ProcessorId,
    context_key: Option<&ContextKey>,
    context_provider: &SnapshotProcessorContextProvider,
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
        let lane_resolver = context_key.map(|context_key| LaneParamResolver {
            processor_id,
            context_key,
            context_provider,
        });
        command_count += execute_output_target(
            ctx,
            snapshot,
            *node,
            &intent.payload,
            lane_resolver.as_ref(),
            &mut fired,
        );
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
    lane_resolver: Option<&LaneParamResolver<'_>>,
    fired: &mut HashSet<NodeId>,
) -> usize {
    if !fired.insert(node) {
        return 0;
    }
    // A command fires itself; an Outputs manager / group fires through its own
    // scheduler (applying its Delay / Stagger / Cancel) — in both cases we just
    // deliver an execute event to the node and let it do the work.
    if node_is_command(snapshot, node)
        || crate::app::systems_alchemist_managed_nodes::is_output_container(snapshot, node)
    {
        let param_overrides = resolved_output_param_overrides(snapshot, node, lane_resolver);
        let _ = crate::app::module_command::emit_command_execute_with_overrides(
            ctx,
            node,
            param_overrides,
        );
        return 1;
    }

    let mut command_count = 0usize;
    for child in snapshot.child_ids(node) {
        if snapshot.node(child).is_some_and(|child| child.enabled) && node_is_command(snapshot, child) {
            if fired.insert(child) {
                let param_overrides = resolved_output_param_overrides(snapshot, child, lane_resolver);
                let _ = crate::app::module_command::emit_command_execute_with_overrides(
                    ctx,
                    child,
                    param_overrides,
                );
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

fn resolved_output_param_overrides(
    snapshot: &ProcessTreeSnapshot,
    root: NodeId,
    lane_resolver: Option<&LaneParamResolver<'_>>,
) -> crate::app::module_command::ModuleCommandParamOverrides {
    let Some(lane_resolver) = lane_resolver else {
        return Vec::new();
    };
    let mut overrides = Vec::new();
    collect_resolved_output_param_overrides(snapshot, root, lane_resolver, &mut overrides);
    overrides
}

fn collect_resolved_output_param_overrides(
    snapshot: &ProcessTreeSnapshot,
    node_id: NodeId,
    lane_resolver: &LaneParamResolver<'_>,
    overrides: &mut crate::app::module_command::ModuleCommandParamOverrides,
) {
    let Some(node) = snapshot.node(node_id) else {
        return;
    };
    if node.param_value.is_some()
        && node
            .param_control
            .as_ref()
            .is_some_and(|control| control.mode != ParameterControlMode::Manual)
    {
        if let Some(value) = lane_resolver.param_value(snapshot, node_id) {
            overrides.push(crate::app::module_command::ModuleCommandParamOverride {
                param_id: node_id,
                value,
            });
        }
    }

    for child in snapshot.child_ids(node_id) {
        collect_resolved_output_param_overrides(snapshot, child, lane_resolver, overrides);
    }
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
            crate::app::systems_alchemist_generic_commands::GENERIC_COMMAND_ITEM_KIND,
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
    context_key: Option<&chataigne_alchemist::ContextKey>,
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
    context_key: Option<&chataigne_alchemist::ContextKey>,
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
    context_key: Option<&chataigne_alchemist::ContextKey>,
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

fn apply_processor_context_property_bindings(
    snapshot: &ProcessTreeSnapshot,
    processor_node: NodeId,
    processor_id: ProcessorId,
    processor: &mut Processor,
    provider: &SnapshotProcessorContextProvider,
) {
    collect_processor_context_property_bindings(
        snapshot,
        processor_node,
        processor_id,
        processor,
        provider,
    );
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

fn collect_processor_context_property_bindings(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    processor_id: ProcessorId,
    processor: &mut Processor,
    provider: &SnapshotProcessorContextProvider,
) {
    for child in snapshot.child_ids(parent) {
        let Some(node) = snapshot.node(child) else {
            continue;
        };
        if !node.enabled {
            continue;
        }
        if node.node_type == PROCESSOR_FOLDER_NODE_TYPE {
            collect_processor_context_property_bindings(
                snapshot,
                child,
                processor_id,
                processor,
                provider,
            );
            continue;
        }
        let Some(surface_id) = node
            .decl_id
            .strip_prefix("surface/")
            .map(SurfaceItemId::new)
        else {
            continue;
        };
        let Some(param) = processor_override_param_node(snapshot, child) else {
            continue;
        };
        let Some(control) = snapshot.node(param).and_then(|param| param.param_control.as_ref())
        else {
            continue;
        };
        if control.mode != ParameterControlMode::ContextLink {
            continue;
        }
        let ParameterControlSpec::ContextLink { symbol, .. } = &control.spec else {
            continue;
        };
        if let Some((axis, path)) = provider.multiplex_link_for_symbol(processor_id, symbol) {
            processor.context_property_bindings.insert(
                surface_id,
                ProcessorContextPropertyBinding { axis, path },
            );
        }
    }
}

fn processor_runtime_inputs(
    context: &mut ProcessorRuntimeInputContext<'_>,
) -> RuntimeInputSnapshot {
    let mut inputs = RuntimeInputSnapshot::default();
    for (reference, value) in context.formula_input_values {
        inputs.insert(reference.clone(), value.clone());
    }
    let processor_node = context.processor_node;
    if let Some(processor_uuid) = context.snapshot.node(processor_node).map(|node| node.uuid) {
        collect_processor_runtime_inputs(
            context,
            processor_uuid,
            processor_node,
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
    processor_dirty: bool,
    input_signal_dirty: bool,
    condition_signal_dirty: bool,
    formula_value_dirty: bool,
) -> bool {
    continuous || processor_dirty || input_signal_dirty || condition_signal_dirty || formula_value_dirty
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
    context: &mut ProcessorRuntimeInputContext<'_>,
    processor_uuid: NodeUuid,
    parent: NodeId,
    inputs: &mut RuntimeInputSnapshot,
) {
    for child in context.snapshot.child_ids(parent) {
        let Some(node) = context.snapshot.node(child) else {
            continue;
        };
        if !node.enabled {
            continue;
        }
        if node.node_type == PROCESSOR_FOLDER_NODE_TYPE {
            collect_processor_runtime_inputs(
                context,
                processor_uuid,
                child,
                inputs,
            );
            continue;
        }
        match node.node_type.as_str() {
            INPUTS_MANAGER_NODE_TYPE => collect_input_manager_runtime_input(
                context,
                processor_uuid,
                child,
                node.decl_id.as_str(),
                inputs,
            ),
            CONDITION_MANAGER_NODE_TYPE => collect_condition_manager_runtime_input(
                context,
                processor_uuid,
                child,
                node.decl_id.as_str(),
                inputs,
            ),
            _ => {}
        }
    }
}

fn collect_input_manager_runtime_input(
    context: &mut ProcessorRuntimeInputContext<'_>,
    processor_uuid: NodeUuid,
    manager: NodeId,
    decl_id: &str,
    inputs: &mut RuntimeInputSnapshot,
) {
    let Some(manager_uuid) = decl_id
        .strip_prefix("surface/")
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    let signal_key = format!("{}:{manager_uuid}", processor_uuid.0);
    if input_manager_has_dirty_source(
        context.snapshot,
        manager,
        context.dirty_input_source_params,
    ) {
        context
            .input_manager_signal_ticks
            .insert(signal_key.clone(), context.logical_tick);
    }
    let Some(signal_tick) = context
        .input_manager_signal_ticks
        .get(&signal_key)
        .copied()
    else {
        return;
    };
    let value_set = processor_input_manager_value_set(context.snapshot, manager, signal_tick);
    if value_set.entries.is_empty() {
        context.input_manager_signal_ticks.remove(&signal_key);
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
    context: &mut ProcessorRuntimeInputContext<'_>,
    processor_uuid: NodeUuid,
    manager: NodeId,
    decl_id: &str,
    inputs: &mut RuntimeInputSnapshot,
) {
    let Some(manager_uuid) = decl_id
        .strip_prefix("surface/")
        .filter(|value| !value.is_empty())
    else {
        return;
    };
    let input_ref = StableRef::new(
        ValueTypeId::new(CONDITIONS_MANAGER_TYPE),
        manager_uuid.to_owned(),
    );
    let signal_key = format!("{}:{manager_uuid}", processor_uuid.0);
    let manager_axes = context
        .condition_manager_axes
        .entry(manager)
        .or_insert_with(|| {
            context_link_axes_in_subtree(
                context.snapshot,
                manager,
                context.processor_id,
                context.context_provider,
            )
        })
        .clone();
    if !manager_axes.is_empty() {
        let source_dirty = !context.dirty_input_source_params.is_empty();
        let mut aggregate_values = Vec::new();
        for context_key in context
            .context_provider
            .iter_context_keys(context.processor_id, &manager_axes)
        {
            let lane_key = format!("{}:{}", signal_key, context_key_cache_id(&context_key));
            let previous = context
                .condition_manager_valid_states
                .get(&lane_key)
                .copied();
            let resolver = LaneParamResolver {
                processor_id: context.processor_id,
                context_key: &context_key,
                context_provider: context.context_provider,
            };
            let validity = evaluate_compiled_condition(context, manager, Some(&resolver), &lane_key)
            .unwrap_or_else(|| ConditionValidity::steady(false));
            context
                .condition_manager_valid_states
                .insert(lane_key.clone(), validity.settled);

            let value_set = if previous != Some(validity.current) {
                let edge_source_dirty =
                    source_dirty || (context.force_processor_recompute && previous.is_some());
                let previous = condition_manager_edge_previous(
                    previous,
                    validity.current,
                    edge_source_dirty,
                );
                condition_manager_value_set(
                    context.logical_tick,
                    validity.current,
                    previous,
                    context.next_trigger_edge_id,
                )
            } else {
                condition_manager_value_set(
                    context.logical_tick,
                    validity.settled,
                    Some(validity.settled),
                    context.next_trigger_edge_id,
                )
            };
            let Ok(value) = value_set.to_runtime_value() else {
                continue;
            };
            inputs.insert(lane_scoped_stable_ref(&input_ref, &context_key), value.clone());
            context.condition_manager_values.insert(lane_key, value.clone());
            aggregate_values.push(value);
        }
        inputs.insert(input_ref, RuntimeValue::Array(aggregate_values));
        return;
    }
    let previous = context
        .condition_manager_valid_states
        .get(&signal_key)
        .copied();
    let mut current_value = None;
    let source_dirty = !context.dirty_input_source_params.is_empty();
    if previous.is_none() || source_dirty || context.force_processor_recompute {
        let validity = evaluate_compiled_condition(context, manager, None, &signal_key)
        .unwrap_or_else(|| ConditionValidity::steady(false));
        set_condition_validity_param(
            context.ctx,
            context.snapshot,
            manager,
            validity,
            context.transient_condition_valid_resets,
        );
        let previous = context
            .condition_manager_valid_states
            .insert(signal_key.clone(), validity.settled);

        if let Ok(value) = condition_manager_value_set(
            context.logical_tick,
            validity.settled,
            Some(validity.settled),
            context.next_trigger_edge_id,
        )
        .to_runtime_value()
        {
            context
                .condition_manager_values
                .insert(signal_key.clone(), value);
        }

        if previous != Some(validity.current) {
            let edge_source_dirty =
                source_dirty || (context.force_processor_recompute && previous.is_some());
            let previous = condition_manager_edge_previous(
                previous,
                validity.current,
                edge_source_dirty,
            );
            let value_set = condition_manager_value_set(
                context.logical_tick,
                validity.current,
                previous,
                context.next_trigger_edge_id,
            );
            if let Ok(value) = value_set.to_runtime_value() {
                current_value = Some(value);
            }
        }
    }
    let Some(value) = current_value
        .or_else(|| context.condition_manager_values.get(&signal_key).cloned())
    else {
        return;
    };
    inputs.insert(input_ref, value);
}

fn evaluate_compiled_condition(
    context: &mut ProcessorRuntimeInputContext<'_>,
    manager: NodeId,
    lane_resolver: Option<&LaneParamResolver<'_>>,
    runtime_key: &str,
) -> Option<ConditionValidity> {
    let compiled = match context.compiled_conditions.get(&manager) {
        Some(compiled) => compiled.clone(),
        None => {
            let compiled = compile_manager_condition(context.snapshot, manager).ok()?;
            context.compiled_conditions.insert(manager, compiled.clone());
            compiled
        }
    };
    let inputs = SnapshotConditionInputs {
        snapshot: context.snapshot,
        lane_resolver,
        bindings: &compiled.bindings,
        dirty_input_source_params: context.dirty_input_source_params,
        logical_tick: context.logical_tick,
    };
    let frame = ConditionEvaluationFrame {
        logical_tick: context.logical_tick,
        delta_time: context.ctx.delta_time,
        inputs: &inputs,
    };
    let current_result = context
        .condition_runtimes
        .entry(runtime_key.to_owned())
        .or_insert_with(|| ConditionRuntime::new(&compiled.current))
        .evaluate(&compiled.current, &frame)
        .ok()?;
    context.condition_observations.insert(
        runtime_key.to_owned(),
        current_result
            .observations
            .iter()
            .map(|(condition, valid)| ProcessorLaneConditionPreviewDto {
                node_id: condition.as_uuid().to_string(),
                valid: *valid,
            })
            .collect(),
    );
    let current = current_result.value;
    let settled = context
        .settled_condition_runtimes
        .entry(runtime_key.to_owned())
        .or_insert_with(|| ConditionRuntime::new(&compiled.settled))
        .evaluate(&compiled.settled, &frame)
        .ok()?
        .value;
    Some(ConditionValidity { current, settled })
}

struct SnapshotConditionInputs<'snapshot, 'resolver, 'context> {
    snapshot: &'snapshot ProcessTreeSnapshot,
    lane_resolver: Option<&'resolver LaneParamResolver<'context>>,
    bindings: &'snapshot HashMap<StableRef, ConditionBinding>,
    dirty_input_source_params: &'snapshot HashSet<NodeUuid>,
    logical_tick: u64,
}

impl SnapshotConditionInputs<'_, '_, '_> {
    fn resolved_param(&self, param: NodeId) -> Option<ParamValue> {
        self.lane_resolver
            .and_then(|resolver| resolver.param_value(self.snapshot, param))
            .or_else(|| self.snapshot.node(param)?.param_value.clone())
    }

    fn source_value(&self, param: NodeId, endpoint: &str) -> Option<RuntimeValue> {
        let ParamValue::Reference(reference) = self.resolved_param(param)? else {
            return None;
        };
        let source = self.snapshot.node_id_by_uuid(reference.uuid())?;
        let source = if endpoint.is_empty() {
            source
        } else {
            self.snapshot.find_child_by_decl_id(source, endpoint)?
        };
        let source = self.snapshot.node(source)?;
        if matches!(source.param_value, Some(ParamValue::Trigger())) {
            let trigger = if self.dirty_input_source_params.contains(&source.uuid) {
                TriggerValue::fired(self.logical_tick, self.logical_tick)
            } else {
                TriggerValue::default()
            };
            return Some(RuntimeValue::Trigger(trigger));
        }
        source.param_value.as_ref().and_then(param_to_condition_value)
    }
}

impl ConditionInputProvider for SnapshotConditionInputs<'_, '_, '_> {
    fn input_value(&self, input: &StableRef) -> Option<RuntimeValue> {
        match self.bindings.get(input)? {
            ConditionBinding::Constant(value) => Some(value.clone()),
            ConditionBinding::Param(param) => self
                .resolved_param(*param)
                .as_ref()
                .and_then(param_to_condition_value),
            ConditionBinding::Source(param) => self.source_value(*param, ""),
        }
    }

    fn input_node_value(&self, provider: &str, node: &StableRef) -> Option<RuntimeValue> {
        let ConditionBinding::Source(param) = self.bindings.get(node)? else {
            return None;
        };
        self.source_value(*param, provider)
    }

    fn script_condition(&self, script: &str) -> Result<bool, String> {
        match script.trim() {
            "true" => Ok(true),
            "false" | "" => Ok(false),
            _ => Err("script conditions require a registered script condition host".to_owned()),
        }
    }
}

fn context_link_axes_in_subtree(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    processor_id: ProcessorId,
    provider: &SnapshotProcessorContextProvider,
) -> AxisSet {
    let mut axes = AxisSet::new();
    collect_processor_context_link_axes(snapshot, parent, processor_id, provider, &mut axes);
    axes
}

fn context_key_cache_id(context_key: &ContextKey) -> String {
    context_key
        .iter()
        .map(|part| format!("{}={}", part.axis.as_str(), part.item.as_str()))
        .collect::<Vec<_>>()
        .join(";")
}

fn condition_manager_edge_previous(
    previous: Option<bool>,
    current: bool,
    source_dirty: bool,
) -> Option<bool> {
    match previous {
        Some(previous) => Some(previous),
        None if source_dirty => None,
        None => Some(current),
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

struct LaneParamResolver<'a> {
    processor_id: ProcessorId,
    context_key: &'a ContextKey,
    context_provider: &'a SnapshotProcessorContextProvider,
}

impl LaneParamResolver<'_> {
    fn param_value(&self, snapshot: &ProcessTreeSnapshot, param: NodeId) -> Option<ParamValue> {
        let node = snapshot.node(param)?;
        let Some(control) = node.param_control.as_ref() else {
            return node.param_value.clone();
        };
        if control.mode == ParameterControlMode::TemplateText {
            let ParamValue::Str(value) = node.param_value.clone()? else {
                return node.param_value.clone();
            };
            return Some(ParamValue::Str(resolve_multiplex_template_value(
                value.as_str(),
                |token| {
                    self.context_provider
                        .resolve_template_token(self.processor_id, self.context_key, token)
                },
            )));
        }
        if control.mode != ParameterControlMode::ContextLink {
            return node.param_value.clone();
        }
        let ParameterControlSpec::ContextLink { symbol, projection } = &control.spec else {
            return node.param_value.clone();
        };
        let Some((axis, path)) = self
            .context_provider
            .multiplex_link_for_symbol(self.processor_id, symbol)
        else {
            return node.param_value.clone();
        };
        let Some(runtime_value) =
            self.context_provider
                .resolve_context_value(self.context_key, &axis, &path)
        else {
            return node.param_value.clone();
        };
        let value = runtime_value_to_param(&runtime_value).ok()?;
        let target = node.param_value.as_ref()?;
        coerce_param_value_for_target(&value, target, *projection)
    }
}

fn resolve_multiplex_template_value(
    value: &str,
    mut resolve: impl FnMut(&MultiplexTemplateToken) -> Option<String>,
) -> String {
    let mut output = String::with_capacity(value.len());
    let mut remaining = value;
    while let Some(open) = remaining.find('{') {
        output.push_str(&remaining[..open]);
        let token_start = open + 1;
        let Some(close) = remaining[token_start..].find('}') else {
            output.push_str(&remaining[open..]);
            return output;
        };
        let token_end = token_start + close;
        let raw = &remaining[token_start..token_end];
        if let Some(token) = parse_multiplex_template_token(raw) {
            if let Some(resolved) = resolve(&token) {
                output.push_str(resolved.as_str());
            } else {
                output.push_str(&remaining[open..=token_end]);
            }
        } else {
            output.push_str(&remaining[open..=token_end]);
        }
        remaining = &remaining[token_end + 1..];
    }
    output.push_str(remaining);
    output
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

fn processor_override_value(
    snapshot: &ProcessTreeSnapshot,
    property: NodeId,
) -> Option<&ParamValue> {
    processor_override_param_node(snapshot, property)
        .and_then(|param| snapshot.node(param))
        .and_then(|node| node.param_value.as_ref())
}

fn processor_override_param_node(
    snapshot: &ProcessTreeSnapshot,
    property: NodeId,
) -> Option<NodeId> {
    if snapshot
        .node(property)
        .is_some_and(|node| node.param_value.is_some())
    {
        return Some(property);
    }
    snapshot.find_child_by_decl_id(property, "value")
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
        ParamValue::File(value) => Some(RuntimeValue::String(value.clone().into())),
        ParamValue::Enum(value) => Some(RuntimeValue::String(value.clone().into())),
        ParamValue::CssValue(value) => Some(RuntimeValue::Float(value.value)),
        ParamValue::Vec2(x, y) => Some(RuntimeValue::Vec2([*x, *y])),
        ParamValue::Vec3(x, y, z) => Some(RuntimeValue::Vec3([*x, *y, *z])),
        ParamValue::Color(r, g, b, a) => Some(RuntimeValue::Color(chataigne_alchemist::ColorValue {
            red: *r,
            green: *g,
            blue: *b,
            alpha: *a,
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
