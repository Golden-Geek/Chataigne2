#[cfg(test)]
mod signals_tests;
#[path = "signals/runtime.rs"]
mod runtime;

use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;

use golden_core::{
    edit::NodeTree,
    engine::NodeExecutionRule,
    events::{CustomEvent, Event, EventFrame, EventKind},
    node,
    node::{
        curve_from_snapshot, Curve, CurveNode, DeclId, Node, NodeHandle, NodeId, NodeMetaPatch,
        NodeScriptDescriptor, NodeUuid, UserContainerRules, UserCreatableItem,
    },
    parameter::{
        Enum, ParamValue, Parameter, ParameterChangeCheck, ParameterEnumOption, RangeConstraint,
        Vec2,
    },
    process_ctx::{ProcessCtx, ProcessTreeNodeSnapshot, ProcessTreeSnapshot},
};

const SIGNALS_UPDATE_RATE_DEFAULT_HZ: i32 = 60;
const SIGNALS_UPDATE_RATE_MAX_HZ: i32 = 240;
const SIGNAL_ITEM_KIND: &str = "signal";
const SIGNAL_ITEM_LABEL: &str = "Signal";
const SIGNAL_SHAPE_SINE: &str = "sine";
const SIGNAL_SHAPE_TRIANGLE: &str = "triangle";
const SIGNAL_SHAPE_SAW: &str = "saw";
const SIGNAL_SHAPE_REVERSE_SAW: &str = "reverseSaw";
const SIGNAL_SHAPE_RANDOM_PERLIN: &str = "randomPerlin";
const SIGNAL_SHAPE_RANDOM_PURE: &str = "randomPure";
const SIGNAL_SHAPE_CURVE: &str = "curve";
const SIGNAL_SCRIPT_METHODS: &[&str] = &["resetSignals", "resetSignal"];
const SIGNAL_CYCLE_CALLBACK: &str = "signalCycle";
const SIGNAL_ENABLEMENT_CHANGED_EVENT: &str = "signals.itemEnablementChanged";

#[derive(Clone, Copy, Debug, PartialEq)]
struct SignalSignature {
    shape: SignalShape,
    frequency_hz: u64,
    phase: u64,
    seed: i32,
}

#[derive(Clone, Debug)]
struct SignalRuntimeState {
    elapsed_seconds: f64,
    last_cycle: i64,
    sampled_once: bool,
    pure_random_step: i64,
    pure_random_value: f64,
    signature: Option<SignalSignature>,
}

impl Default for SignalRuntimeState {
    fn default() -> Self {
        Self {
            elapsed_seconds: 0.0,
            last_cycle: 0,
            sampled_once: false,
            pure_random_step: i64::MIN,
            pure_random_value: 0.0,
            signature: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum SignalShape {
    Sine,
    Triangle,
    Saw,
    ReverseSaw,
    RandomPerlin,
    RandomPure,
    Curve,
}

#[derive(Clone, Debug)]
struct SignalConfig {
    item_id: NodeId,
    value_decl_id: String,
    label: String,
    enabled: bool,
    shape: SignalShape,
    frequency_hz: f64,
    phase: f64,
    range_min: f64,
    range_max: f64,
    curve: Option<Curve>,
    seed: i32,
}

impl SignalConfig {
    fn signature(&self) -> SignalSignature {
        SignalSignature {
            shape: self.shape,
            frequency_hz: self.frequency_hz.to_bits(),
            phase: self.phase.to_bits(),
            seed: self.seed,
        }
    }
}

#[node("signals_module", label = "Signals")]
#[children(
    folder(connection) {
        [base_children];
    }
    folder(parameters) {
        update_rate_hz: i32 = SIGNALS_UPDATE_RATE_DEFAULT_HZ [1..SIGNALS_UPDATE_RATE_MAX_HZ] (
            label = "Update Rate",
            description = "Engine update rate used to evaluate every signal in this module.",
            widget = "text"
        );
        node signals: SignalList = SignalList::new() (
            label = "Signals",
            description = "Create all continuous signals managed by this module."
        );
        [base_children];
    }
    folder(values) {
        [base_children];
    }
)]
pub struct SignalsModule {
    base: crate::app::ModuleBase,
    value_nodes_by_item: HashMap<NodeId, NodeId>,
    value_output_nodes: HashSet<NodeId>,
    pending_value_items: HashSet<NodeId>,
    runtime: Option<runtime::SignalRuntimeHandle>,
    config_dirty: bool,
}

impl SignalsModule {
    pub fn create() -> Self {
        Self::new(
            crate::app::ModuleBase::new(),
            HashMap::new(),
            HashSet::new(),
            HashSet::new(),
            None,
            true,
        )
    }

    fn sync_configuration(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        let configs = self.collect_signals(snapshot);
        let Some(values_root) = self.values_root(snapshot) else {
            self.config_dirty = false;
            self.pending_value_items.clear();
            self.stop_runtime();
            return;
        };
        let waiting_for_values = sync_value_nodes(
            ctx,
            snapshot,
            values_root,
            configs.iter(),
            &mut self.value_nodes_by_item,
            &mut self.value_output_nodes,
            &mut self.pending_value_items,
        );
        self.config_dirty = waiting_for_values;

        self.configure_runtime(runtime::SignalWorkerConfig {
            update_rate_hz: runtime_update_rate_hz(self.update_rate_hz.get()),
            signals: configs,
        });
    }

    fn configure_runtime(&mut self, config: runtime::SignalWorkerConfig) {
        if self.runtime.is_none() {
            match runtime::SignalRuntimeHandle::spawn(config) {
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
        match runtime::SignalRuntimeHandle::spawn(config) {
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
        let mut received_update = false;
        for sample in event.samples.into_values() {
            let Some(value_param) = self.value_nodes_by_item.get(&sample.item_id).copied() else {
                continue;
            };
            self.value_output_nodes.insert(value_param);
            received_update = true;
            ctx.set_param(value_param, ParamValue::Float(sample.value));
            if self.base.log_incoming_enabled() {
                golden_core::log!(origin = module_id; format!(
                    "Signal '{}' updated: {:.6}",
                    sample.label, sample.value
                ));
            }

            if sample.cycles > 0 {
                crate::app::module::script_api::emit_script_callback(
                    ctx,
                    module_id,
                    SIGNAL_CYCLE_CALLBACK,
                    vec![
                        serde_json::json!(sample.label),
                        serde_json::json!(sample.cycles),
                        serde_json::json!({
                            "name": sample.label,
                            "cycles": sample.cycles,
                            "cycle": sample.cycle,
                            "value": sample.value,
                        }),
                    ],
                );
            }
        }

        if received_update {
            self.base.emit_incoming_traffic(ctx);
        }
    }

    fn collect_signals(&self, snapshot: &ProcessTreeSnapshot) -> Vec<SignalConfig> {
        let Some(list_id) = self.signal_list_id(snapshot) else {
            return Vec::new();
        };

        snapshot
            .child_ids(list_id)
            .into_iter()
            .filter_map(|item_id| signal_config(snapshot, item_id))
            .collect()
    }

    fn signal_list_id(&self, snapshot: &ProcessTreeSnapshot) -> Option<NodeId> {
        let parameters_id = self.base.parameters_id()?;
        snapshot.find_child(parameters_id, "signals")
    }

    fn values_root(&self, _snapshot: &ProcessTreeSnapshot) -> Option<NodeId> {
        self.base.values_id()
    }

    fn ensure_default_signal(&self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) -> bool {
        let Some(list_id) = self.signal_list_id(snapshot) else {
            return false;
        };

        if !snapshot.child_ids(list_id).is_empty() {
            return false;
        }

        let mut signal = SignalItem::new();
        if let Some(uuid) = self.restored_default_signal_uuid(snapshot) {
            signal.node_data_mut().meta.uuid = uuid;
        }
        ctx.add_user_item_boxed(list_id, Box::new(signal), None);
        true
    }

    fn restored_default_signal_uuid(&self, snapshot: &ProcessTreeSnapshot) -> Option<NodeUuid> {
        let values_root = self.values_root(snapshot)?;
        snapshot
            .child_ids(values_root)
            .into_iter()
            .filter_map(|child_id| snapshot.node(child_id))
            .filter_map(|child| child.decl_id.strip_prefix("signal_"))
            .filter_map(|uuid| uuid::Uuid::parse_str(uuid).ok().map(NodeUuid))
            .find(|uuid| snapshot.node_id_by_uuid(*uuid).is_none())
    }

    fn reset_matching_signals(
        &mut self,
        snapshot: &ProcessTreeSnapshot,
        selector: Option<&ParamValue>,
    ) -> Result<(), String> {
        let configs = self.collect_signals(snapshot);
        let mut matched_ids = Vec::new();
        for (index, config) in configs.iter().enumerate() {
            if !selector_matches(snapshot, index, config.item_id, config.label.as_str(), selector) {
                continue;
            }
            matched_ids.push(config.item_id);
        }

        if matched_ids.is_empty() {
            return Err("no matching signal".to_string());
        }

        let runtime = self
            .runtime
            .as_ref()
            .ok_or_else(|| "Signals worker is not running".to_string())?;
        runtime.reset(matched_ids)
    }

    fn handle_script_method(
        &mut self,
        snapshot: &ProcessTreeSnapshot,
        method: &str,
        args: &[ParamValue],
    ) -> Option<Result<(), String>> {
        match method {
            "resetSignals" => Some(self.reset_matching_signals(snapshot, None)),
            "resetSignal" => Some(self.reset_matching_signals(snapshot, args.first())),
            _ => None,
        }
    }

    fn is_signal_configuration_event(&self, snapshot: &ProcessTreeSnapshot, node_id: NodeId) -> bool {
        let Some(list_id) = self.signal_list_id(snapshot) else {
            return false;
        };
        node_is_descendant_or_self(snapshot, node_id, list_id)
            || self.update_rate_hz.is_bound() && self.update_rate_hz.id() == node_id
    }

    fn is_value_output(&self, node_id: NodeId) -> bool {
        self.value_output_nodes.contains(&node_id)
    }

    fn handle_signal_enablement_event(&mut self, ctx: &mut ProcessCtx, event: &CustomEvent) {
        if event.topic != SIGNAL_ENABLEMENT_CHANGED_EVENT {
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
        let Some(list_id) = self.signal_list_id(snapshot) else {
            return;
        };
        if !node_is_descendant_or_self(snapshot, origin, list_id) {
            return;
        }
        self.config_dirty = true;
        self.sync_configuration(ctx, snapshot);
    }
}

#[golden_core::item(
    "module",
    node = "signals_module",
    via = base,
    from_struct,
    menu_path = ["Generators"]
)]
impl Node for SignalsModule {
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
            if self.ensure_default_signal(ctx, snapshot) {
                self.config_dirty = true;
                return;
            }
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

    fn inbox_requires_tree_snapshot(&self, _events: &EventFrame) -> bool {
        false
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::periodic(runtime_update_rate_hz(self.update_rate_hz.get()))
    }

    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        crate::app::module::script_api::descriptor_for_node(
            self.node_data(),
            self.get_type(),
            SIGNAL_SCRIPT_METHODS,
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
        if let Some(result) = self.handle_script_method(snapshot_arc.as_ref(), method, args) {
            result?;
            return Ok(true);
        }

        self.base.engine_call_script_method(ctx, method, args)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, old_value: ParamValue) {
        if self.is_value_output(param) {
            return;
        }

        self.config_dirty = true;

        if self.update_rate_hz.is_bound() && self.update_rate_hz.id() == param {
            ctx.reevaluate_graph();
        }

        if let Some(snapshot_arc) = ctx.tree_snapshot_arc() {
            let snapshot = snapshot_arc.as_ref();
            if self.is_signal_configuration_event(snapshot, param) {
                self.config_dirty = true;
                self.sync_configuration(ctx, snapshot);
            }
            self.base
                .emit_script_param_callback(ctx, snapshot, param, &old_value);
        }
    }

    fn on_child_added(&mut self, ctx: &mut ProcessCtx, parent: NodeId, child: NodeId) {
        self.config_dirty = true;
        if let Some(snapshot_arc) = ctx.tree_snapshot_arc() {
            let snapshot = snapshot_arc.as_ref();
            if self.is_signal_configuration_event(snapshot, parent)
                || self.is_signal_configuration_event(snapshot, child)
            {
                self.config_dirty = true;
                self.sync_configuration(ctx, snapshot);
            }
        }
    }

    fn on_child_removed(&mut self, ctx: &mut ProcessCtx, parent: NodeId, child: NodeId) {
        self.config_dirty = true;
        self.value_nodes_by_item.retain(|_, value| *value != child);
        self.value_output_nodes.remove(&child);
        self.pending_value_items.remove(&child);
        if let Some(snapshot_arc) = ctx.tree_snapshot_arc() {
            let snapshot = snapshot_arc.as_ref();
            if self.is_signal_configuration_event(snapshot, parent) {
                self.config_dirty = true;
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
        self.handle_signal_enablement_event(ctx, &event);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

#[node("signal_list", label = "Signals")]
pub struct SignalList {}

#[node("signal_list", from_struct)]
impl Node for SignalList {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_authoring(self.node_data_mut());
        self.node_data_mut().meta.can_be_disabled = false;
    }

    // Lifecycle hooks never read ctx.tree_snapshot(); opting out avoids a whole-tree
    // snapshot per inserted list on bulk paths (project load adds one per module).
    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[SIGNAL_ITEM_KIND]))
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![
            UserCreatableItem::new(SignalItem::NODE_TYPE, SIGNAL_ITEM_KIND, SIGNAL_ITEM_LABEL)
                .with_select_when_created(false),
        ]
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        (node_type == SignalItem::NODE_TYPE || node_type == SIGNAL_ITEM_KIND)
            .then(|| Box::new(SignalItem::new()) as Box<dyn Node>)
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("signal", label = "Signal")]
#[children(
    shape: Enum = SIGNAL_SHAPE_SINE (
        label = "Shape",
        description = "Signal shape used to produce the normalized value.",
        enum_options = signal_shape_options()
    );
    frequency_hz: f64 = 1.0 (
        label = "Frequency",
        description = "Signal frequency in hertz."
    );
    phase: f64 = 0.0 (
        label = "Phase",
        description = "Phase offset in cycles."
    );
    range: Vec2 = (0.0, 1.0) (
        label = "Range",
        description = "Minimum and maximum output value."
    );
    seed: i32 = 0 (
        label = "Seed",
        description = "Deterministic seed used for random shapes.",
        dependency = shape == "randomPerlin" || shape == "randomPure"
    );
)]
pub struct SignalItem {}

#[node("signal", from_struct)]
impl Node for SignalItem {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_authoring(self.node_data_mut());
        if let Some(snapshot_arc) = ctx.tree_snapshot_arc() {
            sync_signal_curve_child(
                ctx,
                snapshot_arc.as_ref(),
                self.id(),
                self.shape.get_ref().as_str(),
            );
        }
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        if !self.shape.is_bound() || self.shape.id() != param {
            return;
        }
        if let Some(snapshot_arc) = ctx.tree_snapshot_arc() {
            sync_signal_curve_child(
                ctx,
                snapshot_arc.as_ref(),
                self.id(),
                self.shape.get_ref().as_str(),
            );
        }
    }

    fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, _enabled: bool) {
        ctx.emit_custom_event(CustomEvent::new(
            SIGNAL_ENABLEMENT_CHANGED_EVENT,
            Some(self.id()),
            serde_json::Value::Null,
        ));
    }

    fn inbox_requires_tree_snapshot(&self, events: &EventFrame) -> bool {
        events.iter().any(|event| match &event.kind {
            EventKind::ParamChanged { param, .. } => self.shape.is_bound() && self.shape.id() == *param,
            _ => true,
        })
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[derive(Clone, Copy, Debug)]
struct SignalSample {
    value: f64,
    cycle: i64,
}

fn sample_signal(config: &SignalConfig, state: &mut SignalRuntimeState) -> SignalSample {
    let phase_position = config.phase + state.elapsed_seconds * config.frequency_hz;
    let phase = normalized_phase(phase_position);
    let cycle = phase_position.floor() as i64;
    let unit = match config.shape {
        SignalShape::Sine => 0.5 + 0.5 * (phase * PI * 2.0).sin(),
        SignalShape::Triangle => 1.0 - (2.0 * phase - 1.0).abs(),
        SignalShape::Saw => phase,
        SignalShape::ReverseSaw => 1.0 - phase,
        SignalShape::RandomPerlin => smooth_noise(config.seed as i64, phase_position),
        SignalShape::RandomPure => pure_random_unit(config, state, cycle),
        SignalShape::Curve => curve_unit(config, phase),
    }
    .clamp(0.0, 1.0);

    SignalSample {
        value: config.range_min + (config.range_max - config.range_min) * unit,
        cycle,
    }
}

fn reset_signal_state_if_config_changed(state: &mut SignalRuntimeState, config: &SignalConfig) {
    let signature = config.signature();
    if state.signature == Some(signature) {
        return;
    }
    state.elapsed_seconds = 0.0;
    state.last_cycle = config.phase.floor() as i64;
    state.sampled_once = false;
    state.pure_random_step = i64::MIN;
    state.pure_random_value = 0.0;
    state.signature = Some(signature);
}

fn runtime_update_rate_hz(value: i32) -> u32 {
    value
        .clamp(1, SIGNALS_UPDATE_RATE_MAX_HZ)
        .try_into()
        .unwrap_or(SIGNALS_UPDATE_RATE_DEFAULT_HZ as u32)
}

fn signal_shape_options() -> Vec<ParameterEnumOption> {
    vec![
        enum_option(SIGNAL_SHAPE_SINE, "Sine", 0),
        enum_option(SIGNAL_SHAPE_TRIANGLE, "Triangle", 1),
        enum_option(SIGNAL_SHAPE_SAW, "Saw", 2),
        enum_option(SIGNAL_SHAPE_REVERSE_SAW, "Reverse Saw", 3),
        enum_option(SIGNAL_SHAPE_RANDOM_PERLIN, "Random Perlin", 4),
        enum_option(SIGNAL_SHAPE_RANDOM_PURE, "Random Pure", 5),
        enum_option(SIGNAL_SHAPE_CURVE, "Curve", 6),
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

fn signal_config(snapshot: &ProcessTreeSnapshot, item_id: NodeId) -> Option<SignalConfig> {
    let item = snapshot.node(item_id)?;
    if item.node_type != SignalItem::NODE_TYPE {
        return None;
    }

    let (range_min, range_max) = child_vec2(snapshot, item_id, "range", (0.0, 1.0));
    let shape = parse_signal_shape(child_enum(snapshot, item_id, "shape", SIGNAL_SHAPE_SINE).as_str());
    let curve = (shape == SignalShape::Curve)
        .then(|| {
            snapshot
                .find_child(item_id, "curve")
                .and_then(|curve_id| curve_from_snapshot(snapshot, curve_id))
        })
        .flatten();
    Some(SignalConfig {
        item_id,
        value_decl_id: value_folder_decl_id("signal", item),
        label: item.label.clone(),
        enabled: item.enabled,
        shape,
        frequency_hz: child_float(snapshot, item_id, "frequency_hz", 1.0),
        phase: child_float(snapshot, item_id, "phase", 0.0),
        range_min,
        range_max,
        curve,
        seed: child_int(snapshot, item_id, "seed", 0),
    })
}

fn sync_signal_curve_child(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    item_id: NodeId,
    shape: &str,
) {
    let curve_ids: Vec<NodeId> = snapshot
        .child_ids(item_id)
        .into_iter()
        .filter(|child_id| {
            snapshot.node(*child_id).is_some_and(|child| {
                child.decl_id == "curve"
                    || child.decl_id.rsplit('/').next() == Some("curve")
                    || child.short_name == "curve"
            })
        })
        .collect();

    if shape == SIGNAL_SHAPE_CURVE {
        if curve_ids.is_empty() {
            ctx.add_child_tree(item_id, signal_curve_tree(), None);
            return;
        }

        for duplicate_id in curve_ids.into_iter().skip(1) {
            NodeHandle::new(duplicate_id).remove(ctx);
        }
        return;
    }

    for curve_id in curve_ids {
        NodeHandle::new(curve_id).remove(ctx);
    }
}

fn signal_curve_tree() -> NodeTree {
    let mut curve = CurveNode::new();
    crate::app::module::enable_module_authoring(curve.node_data_mut());
    let meta = &mut curve.node_data_mut().meta;
    meta.decl_id = DeclId("curve".to_string());
    meta.short_name = "curve".to_string();
    meta.label = "Curve".to_string();
    meta.description = Some("Custom normalized curve sampled when Shape is Curve.".to_string());
    NodeTree::new(curve)
}

fn parse_signal_shape(shape: &str) -> SignalShape {
    match shape.trim() {
        SIGNAL_SHAPE_TRIANGLE => SignalShape::Triangle,
        SIGNAL_SHAPE_SAW => SignalShape::Saw,
        SIGNAL_SHAPE_REVERSE_SAW => SignalShape::ReverseSaw,
        SIGNAL_SHAPE_RANDOM_PERLIN => SignalShape::RandomPerlin,
        SIGNAL_SHAPE_RANDOM_PURE => SignalShape::RandomPure,
        SIGNAL_SHAPE_CURVE => SignalShape::Curve,
        _ => SignalShape::Sine,
    }
}

fn curve_unit(config: &SignalConfig, phase: f64) -> f64 {
    config
        .curve
        .as_ref()
        .and_then(|curve| curve.sample(phase))
        .unwrap_or(0.0)
}

fn pure_random_unit(
    config: &SignalConfig,
    state: &mut SignalRuntimeState,
    step: i64,
) -> f64 {
    if state.pure_random_step != step {
        state.pure_random_step = step;
        state.pure_random_value = hash_unit(config.seed as i64, step);
    }
    state.pure_random_value
}

fn smooth_noise(seed: i64, position: f64) -> f64 {
    let left = position.floor();
    let right = left + 1.0;
    let t = smooth_step(position - left);
    let a = hash_unit(seed, left as i64);
    let b = hash_unit(seed, right as i64);
    a + (b - a) * t
}

fn smooth_step(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn normalized_phase(value: f64) -> f64 {
    let phase = value.fract();
    if phase.is_sign_negative() {
        phase + 1.0
    } else {
        phase
    }
}

fn sync_value_nodes<'a, I>(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    root_id: NodeId,
    configs: I,
    value_nodes_by_item: &mut HashMap<NodeId, NodeId>,
    value_output_nodes: &mut HashSet<NodeId>,
    pending_value_items: &mut HashSet<NodeId>,
) -> bool
where
    I: IntoIterator<Item = &'a SignalConfig>,
{
    let configs = configs.into_iter().collect::<Vec<_>>();
    let config_count = configs.len();
    let existing_by_decl = child_ids_by_decl(snapshot, root_id);
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
                if config_count == 1 {
                    adoptable_persisted_signal_value(snapshot, root_id, config, &used_node_ids)
                } else {
                    None
                }
            });

        match existing_node_id {
            Some(node_id) => {
                pending_value_items.remove(&config.item_id);
                used_node_ids.insert(node_id);
                next_value_nodes_by_item.insert(config.item_id, node_id);
                if let Some(node) = snapshot.node(node_id) {
                    if node.label != config.label {
                        ctx.patch_node_meta(
                            node_id,
                            NodeMetaPatch {
                                label: Some(config.label.clone()),
                                ..Default::default()
                            },
                        );
                    }
                    let next_constraints = signal_value_constraints(config);
                    if node.param_constraints.as_ref().map(|constraints| &constraints.range)
                        != Some(&next_constraints.range)
                    {
                        let value = node
                            .param_value
                            .clone()
                            .unwrap_or_else(|| ParamValue::Float(config.range_min));
                        NodeHandle::new(node_id).replace_with(
                            ctx,
                            signal_value_param(config.label.as_str(), config.value_decl_id.as_str(), value, config),
                        );
                    }
                }
            }
            None => {
                waiting_for_values = true;
                if pending_value_items.insert(config.item_id) {
                    ctx.add_child_tree(root_id, signal_values_tree(config), None);
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
        if !child.decl_id.starts_with("signal_") {
            continue;
        }
        NodeHandle::new(child_id).remove(ctx);
    }

    pending_value_items.retain(|item_id| active_item_ids.contains(item_id));
    *value_nodes_by_item = next_value_nodes_by_item;
    *value_output_nodes = value_nodes_by_item.values().copied().collect();
    waiting_for_values
}

fn adoptable_persisted_signal_value(
    snapshot: &ProcessTreeSnapshot,
    root_id: NodeId,
    config: &SignalConfig,
    used_node_ids: &HashSet<NodeId>,
) -> Option<NodeId> {
    snapshot.child_ids(root_id).into_iter().find(|child_id| {
        !used_node_ids.contains(child_id)
            && snapshot.node(*child_id).is_some_and(|child| {
                child.decl_id.starts_with("signal_")
                    && child.label == config.label
                    && child.param_value.is_some()
            })
    })
}

fn signal_values_tree(config: &SignalConfig) -> NodeTree {
    NodeTree::new(signal_value_param(
        config.label.as_str(),
        config.value_decl_id.as_str(),
        ParamValue::Float(config.range_min),
        config,
    ))
}

fn signal_value_param(label: &str, decl_id: &str, value: ParamValue, config: &SignalConfig) -> Parameter {
    let mut parameter = read_only_param(label, decl_id, value);
    parameter.constraints = signal_value_constraints(config);
    parameter
}

fn signal_value_constraints(config: &SignalConfig) -> golden_core::parameter::ParameterConstraints {
    let mut constraints = golden_core::parameter::ParameterConstraints::default();
    let min = config.range_min.min(config.range_max);
    let max = config.range_min.max(config.range_max);
    constraints.range = RangeConstraint::uniform(Some(min), Some(max));
    constraints
}

fn read_only_param(label: &str, decl_id: &str, value: ParamValue) -> Parameter {
    let mut parameter = Parameter::new(label, value, ParameterChangeCheck::ValueChange);
    parameter.read_only = true;
    crate::app::module::enable_module_authoring(parameter.node_data_mut());
    let meta = &mut parameter.node_data_mut().meta;
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

fn value_folder_decl_id(prefix: &str, node: &ProcessTreeNodeSnapshot) -> String {
    format!("{}_{}", prefix, node.uuid.0.simple())
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
