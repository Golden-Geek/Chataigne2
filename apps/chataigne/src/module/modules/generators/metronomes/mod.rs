#[cfg(test)]
mod tests;
mod runtime;

use std::collections::{HashMap, HashSet};

use golden_core::{
    edit::NodeTree,
    engine::NodeExecutionRule,
    events::{CustomEvent, Event, EventKind},
    node,
    node::{
        DeclId, Node, NodeHandle, NodeId, NodeMetaPatch, NodeScriptDescriptor, NodeUuid,
        UserContainerRules, UserCreatableItem,
    },
    parameter::{
        Enum, ParamValue, Parameter, ParameterChangeCheck, ParameterEnumOption,
        ParameterEventBehaviour, Vec2,
    },
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

const METRONOMES_UPDATE_RATE_DEFAULT_HZ: i32 = 60;
const METRONOMES_UPDATE_RATE_MAX_HZ: i32 = 240;
const METRONOMES_COMPILED_KERNEL: &str = "chataigne.runtime.metronomes";
const METRONOME_ITEM_KIND: &str = "metronome";
const METRONOME_ITEM_LABEL: &str = "Metronome";
const METRONOME_MODE_FREQUENCY: &str = "frequency";
const METRONOME_MODE_TIME: &str = "time";
const METRONOME_MODE_BPM: &str = "bpm";
const METRONOME_SCRIPT_METHODS: &[&str] = &[
    "resetMetronomes",
    "resetMetronome",
    "tickMetronome",
];
const METRONOME_TICK_CALLBACK: &str = "metronomeTick";
const METRONOME_ENABLEMENT_CHANGED_EVENT: &str = "metronomes.itemEnablementChanged";
const MAX_TICKS_PER_UPDATE: u32 = 128;
const MIN_INTERVAL_SECONDS: f64 = 0.001;

#[derive(Clone, Copy, Debug, PartialEq)]
struct MetronomeSignature {
    interval_seconds: u64,
    randomize_gap: bool,
    random_min: u64,
    random_max: u64,
    seed: i32,
}

#[derive(Clone, Debug)]
struct MetronomeRuntimeState {
    elapsed_seconds: f64,
    next_gap_seconds: f64,
    tick_count: u64,
    signature: Option<MetronomeSignature>,
}

impl Default for MetronomeRuntimeState {
    fn default() -> Self {
        Self {
            elapsed_seconds: 0.0,
            next_gap_seconds: 0.0,
            tick_count: 0,
            signature: None,
        }
    }
}

#[derive(Clone, Debug)]
struct MetronomeConfig {
    item_id: NodeId,
    item_uuid: NodeUuid,
    value_decl_id: String,
    label: String,
    enabled: bool,
    interval_seconds: f64,
    randomize_gap: bool,
    random_min: f64,
    random_max: f64,
    seed: i32,
}

impl MetronomeConfig {
    fn signature(&self) -> MetronomeSignature {
        MetronomeSignature {
            interval_seconds: self.interval_seconds.to_bits(),
            randomize_gap: self.randomize_gap,
            random_min: self.random_min.to_bits(),
            random_max: self.random_max.to_bits(),
            seed: self.seed,
        }
    }
}

#[node("metronomes_module", label = "Metronomes")]
#[children(
    folder(connection) {
        [base_children];
    }
    folder(parameters) {
        update_rate_hz: i32 = METRONOMES_UPDATE_RATE_DEFAULT_HZ [1..METRONOMES_UPDATE_RATE_MAX_HZ] (
            label = "Update Rate",
            description = "Engine update rate used to evaluate every metronome in this module.",
            widget = "text"
        );
        node metronomes: MetronomeList = MetronomeList::new() (
            label = "Metronomes",
            description = "Create all metronomes managed by this module."
        );
        [base_children];
    }
    folder(values) {
        [base_children];
    }
)]
pub struct MetronomesModule {
    base: crate::app::ModuleBase,
    value_nodes_by_item: HashMap<NodeId, NodeId>,
    pending_value_items: HashSet<NodeId>,
    runtime: Option<runtime::MetronomeRuntimeHandle>,
    config_dirty: bool,
}

impl MetronomesModule {
    pub fn create() -> Self {
        Self::new(
            crate::app::ModuleBase::new(),
            HashMap::new(),
            HashSet::new(),
            None,
            true,
        )
    }

    fn sync_configuration(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        let configs = self.collect_metronomes(snapshot);
        let Some(values_root) = self.values_root(snapshot) else {
            self.config_dirty = false;
            self.pending_value_items.clear();
            self.stop_runtime();
            return;
        };
        let waiting_for_values = sync_value_folders(
            ctx,
            snapshot,
            values_root,
            configs.iter(),
            &mut self.value_nodes_by_item,
            &mut self.pending_value_items,
            metronome_values_tree,
        );
        self.config_dirty = waiting_for_values;

        self.configure_runtime(runtime::MetronomeWorkerConfig {
            update_rate_hz: runtime_update_rate_hz(self.update_rate_hz.get()),
            metronomes: configs,
        });
    }

    fn configure_runtime(&mut self, config: runtime::MetronomeWorkerConfig) {
        if self.runtime.is_none() {
            match runtime::MetronomeRuntimeHandle::spawn(config) {
                Ok(runtime) => self.runtime = Some(runtime),
                Err(error) => {
                    golden_core::log!(origin = self.id(); format!("{error}"));
                }
            }
            return;
        }

        let send_result = self
            .runtime
            .as_ref()
            .expect("runtime should exist")
            .configure(config.clone());
        if send_result.is_ok() {
            return;
        }

        self.stop_runtime();
        match runtime::MetronomeRuntimeHandle::spawn(config) {
            Ok(runtime) => self.runtime = Some(runtime),
            Err(error) => {
                golden_core::log!(origin = self.id(); format!("{error}"));
            }
        }
    }

    fn stop_runtime(&mut self) {
        if let Some(mut runtime) = self.runtime.take() {
            runtime.stop();
        }
    }

    fn drain_runtime_events(&mut self, ctx: &mut ProcessCtx) {
        let Some(event) = self
            .runtime
            .as_ref()
            .and_then(|runtime| runtime.take_event())
        else {
            return;
        };

        let module_id = self.id();
        let mut received_tick = false;
        for tick in event.ticks.into_values() {
            let Some(tick_param) = self.value_nodes_by_item.get(&tick.item_id).copied() else {
                continue;
            };
            if tick.fired == 0 {
                continue;
            }

            received_tick = true;
            ctx.set_param_with_behaviour(
                tick_param,
                ParamValue::Trigger(),
                ParameterEventBehaviour::Coalesce,
            );
            if self.base.log_incoming_enabled() {
                golden_core::log!(origin = module_id; format!(
                    "Metronome '{}' ticked {} time(s), total {}.",
                    tick.label, tick.fired, tick.total_ticks
                ));
            }
            crate::app::module::script_api::emit_script_callback(
                ctx,
                module_id,
                METRONOME_TICK_CALLBACK,
                metronome_tick_callback_args(&tick),
            );
        }

        if received_tick {
            self.base.emit_incoming_traffic(ctx);
        }
    }

    fn collect_metronomes(&self, snapshot: &ProcessTreeSnapshot) -> Vec<MetronomeConfig> {
        let Some(list_id) = self.metronome_list_id(snapshot) else {
            return Vec::new();
        };

        snapshot
            .child_ids(list_id)
            .into_iter()
            .filter_map(|item_id| metronome_config(snapshot, item_id))
            .collect()
    }

    fn metronome_list_id(&self, snapshot: &ProcessTreeSnapshot) -> Option<NodeId> {
        let parameters_id = self.base.parameters_id()?;
        snapshot.find_child(parameters_id, "metronomes")
    }

    fn values_root(&self, _snapshot: &ProcessTreeSnapshot) -> Option<NodeId> {
        self.base.values_id()
    }

    fn ensure_default_metronome(&self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        let Some(list_id) = self.metronome_list_id(snapshot) else {
            return;
        };

        if snapshot.child_ids(list_id).is_empty() {
            ctx.add_user_item_boxed(list_id, Box::new(MetronomeItem::new()), None);
        }
    }

    fn reset_matching_metronomes(
        &mut self,
        snapshot: &ProcessTreeSnapshot,
        selector: Option<&ParamValue>,
    ) -> Result<(), String> {
        let configs = self.collect_metronomes(snapshot);
        let mut matched_ids = Vec::new();
        for (index, config) in configs.iter().enumerate() {
            if !selector_matches(snapshot, index, config.item_id, config.label.as_str(), selector) {
                continue;
            }
            matched_ids.push(config.item_id);
        }

        if matched_ids.is_empty() {
            return Err("no matching metronome".to_string());
        }

        let runtime = self
            .runtime
            .as_ref()
            .ok_or_else(|| "Metronomes worker is not running".to_string())?;
        runtime.reset(matched_ids)
    }

    fn manual_tick_metronome(
        &mut self,
        _ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        selector: Option<&ParamValue>,
    ) -> Result<(), String> {
        let configs = self.collect_metronomes(snapshot);
        for (index, config) in configs.iter().enumerate() {
            if !selector_matches(snapshot, index, config.item_id, config.label.as_str(), selector) {
                continue;
            }
            if !self.value_nodes_by_item.contains_key(&config.item_id) {
                return Err("metronome trigger output is not materialized yet".to_string());
            }
            let runtime = self
                .runtime
                .as_ref()
                .ok_or_else(|| "Metronomes worker is not running".to_string())?;
            runtime.manual_tick(config.item_id)?;
            return Ok(());
        }

        Err("no matching metronome".to_string())
    }

    fn handle_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        method: &str,
        args: &[ParamValue],
    ) -> Option<Result<(), String>> {
        match method {
            "resetMetronomes" => Some(self.reset_matching_metronomes(snapshot, None)),
            "resetMetronome" => Some(self.reset_matching_metronomes(snapshot, args.first())),
            "tickMetronome" => Some(self.manual_tick_metronome(ctx, snapshot, args.first())),
            _ => None,
        }
    }

    fn is_metronome_configuration_event(
        &self,
        snapshot: &ProcessTreeSnapshot,
        node_id: NodeId,
    ) -> bool {
        let Some(list_id) = self.metronome_list_id(snapshot) else {
            return false;
        };
        node_is_descendant_or_self(snapshot, node_id, list_id)
            || self.update_rate_hz.is_bound() && self.update_rate_hz.id() == node_id
    }

    fn is_value_output(&self, node_id: NodeId) -> bool {
        self.value_nodes_by_item
            .values()
            .any(|value_id| *value_id == node_id)
    }

    fn handle_metronome_enablement_event(&mut self, ctx: &mut ProcessCtx, event: &CustomEvent) {
        if event.topic != METRONOME_ENABLEMENT_CHANGED_EVENT {
            return;
        }
        let Some(origin) = event.origin else {
            return;
        };
        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            self.config_dirty = true;
            return;
        };
        let snapshot = snapshot_arc.as_ref();
        let Some(list_id) = self.metronome_list_id(snapshot) else {
            return;
        };
        if !node_is_descendant_or_self(snapshot, origin, list_id) {
            return;
        }
        self.config_dirty = true;
        self.sync_configuration(ctx, snapshot);
    }
}

fn metronome_tick_callback_args(tick: &runtime::MetronomeWorkerTick) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!(tick.label),
        serde_json::json!(tick.fired),
        serde_json::json!(tick.total_ticks),
        serde_json::json!({
            "name": tick.label,
            "ticks": tick.fired,
            "totalTicks": tick.total_ticks,
            "intervalSeconds": tick.interval_seconds,
            "lastGapSeconds": tick.last_gap_seconds,
        }),
    ]
}

#[golden_core::item(
    "module",
    node = "metronomes_module",
    via = base,
    from_struct,
    menu_path = ["Generators"]
)]
impl Node for MetronomesModule {
    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        match event.kind {
            EventKind::ParamChanged { .. }
            | EventKind::ChildAdded { .. }
            | EventKind::ChildRemoved { .. }
            | EventKind::ChildReplaced { .. }
            | EventKind::Custom(_) => u32::MAX,
            _ => 1,
        }
    }

    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);
        self.base
            .set_data_capabilities(ctx, crate::app::module::ModuleDataCapabilities::new(true, false));
        self.base.set_connected(ctx, true);
        crate::app::module::enable_module_authoring(self.node_data_mut());
        if let Some(snapshot_arc) = ctx.tree_snapshot_arc() {
            let snapshot = snapshot_arc.as_ref();
            self.ensure_default_metronome(ctx, snapshot);
            self.sync_configuration(ctx, snapshot);
        }
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        if self.config_dirty {
            if let Some(snapshot_arc) = ctx.tree_snapshot_arc() {
                self.sync_configuration(ctx, snapshot_arc.as_ref());
            }
        }
        self.drain_runtime_events(ctx);
    }

    fn needs_update(&self) -> bool {
        self.config_dirty || self.runtime.as_ref().is_some_and(|runtime| runtime.has_pending())
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        self.config_dirty
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::periodic(runtime_update_rate_hz(self.update_rate_hz.get()))
            .with_compiled_kernel(METRONOMES_COMPILED_KERNEL)
    }

    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        crate::app::module::script_api::descriptor_for_node(
            self.node_data(),
            self.get_type(),
            METRONOME_SCRIPT_METHODS,
        )
    }

    fn engine_call_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Result<bool, String> {
        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return self.base.engine_call_script_method(ctx, method, args);
        };
        if let Some(result) = self.handle_script_method(ctx, snapshot_arc.as_ref(), method, args) {
            result?;
            return Ok(true);
        }

        self.base.engine_call_script_method(ctx, method, args)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, old_value: ParamValue) {
        if self.is_value_output(param) {
            return;
        }

        if self.update_rate_hz.is_bound() && self.update_rate_hz.id() == param {
            ctx.reevaluate_graph();
        }

        if let Some(snapshot_arc) = ctx.tree_snapshot_arc() {
            let snapshot = snapshot_arc.as_ref();
            if self.is_metronome_configuration_event(snapshot, param) {
                self.config_dirty = true;
                self.sync_configuration(ctx, snapshot);
            }
            self.base
                .emit_script_param_callback(ctx, snapshot, param, &old_value);
        }
    }

    fn on_child_added(&mut self, ctx: &mut ProcessCtx, parent: NodeId, child: NodeId) {
        if let Some(snapshot_arc) = ctx.tree_snapshot_arc() {
            let snapshot = snapshot_arc.as_ref();
            if self.is_metronome_configuration_event(snapshot, parent)
                || self.is_metronome_configuration_event(snapshot, child)
            {
                self.config_dirty = true;
                self.sync_configuration(ctx, snapshot);
            }
        }
    }

    fn on_child_removed(&mut self, ctx: &mut ProcessCtx, parent: NodeId, child: NodeId) {
        if let Some(snapshot_arc) = ctx.tree_snapshot_arc() {
            let snapshot = snapshot_arc.as_ref();
            if self.is_metronome_configuration_event(snapshot, parent) {
                self.config_dirty = true;
                self.value_nodes_by_item.remove(&child);
                self.pending_value_items.remove(&child);
                self.sync_configuration(ctx, snapshot);
            }
        }
    }

    fn on_effective_enabled_changed(&mut self, _ctx: &mut ProcessCtx, enabled: bool) {
        if !enabled {
            self.stop_runtime();
        }
        self.config_dirty = true;
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        self.handle_metronome_enablement_event(ctx, &event);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

#[node("metronome_list", label = "Metronomes")]
pub struct MetronomeList {}

#[node("metronome_list", from_struct)]
impl Node for MetronomeList {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_authoring(self.node_data_mut());
        self.node_data_mut().meta.can_be_disabled = false;
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[METRONOME_ITEM_KIND]))
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![
            UserCreatableItem::new(MetronomeItem::NODE_TYPE, METRONOME_ITEM_KIND, METRONOME_ITEM_LABEL)
                .with_select_when_created(false),
        ]
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        (node_type == MetronomeItem::NODE_TYPE || node_type == METRONOME_ITEM_KIND)
            .then(|| Box::new(MetronomeItem::new()) as Box<dyn Node>)
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("metronome", label = "Metronome")]
#[children(
    mode: Enum = METRONOME_MODE_FREQUENCY (
        label = "Mode",
        description = "How the regular tick interval is specified.",
        enum_options = metronome_mode_options()
    );
    frequency_hz: f64 = 1.0 [MIN_INTERVAL_SECONDS..] (
        label = "Frequency",
        description = "Tick frequency in hertz.",
        dependency = mode == "frequency",
        widget = "text"
    );
    interval_seconds: f64 = 1.0 [MIN_INTERVAL_SECONDS..] (
        label = "Time",
        description = "Time between ticks in seconds.",
        dependency = mode == "time",
        widget = "time"
    );
    bpm: f64 = 120.0 [MIN_INTERVAL_SECONDS..] (
        label = "BPM",
        description = "Tempo in beats per minute. One tick is one beat.",
        dependency = mode == "bpm",
        widget = "text"
    );
    randomize_gap: bool = false (
        label = "Randomize Gap",
        description = "Whether each next gap uses a random multiplier."
    );
    random_gap: Vec2 = (0.75, 1.25) [(MIN_INTERVAL_SECONDS, MIN_INTERVAL_SECONDS)..(1000.0, 1000.0)] (
        label = "Gap Multiplier Range",
        description = "Minimum and maximum multiplier applied to each generated gap.",
        dependency = randomize_gap
    );
    seed: i32 = 0 (
        label = "Seed",
        description = "Deterministic seed used for random gap generation.",
        dependency = randomize_gap
    );
)]
pub struct MetronomeItem {}

#[node("metronome", from_struct)]
impl Node for MetronomeItem {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }

    fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, _enabled: bool) {
        ctx.emit_custom_event(CustomEvent::new(
            METRONOME_ENABLEMENT_CHANGED_EVENT,
            Some(self.id()),
            serde_json::Value::Null,
        ));
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

fn runtime_update_rate_hz(value: i32) -> u32 {
    value
        .clamp(1, METRONOMES_UPDATE_RATE_MAX_HZ)
        .try_into()
        .unwrap_or(METRONOMES_UPDATE_RATE_DEFAULT_HZ as u32)
}

fn metronome_mode_options() -> Vec<ParameterEnumOption> {
    vec![
        enum_option(METRONOME_MODE_FREQUENCY, "Frequency", 0),
        enum_option(METRONOME_MODE_TIME, "Time", 1),
        enum_option(METRONOME_MODE_BPM, "BPM", 2),
    ]
}

fn enum_option(variant_id: &str, label: &str, ordering: i32) -> ParameterEnumOption {
    ParameterEnumOption {
        variant_id: variant_id.to_string(),
        value: ParamValue::Enum(variant_id.to_string()),
        label: label.to_string(),
        tags: Vec::new(),
        ordering: Some(ordering),
    }
}

fn reset_state_if_config_changed(state: &mut MetronomeRuntimeState, config: &MetronomeConfig) {
    let signature = config.signature();
    if state.signature == Some(signature) {
        return;
    }
    state.elapsed_seconds = 0.0;
    state.next_gap_seconds = metronome_gap_seconds(config, state.tick_count);
    state.signature = Some(signature);
}

fn metronome_gap_seconds(config: &MetronomeConfig, tick_count: u64) -> f64 {
    let interval = config.interval_seconds.max(MIN_INTERVAL_SECONDS);
    if !config.randomize_gap {
        return interval;
    }

    let min = config.random_min.min(config.random_max).max(MIN_INTERVAL_SECONDS);
    let max = config.random_min.max(config.random_max).max(min);
    let random = hash_unit(config.seed as i64, tick_count as i64);
    (interval * (min + (max - min) * random)).max(MIN_INTERVAL_SECONDS)
}

fn metronome_config(
    snapshot: &ProcessTreeSnapshot,
    item_id: NodeId,
) -> Option<MetronomeConfig> {
    let item = snapshot.node(item_id)?;
    if item.node_type != MetronomeItem::NODE_TYPE {
        return None;
    }

    let mode = child_enum(snapshot, item_id, "mode", METRONOME_MODE_FREQUENCY);
    let interval_seconds = match mode.as_str() {
        METRONOME_MODE_TIME => child_float(snapshot, item_id, "interval_seconds", 1.0),
        METRONOME_MODE_BPM => 60.0 / child_float(snapshot, item_id, "bpm", 120.0).max(MIN_INTERVAL_SECONDS),
        _ => 1.0 / child_float(snapshot, item_id, "frequency_hz", 1.0).max(MIN_INTERVAL_SECONDS),
    }
    .max(MIN_INTERVAL_SECONDS);
    let (random_min, random_max) = child_vec2(snapshot, item_id, "random_gap", (0.75, 1.25));

    Some(MetronomeConfig {
        item_id,
        item_uuid: item.uuid,
        value_decl_id: value_folder_decl_id("metronome", item.uuid),
        label: item.label.clone(),
        enabled: item.enabled,
        interval_seconds,
        randomize_gap: child_bool(snapshot, item_id, "randomize_gap", false),
        random_min,
        random_max,
        seed: child_int(snapshot, item_id, "seed", 0),
    })
}

fn sync_value_folders<'a, I>(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    root_id: NodeId,
    configs: I,
    value_nodes_by_item: &mut HashMap<NodeId, NodeId>,
    pending_value_items: &mut HashSet<NodeId>,
    build_tree: fn(&MetronomeConfig) -> NodeTree,
) -> bool
where
    I: IntoIterator<Item = &'a MetronomeConfig>,
{
    let existing_by_decl = child_ids_by_decl(snapshot, root_id);
    let existing_by_label = metronome_value_child_ids_by_label(snapshot, root_id);
    let mut used_node_ids = HashSet::new();
    let mut next_value_nodes_by_item = HashMap::new();
    let mut active_item_ids = HashSet::new();
    let mut waiting_for_values = false;

    for config in configs {
        active_item_ids.insert(config.item_id);
        let existing_node_id = value_nodes_by_item
            .get(&config.item_id)
            .copied()
            .filter(|node_id| snapshot.node(*node_id).is_some())
            .or_else(|| existing_by_decl.get(config.value_decl_id.as_str()).copied())
            .or_else(|| {
                existing_by_label
                    .get(config.label.as_str())
                    .and_then(|nodes| {
                        nodes
                            .iter()
                            .copied()
                            .find(|node_id| !used_node_ids.contains(node_id))
                    })
            });

        match existing_node_id {
            Some(node_id) => {
                pending_value_items.remove(&config.item_id);
                used_node_ids.insert(node_id);
                next_value_nodes_by_item.insert(config.item_id, node_id);
                if snapshot
                    .node(node_id)
                    .is_some_and(|node| node.label != config.label)
                {
                    ctx.patch_node_meta(
                        node_id,
                        NodeMetaPatch {
                            label: Some(config.label.clone()),
                            ..Default::default()
                        },
                    );
                }
            }
            None => {
                waiting_for_values = true;
                if pending_value_items.insert(config.item_id) {
                    ctx.add_child_tree(root_id, build_tree(config), None);
                }
            }
        }
    }

    for child_id in snapshot.child_ids(root_id) {
        if used_node_ids.contains(&child_id) {
            continue;
        }
        let Some(child) = snapshot.node(child_id) else {
            continue;
        };
        if !child.decl_id.starts_with("metronome_") {
            continue;
        }
        NodeHandle::new(child_id).remove(ctx);
    }

    pending_value_items.retain(|item_id| active_item_ids.contains(item_id));
    *value_nodes_by_item = next_value_nodes_by_item;
    waiting_for_values
}

fn metronome_values_tree(config: &MetronomeConfig) -> NodeTree {
    NodeTree::new(read_only_param(
        config.label.as_str(),
        config.value_decl_id.as_str(),
        metronome_value_uuid(config.item_uuid),
        ParamValue::Trigger(),
    ))
}

fn read_only_param(label: &str, decl_id: &str, uuid: NodeUuid, value: ParamValue) -> Parameter {
    let mut parameter = Parameter::new(label, value, ParameterChangeCheck::ValueChange);
    parameter.read_only = true;
    crate::app::module::enable_module_authoring(parameter.node_data_mut());
    let meta = &mut parameter.node_data_mut().meta;
    meta.uuid = uuid;
    meta.decl_id = DeclId(decl_id.to_string());
    meta.short_name = decl_id.to_string();
    parameter
}

fn child_ids_by_decl(snapshot: &ProcessTreeSnapshot, root_id: NodeId) -> HashMap<String, NodeId> {
    let mut by_decl = HashMap::new();
    for child_id in snapshot.child_ids(root_id) {
        let Some(child) = snapshot.node(child_id) else {
            continue;
        };
        if child.decl_id.trim().is_empty() {
            continue;
        }
        by_decl.entry(child.decl_id.clone()).or_insert(child_id);
    }
    by_decl
}

fn metronome_value_child_ids_by_label(
    snapshot: &ProcessTreeSnapshot,
    root_id: NodeId,
) -> HashMap<String, Vec<NodeId>> {
    let mut by_label = HashMap::<String, Vec<NodeId>>::new();
    for child_id in snapshot.child_ids(root_id) {
        let Some(child) = snapshot.node(child_id) else {
            continue;
        };
        if child.decl_id.starts_with("metronome_") {
            by_label
                .entry(child.label.clone())
                .or_default()
                .push(child_id);
        }
    }
    by_label
}

fn metronome_value_uuid(item_uuid: NodeUuid) -> NodeUuid {
    const METRONOME_VALUE_UUID_MASK: u128 = 0x6d657472_6f6e_6f6d_655f_76616c756500;
    NodeUuid(uuid::Uuid::from_u128(
        item_uuid.0.as_u128() ^ METRONOME_VALUE_UUID_MASK,
    ))
}

fn value_folder_decl_id(prefix: &str, item_uuid: NodeUuid) -> String {
    format!("{}_{}", prefix, item_uuid.0.simple())
}

fn selector_matches(
    snapshot: &ProcessTreeSnapshot,
    index: usize,
    item_id: NodeId,
    label: &str,
    selector: Option<&ParamValue>,
) -> bool {
    let Some(selector) = selector else {
        return true;
    };
    if let Some(index_arg) = selector.as_int() {
        return index_arg > 0 && (index_arg as usize) == index + 1;
    }
    let Some(text) = selector.as_str() else {
        return false;
    };
    let text = text.trim();
    if text.is_empty() {
        return true;
    }
    text == label
        || snapshot.node(item_id).is_some_and(|node| {
            node.short_name == text || node.decl_id == text || node.decl_id.rsplit('/').next() == Some(text)
        })
}

fn node_is_descendant_or_self(
    snapshot: &ProcessTreeSnapshot,
    node_id: NodeId,
    ancestor_id: NodeId,
) -> bool {
    let mut current = Some(node_id);
    while let Some(current_id) = current {
        if current_id == ancestor_id {
            return true;
        }
        current = snapshot.node(current_id).and_then(|node| node.parent);
    }
    false
}

fn child_value(snapshot: &ProcessTreeSnapshot, parent: NodeId, key: &str) -> Option<ParamValue> {
    snapshot
        .find_child(parent, key)
        .and_then(|node_id| snapshot.node(node_id))
        .and_then(|node| node.param_value.clone())
}

fn child_bool(snapshot: &ProcessTreeSnapshot, parent: NodeId, key: &str, default: bool) -> bool {
    child_value(snapshot, parent, key)
        .and_then(|value| value.as_bool())
        .unwrap_or(default)
}

fn child_int(snapshot: &ProcessTreeSnapshot, parent: NodeId, key: &str, default: i32) -> i32 {
    child_value(snapshot, parent, key)
        .and_then(|value| value.as_int())
        .unwrap_or(default)
}

fn child_float(snapshot: &ProcessTreeSnapshot, parent: NodeId, key: &str, default: f64) -> f64 {
    child_value(snapshot, parent, key)
        .and_then(|value| value.as_float())
        .filter(|value| value.is_finite())
        .unwrap_or(default)
}

fn child_enum(snapshot: &ProcessTreeSnapshot, parent: NodeId, key: &str, default: &str) -> String {
    child_value(snapshot, parent, key)
        .and_then(|value| value.as_enum())
        .unwrap_or_else(|| default.to_string())
}

fn child_vec2(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    key: &str,
    default: (f64, f64),
) -> (f64, f64) {
    match child_value(snapshot, parent, key) {
        Some(ParamValue::Vec2(x, y)) if x.is_finite() && y.is_finite() => (x, y),
        _ => default,
    }
}

fn hash_unit(seed: i64, index: i64) -> f64 {
    let mut x = (seed as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ index as u64;
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    ((x >> 11) as f64) / ((1u64 << 53) as f64)
}
