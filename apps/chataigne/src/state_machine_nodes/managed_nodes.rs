use golden_core::{
    engine::NodeExecutionRule,
    events::{Event, EventFrame, EventKind},
    item, node,
    node::{
        Node, NodeCreationContext, NodeId, NodeMetaPatch, NodeReference, NodeUserPermissions,
        UserContainerRules, UserCreatableItem,
    },
    parameter::{ParamValue, ReferenceTargetKind},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

const INPUT_ITEM_KIND: &str = "sm_input";
use crate::app::module_command::{self, ModuleCommandParamOverrides};
use crate::app::state_machine_nodes_generic_commands::GENERIC_COMMAND_ITEM_KIND;
const GENERIC_OUTPUT_MENU_PATH: &str = "Generic";

// ─── Output scheduling (shared by OutputsManager and OutputGroup) ────────────
//
// Both containers expose the same trigger / delay / stagger / cancel parameters
// and firing behaviour: when triggered they fire each contained output (command
// or nested group) after `delay` seconds, offset by `stagger` seconds per
// output. Firing a nested group runs that group's own schedule, so delay/stagger
// compose recursively.
pub(crate) const OUTPUT_GROUP_NODE_TYPE: &str = "sm_output_group";
pub(crate) const OUTPUT_GROUP_ITEM_KIND: &str = "sm_output_group";
const OUTPUT_TRIGGER_DECL: &str = "trigger";
const OUTPUT_ADVANCED_DECL: &str = "advanced";
const OUTPUT_DELAY_DECL: &str = "delay";
const OUTPUT_STAGGER_DECL: &str = "stagger";
const OUTPUT_CANCEL_DECL: &str = "cancel_on_trigger";
const OUTPUT_SCHEDULE_UPDATE_RATE_HZ: u32 = 60;

/// Runtime-only queue of outputs waiting to fire (not persisted).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct OutputSchedule {
    pending: Vec<PendingOutput>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct PendingOutput {
    target: NodeId,
    remaining: f64,
    param_overrides: ModuleCommandParamOverrides,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct OutputRuntimeCache {
    dirty: bool,
    trigger_param: Option<NodeId>,
    delay_param: Option<NodeId>,
    stagger_param: Option<NodeId>,
    cancel_param: Option<NodeId>,
    delay: f64,
    stagger: f64,
    cancel_on_trigger: bool,
    outputs: Vec<NodeId>,
}

impl Default for OutputRuntimeCache {
    fn default() -> Self {
        Self {
            dirty: true,
            trigger_param: None,
            delay_param: None,
            stagger_param: None,
            cancel_param: None,
            delay: 0.0,
            stagger: 0.0,
            cancel_on_trigger: false,
            outputs: Vec::new(),
        }
    }
}

impl OutputSchedule {
    fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    fn on_trigger_cached(
        &mut self,
        ctx: &mut ProcessCtx,
        cache: &OutputRuntimeCache,
        param_overrides: ModuleCommandParamOverrides,
    ) {
        if cache.cancel_on_trigger {
            self.pending.clear();
        }

        for (index, child) in cache.outputs.iter().copied().enumerate() {
            let remaining = cache.delay + (index as f64) * cache.stagger;
            if remaining <= f64::EPSILON {
                let _ = module_command::emit_command_execute_with_overrides(
                    ctx,
                    child,
                    param_overrides.clone(),
                );
            } else {
                self.pending.push(PendingOutput {
                    target: child,
                    remaining,
                    param_overrides: param_overrides.clone(),
                });
            }
        }
    }

    /// Advances pending outputs by `delta_seconds` and fires the ones now due.
    fn tick(&mut self, ctx: &mut ProcessCtx, delta_seconds: f64) {
        if self.pending.is_empty() {
            return;
        }
        let mut due: Vec<PendingOutput> = Vec::new();
        self.pending.retain_mut(|pending| {
            pending.remaining -= delta_seconds;
            if pending.remaining <= f64::EPSILON {
                due.push(pending.clone());
                false
            } else {
                true
            }
        });
        for pending in due {
            let _ = module_command::emit_command_execute_with_overrides(
                ctx,
                pending.target,
                pending.param_overrides,
            );
        }
    }
}

fn refresh_output_runtime_cache(
    cache: &mut OutputRuntimeCache,
    snapshot: &ProcessTreeSnapshot,
    container: NodeId,
) {
    cache.trigger_param = snapshot.find_child_by_decl_id(container, OUTPUT_TRIGGER_DECL);
    cache.delay_param = output_advanced_child(snapshot, container, OUTPUT_DELAY_DECL);
    cache.stagger_param = output_advanced_child(snapshot, container, OUTPUT_STAGGER_DECL);
    cache.cancel_param = output_advanced_child(snapshot, container, OUTPUT_CANCEL_DECL);
    cache.delay = output_float_child(snapshot, container, OUTPUT_DELAY_DECL)
        .unwrap_or(0.0)
        .max(0.0);
    cache.stagger = output_float_child(snapshot, container, OUTPUT_STAGGER_DECL)
        .unwrap_or(0.0)
        .max(0.0);
    cache.cancel_on_trigger =
        output_bool_child(snapshot, container, OUTPUT_CANCEL_DECL).unwrap_or(false);
    cache.outputs = snapshot
        .child_ids(container)
        .into_iter()
        .filter(|child| is_output_node(snapshot, *child))
        .filter(|child| snapshot.node(*child).is_some_and(|node| node.enabled))
        .collect();
    cache.dirty = false;
}

fn output_cache_requires_snapshot(
    cache: &OutputRuntimeCache,
    container: NodeId,
    events: &EventFrame,
) -> bool {
    events.iter().any(|event| match &event.kind {
        EventKind::ChildAdded { .. } | EventKind::ChildRemoved { .. } => true,
        EventKind::ParamChanged { param, .. } => {
            cache.dirty
                || Some(*param) == cache.delay_param
                || Some(*param) == cache.stagger_param
                || Some(*param) == cache.cancel_param
        }
        EventKind::Custom(custom) => {
            cache.dirty && crate::app::module_command::is_command_execute_request(custom, container)
        }
        _ => true,
    })
}

/// `true` for nodes that behave as outputs: module commands, generic commands,
/// and nested output groups.
pub(crate) fn is_output_node(snapshot: &ProcessTreeSnapshot, node: NodeId) -> bool {
    snapshot.node(node).is_some_and(|node| {
        crate::app::declared_user_item_type_matches(
            &node.node_type,
            crate::app::module_command::MODULE_COMMAND_ITEM_KIND,
        ) || crate::app::declared_user_item_type_matches(&node.node_type, GENERIC_COMMAND_ITEM_KIND)
            || node.node_type == OUTPUT_GROUP_NODE_TYPE
    })
}

/// `true` when `node` is a container that schedules outputs (Outputs manager or
/// Output group).
pub(crate) fn is_output_container(snapshot: &ProcessTreeSnapshot, node: NodeId) -> bool {
    snapshot.node(node).is_some_and(|node| {
        node.node_type == OutputsManager::NODE_TYPE || node.node_type == OUTPUT_GROUP_NODE_TYPE
    })
}

fn output_float_child(
    snapshot: &ProcessTreeSnapshot,
    container: NodeId,
    decl_id: &str,
) -> Option<f64> {
    output_child_param(snapshot, container, decl_id).and_then(|value| value.as_float())
}

fn output_bool_child(
    snapshot: &ProcessTreeSnapshot,
    container: NodeId,
    decl_id: &str,
) -> Option<bool> {
    output_child_param(snapshot, container, decl_id).and_then(|value| value.as_bool())
}

fn output_child_param(
    snapshot: &ProcessTreeSnapshot,
    container: NodeId,
    decl_id: &str,
) -> Option<ParamValue> {
    let child = output_advanced_child(snapshot, container, decl_id)?;
    snapshot.node(child).and_then(|node| node.param_value.clone())
}

fn output_advanced_child(
    snapshot: &ProcessTreeSnapshot,
    container: NodeId,
    decl_id: &str,
) -> Option<NodeId> {
    let advanced = snapshot.find_child_by_decl_id(container, OUTPUT_ADVANCED_DECL)?;
    snapshot.find_child_by_decl_id(advanced, decl_id)
}

fn sync_output_control_visibility(ctx: &mut ProcessCtx, container: NodeId) {
    sync_output_control_visibility_with_change(ctx, container, OutputChildChange::None);
}

fn sync_output_control_visibility_after_child_added(
    ctx: &mut ProcessCtx,
    container: NodeId,
    child: NodeId,
) {
    sync_output_control_visibility_with_change(ctx, container, OutputChildChange::Added(child));
}

fn sync_output_control_visibility_after_child_removed(
    ctx: &mut ProcessCtx,
    container: NodeId,
    child: NodeId,
) {
    sync_output_control_visibility_with_change(ctx, container, OutputChildChange::Removed(child));
}

fn sync_output_control_visibility_with_change(
    ctx: &mut ProcessCtx,
    container: NodeId,
    change: OutputChildChange,
) {
    let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
        return;
    };
    let snapshot = snapshot_arc.as_ref();
    let stagger_visible = output_item_count_after_change(snapshot, container, change) >= 2;
    let cancel_visible = output_float_child(snapshot, container, OUTPUT_DELAY_DECL)
        .is_some_and(|delay| delay > f64::EPSILON)
        || output_float_child(snapshot, container, OUTPUT_STAGGER_DECL)
            .is_some_and(|stagger| stagger > f64::EPSILON);

    patch_output_control_visibility(
        ctx,
        snapshot,
        container,
        OUTPUT_STAGGER_DECL,
        stagger_visible,
    );
    patch_output_control_visibility(
        ctx,
        snapshot,
        container,
        OUTPUT_CANCEL_DECL,
        cancel_visible,
    );
}

fn patch_output_control_visibility(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    container: NodeId,
    decl_id: &str,
    visible: bool,
) {
    let Some(control_id) = output_advanced_child(snapshot, container, decl_id) else {
        return;
    };
    let Some(control) = snapshot.node(control_id) else {
        return;
    };
    if control.presentation.show_in_inspector_content == visible {
        return;
    }

    let mut presentation = control.presentation.clone();
    presentation.show_in_inspector_content = visible;
    ctx.patch_node_meta(
        control_id,
        NodeMetaPatch {
            presentation: Some(presentation),
            ..NodeMetaPatch::default()
        },
    );
}

#[derive(Clone, Copy)]
enum OutputChildChange {
    None,
    Added(NodeId),
    Removed(NodeId),
}

fn output_item_count_after_change(
    snapshot: &ProcessTreeSnapshot,
    container: NodeId,
    change: OutputChildChange,
) -> usize {
    let child_ids = snapshot.child_ids(container);
    let count = child_ids
        .iter()
        .filter(|child| is_output_node(snapshot, **child))
        .count() as isize;
    let adjusted = match change {
        OutputChildChange::None => count,
        OutputChildChange::Added(child) if !child_ids.contains(&child) => count + 1,
        OutputChildChange::Removed(child)
            if child_ids.contains(&child) && is_output_node(snapshot, child) =>
        {
            count - 1
        }
        _ => count,
    };
    adjusted.max(0) as usize
}

fn locked_manager_permissions() -> NodeUserPermissions {
    let mut p = NodeUserPermissions::all();
    p.can_remove_and_duplicate = false;
    p.can_edit_name = false;
    p
}

#[node("sm_condition_manager", label = "Conditions")]
#[children(
    operator: golden_core::parameter::Enum = "all" (
        label = "Operator",
        show_in_inspector_content = false,
        enum_options = ["all", "any", "none", "at_least", "exactly"]
    );
    operator_count: f64 = 1.0 (
        label = "Count",
        show_in_inspector_content = false
    );
    valid: bool = false (
        label = "Valid",
        read_only = true,
        show_in_inspector_content = false
    );
)]
pub struct ConditionManager {}

#[node("sm_condition_manager", from_struct)]
impl Node for ConditionManager {
    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        crate::app::state_machine_nodes_conditions::sync_condition_operator_visibility(ctx, self.id());
    }

    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        match event.kind {
            EventKind::ChildAdded { .. } | EventKind::ChildRemoved { .. } => u32::MAX,
            _ => 0,
        }
    }

    fn on_child_added(&mut self, ctx: &mut ProcessCtx, parent: NodeId, _child: NodeId) {
        if parent == self.id() {
            crate::app::state_machine_nodes_conditions::sync_condition_operator_visibility_after_child_added(
                ctx,
                self.id(),
                _child,
            );
        }
    }

    fn on_child_removed(&mut self, ctx: &mut ProcessCtx, parent: NodeId, _child: NodeId) {
        if parent == self.id() {
            crate::app::state_machine_nodes_conditions::sync_condition_operator_visibility_after_child_removed(
                ctx,
                self.id(),
                _child,
            );
        }
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&["sm_condition"]))
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        item_kind == "sm_condition"
            && crate::app::declared_user_item_type_matches(item_type, "sm_condition")
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        crate::app::state_machine_nodes_conditions::ordered_condition_creatable_items()
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        crate::app::create_declared_user_item(node_type, "sm_condition")
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = locked_manager_permissions();
        self.node_data_mut().meta.can_be_disabled = false;
    }
}

#[node("sm_consequences_manager", label = "Consequences")]
pub struct ConsequencesManager {}

#[node("sm_consequences_manager", from_struct)]
impl Node for ConsequencesManager {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&["sm_consequence"]))
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        item_kind == "sm_consequence"
            && crate::app::declared_user_item_type_matches(item_type, "sm_consequence")
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        crate::app::declared_user_creatable_items("sm_consequence")
            .into_iter()
            .map(|item| item.with_select_when_created(false))
            .collect()
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        crate::app::create_declared_user_item(node_type, "sm_consequence")
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = locked_manager_permissions();
        self.node_data_mut().meta.can_be_disabled = false;
    }
}

#[node("sm_inputs_manager", label = "Inputs")]
pub struct InputsManager {}

#[node("sm_inputs_manager", from_struct)]
impl Node for InputsManager {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[INPUT_ITEM_KIND]))
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        item_kind == INPUT_ITEM_KIND
            && crate::app::declared_user_item_type_matches(item_type, INPUT_ITEM_KIND)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        crate::app::declared_user_creatable_items(INPUT_ITEM_KIND)
            .into_iter()
            .map(|item| item.with_select_when_created(false))
            .collect()
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        crate::app::create_declared_user_item(node_type, INPUT_ITEM_KIND)
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = locked_manager_permissions();
        self.node_data_mut().meta.can_be_disabled = false;
    }
}

#[node("sm_input_source", label = "Input Source")]
#[children(
    source: NodeReference (
        label = "Source",
        reference_target_kind = ReferenceTargetKind::ParameterOnly
    );
)]
pub struct InputSource {}

#[item("sm_input", node = "sm_input_source", from_struct)]
impl Node for InputSource {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("sm_filter_chain_manager", label = "Filter Chain")]
pub struct FilterChainManager {}

#[node("sm_filter_chain_manager", from_struct)]
impl Node for FilterChainManager {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&["sm_filter"]))
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        item_kind == "sm_filter"
            && crate::app::declared_user_item_type_matches(item_type, "sm_filter")
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        crate::app::declared_user_creatable_items("sm_filter")
            .into_iter()
            .map(|item| item.with_select_when_created(false))
            .collect()
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        crate::app::create_declared_user_item(node_type, "sm_filter")
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = locked_manager_permissions();
        self.node_data_mut().meta.can_be_disabled = false;
    }
}

// The Outputs manager and Output groups declare identical trigger / delay /
// stagger / cancel children and share their scheduling behaviour. The child
// declarations are repeated on each struct because `#[children(...)]` must
// annotate the struct directly.
#[node("sm_outputs_manager", label = "Outputs")]
#[children(
    trigger: ParamValue = ParamValue::Trigger() (
        label = "Trigger",
        description = "Fire all contained outputs, respecting Delay and Stagger.",
        show_in_inspector_content = false
    );
    folder(advanced, label = "Advanced", collapsed = true) {
        delay: f64 = 0.0 [0.0..] (
            label = "Delay",
            description = "Seconds to wait before firing outputs.",
            widget = "time_slider"
        );
        stagger: f64 = 0.0 [0.0..] (
            label = "Stagger",
            description = "Seconds to wait between each fired output.",
            widget = "time_slider",
            show_in_inspector_content = false
        );
        cancel_on_trigger: bool = false (
            label = "Cancel Pending On Trigger",
            description = "Enabled: a new trigger cancels any still-pending outputs first. Disabled: each trigger queues its own outputs.",
            show_in_inspector_content = false
        );
    }
)]
pub struct OutputsManager {
    #[state(default = OutputSchedule::default())]
    schedule: OutputSchedule,
    #[state(default = OutputRuntimeCache::default())]
    output_cache: OutputRuntimeCache,
}

fn output_generic_items() -> Vec<UserCreatableItem> {
    crate::app::declared_user_creatable_items(GENERIC_COMMAND_ITEM_KIND)
        .into_iter()
        .map(|item| {
            item.with_menu_path([GENERIC_OUTPUT_MENU_PATH])
                .with_select_when_created(false)
        })
        .collect()
}

fn output_group_item() -> UserCreatableItem {
    UserCreatableItem::new(OUTPUT_GROUP_NODE_TYPE, OUTPUT_GROUP_ITEM_KIND, "Group")
        .with_select_when_created(false)
}

fn output_container_accepts_item(item_type: &str, item_kind: &str) -> bool {
    item_kind == GENERIC_COMMAND_ITEM_KIND
        && crate::app::declared_user_item_type_matches(item_type, GENERIC_COMMAND_ITEM_KIND)
        || item_kind == crate::app::module_command::MODULE_COMMAND_ITEM_KIND
            && crate::app::declared_user_item_type_matches(
                item_type,
                crate::app::module_command::MODULE_COMMAND_ITEM_KIND,
            )
        || item_kind == OUTPUT_GROUP_ITEM_KIND && item_type == OUTPUT_GROUP_NODE_TYPE
}

fn output_container_create_item(node_type: &str) -> Option<Box<dyn Node>> {
    crate::app::create_declared_user_item(node_type, GENERIC_COMMAND_ITEM_KIND)
        .or_else(|| {
            crate::app::create_declared_user_item(
                node_type,
                crate::app::module_command::MODULE_COMMAND_ITEM_KIND,
            )
        })
        .or_else(|| crate::app::create_declared_user_item(node_type, OUTPUT_GROUP_ITEM_KIND))
}

fn collect_module_roots(snapshot: &ProcessTreeSnapshot) -> Vec<NodeId> {
    let mut modules = Vec::new();
    let mut stack = vec![snapshot.root()];
    while let Some(node_id) = stack.pop() {
        let Some(node) = snapshot.node(node_id) else {
            continue;
        };
        if crate::app::declared_user_item_type_matches(
            &node.node_type,
            crate::app::module::MODULE_ITEM_KIND,
        ) {
            modules.push(node_id);
        }
        let mut children = snapshot.child_ids(node_id);
        children.reverse();
        stack.extend(children);
    }
    modules
}

fn output_module_command_items(
    snapshot: &ProcessTreeSnapshot,
    child_catalog: &dyn Fn(NodeId) -> Vec<UserCreatableItem>,
) -> Vec<UserCreatableItem> {
    collect_module_roots(snapshot)
        .into_iter()
        .flat_map(|module_id| {
            let Some(module) = snapshot.node(module_id) else {
                return Vec::new();
            };
            let Some(command_tester) = snapshot.find_child_by_decl_id(module_id, "command_tester")
            else {
                return Vec::new();
            };
            child_catalog(command_tester)
                .into_iter()
                .filter(|item| item.item_kind == crate::app::module_command::MODULE_COMMAND_ITEM_KIND)
                .map(|item| {
                    let mut menu_path = Vec::with_capacity(item.menu_path.len() + 1);
                    menu_path.push(module.label.clone());
                    menu_path.extend(item.menu_path.iter().cloned());
                    item.with_menu_path(menu_path).with_initial_param(
                        crate::app::module_command::MODULE_COMMAND_TARGET_MODULE_PATH,
                        ParamValue::Reference(NodeReference::new(module.uuid)),
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

#[node("sm_outputs_manager", from_struct)]
impl Node for OutputsManager {
    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        let id = self.id();
        sync_output_control_visibility(ctx, id);
        if let Some(snapshot) = ctx.tree_snapshot() {
            refresh_output_runtime_cache(&mut self.output_cache, snapshot, id);
        } else {
            self.output_cache.dirty = true;
        }
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[
            GENERIC_COMMAND_ITEM_KIND,
            crate::app::module_command::MODULE_COMMAND_ITEM_KIND,
            OUTPUT_GROUP_ITEM_KIND,
        ]))
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        output_container_accepts_item(item_type, item_kind)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        output_generic_items()
            .into_iter()
            .chain(std::iter::once(output_group_item()))
            .collect()
    }

    fn user_creatable_items_with_context(
        &self,
        snapshot: &ProcessTreeSnapshot,
        _parent: NodeId,
        child_catalog: &dyn Fn(NodeId) -> Vec<UserCreatableItem>,
    ) -> Vec<UserCreatableItem> {
        output_module_command_items(snapshot, child_catalog)
            .into_iter()
            .chain(output_generic_items())
            .chain(std::iter::once(output_group_item()))
            .collect()
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        output_container_create_item(node_type)
    }

    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        match event.kind {
            EventKind::ChildAdded { .. }
            | EventKind::ChildRemoved { .. }
            | EventKind::ParamChanged { .. } => u32::MAX,
            _ => 0,
        }
    }

    fn on_child_added(&mut self, ctx: &mut ProcessCtx, parent: NodeId, child: NodeId) {
        let id = self.id();
        if parent == id {
            sync_output_control_visibility_after_child_added(ctx, id, child);
            if let Some(snapshot) = ctx.tree_snapshot() {
                refresh_output_runtime_cache(&mut self.output_cache, snapshot, id);
            } else {
                self.output_cache.dirty = true;
            }
        }
    }

    fn on_child_removed(&mut self, ctx: &mut ProcessCtx, parent: NodeId, child: NodeId) {
        let id = self.id();
        if parent == id {
            sync_output_control_visibility_after_child_removed(ctx, id, child);
            if let Some(snapshot) = ctx.tree_snapshot() {
                refresh_output_runtime_cache(&mut self.output_cache, snapshot, id);
            } else {
                self.output_cache.dirty = true;
            }
        }
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        let id = self.id();
        if self.output_cache.dirty {
            if let Some(snapshot) = ctx.tree_snapshot() {
                refresh_output_runtime_cache(&mut self.output_cache, snapshot, id);
            }
        }
        if Some(param) == self.output_cache.trigger_param {
            self.schedule
                .on_trigger_cached(ctx, &self.output_cache, Vec::new());
        }
        if Some(param) == self.output_cache.delay_param
            || Some(param) == self.output_cache.stagger_param
            || Some(param) == self.output_cache.cancel_param
        {
            if let Some(snapshot) = ctx.tree_snapshot() {
                refresh_output_runtime_cache(&mut self.output_cache, snapshot, id);
            } else {
                self.output_cache.dirty = true;
            }
            sync_output_control_visibility(ctx, id);
        }
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        let id = self.id();
        if let Some(param_overrides) = module_command::command_execute_param_overrides(&event, id) {
            if self.output_cache.dirty {
                if let Some(snapshot) = ctx.tree_snapshot() {
                    refresh_output_runtime_cache(&mut self.output_cache, snapshot, id);
                }
            }
            self.schedule
                .on_trigger_cached(ctx, &self.output_cache, param_overrides);
        }
    }

    fn inbox_requires_tree_snapshot(&self, events: &EventFrame) -> bool {
        output_cache_requires_snapshot(&self.output_cache, self.id(), events)
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        let delta = ctx.delta_time.as_secs_f64();
        self.schedule.tick(ctx, delta);
    }

    fn needs_update(&self) -> bool {
        !self.schedule.is_empty()
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::periodic(OUTPUT_SCHEDULE_UPDATE_RATE_HZ)
            .with_compiled_kernel("chataigne.runtime.state-machine-outputs")
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = locked_manager_permissions();
        self.node_data_mut().meta.can_be_disabled = false;
    }

}

// ─── OutputGroup ─────────────────────────────────────────────────────────────
//
// A nestable group of outputs, mirroring Condition Group. It is both an output
// item (creatable inside the Outputs manager and other groups) and a container,
// and shares the Outputs manager's trigger / delay / stagger / cancel behaviour.
#[node("sm_output_group", label = "Group")]
#[children(
    trigger: ParamValue = ParamValue::Trigger() (
        label = "Trigger",
        description = "Fire all outputs in this group, respecting Delay and Stagger.",
        show_in_inspector_content = false
    );
    folder(advanced, label = "Advanced", collapsed = true) {
        delay: f64 = 0.0 [0.0..] (
            label = "Delay",
            description = "Seconds to wait before firing this group's outputs.",
            widget = "time_slider"
        );
        stagger: f64 = 0.0 [0.0..] (
            label = "Stagger",
            description = "Seconds to wait between each fired output.",
            widget = "time_slider",
            show_in_inspector_content = false
        );
        cancel_on_trigger: bool = false (
            label = "Cancel Pending On Trigger",
            description = "Enabled: a new trigger cancels any still-pending outputs first. Disabled: each trigger queues its own outputs.",
            show_in_inspector_content = false
        );
    }
)]
pub struct OutputGroup {
    #[state(default = OutputSchedule::default())]
    schedule: OutputSchedule,
    #[state(default = OutputRuntimeCache::default())]
    output_cache: OutputRuntimeCache,
}

#[item("sm_output_group", node = "sm_output_group", from_struct)]
impl Node for OutputGroup {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == OUTPUT_GROUP_NODE_TYPE).then(Self::new)
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        let id = self.id();
        sync_output_control_visibility(ctx, id);
        if let Some(snapshot) = ctx.tree_snapshot() {
            refresh_output_runtime_cache(&mut self.output_cache, snapshot, id);
        } else {
            self.output_cache.dirty = true;
        }
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[
            GENERIC_COMMAND_ITEM_KIND,
            crate::app::module_command::MODULE_COMMAND_ITEM_KIND,
            OUTPUT_GROUP_ITEM_KIND,
        ]))
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        output_container_accepts_item(item_type, item_kind)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        output_generic_items()
            .into_iter()
            .chain(std::iter::once(output_group_item()))
            .collect()
    }

    fn user_creatable_items_with_context(
        &self,
        snapshot: &ProcessTreeSnapshot,
        _parent: NodeId,
        child_catalog: &dyn Fn(NodeId) -> Vec<UserCreatableItem>,
    ) -> Vec<UserCreatableItem> {
        output_module_command_items(snapshot, child_catalog)
            .into_iter()
            .chain(output_generic_items())
            .chain(std::iter::once(output_group_item()))
            .collect()
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        output_container_create_item(node_type)
    }

    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        match event.kind {
            EventKind::ChildAdded { .. }
            | EventKind::ChildRemoved { .. }
            | EventKind::ParamChanged { .. } => u32::MAX,
            _ => 0,
        }
    }

    fn on_child_added(&mut self, ctx: &mut ProcessCtx, parent: NodeId, child: NodeId) {
        let id = self.id();
        if parent == id {
            sync_output_control_visibility_after_child_added(ctx, id, child);
            if let Some(snapshot) = ctx.tree_snapshot() {
                refresh_output_runtime_cache(&mut self.output_cache, snapshot, id);
            } else {
                self.output_cache.dirty = true;
            }
        }
    }

    fn on_child_removed(&mut self, ctx: &mut ProcessCtx, parent: NodeId, child: NodeId) {
        let id = self.id();
        if parent == id {
            sync_output_control_visibility_after_child_removed(ctx, id, child);
            if let Some(snapshot) = ctx.tree_snapshot() {
                refresh_output_runtime_cache(&mut self.output_cache, snapshot, id);
            } else {
                self.output_cache.dirty = true;
            }
        }
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        let id = self.id();
        if self.output_cache.dirty {
            if let Some(snapshot) = ctx.tree_snapshot() {
                refresh_output_runtime_cache(&mut self.output_cache, snapshot, id);
            }
        }
        if Some(param) == self.output_cache.trigger_param {
            self.schedule
                .on_trigger_cached(ctx, &self.output_cache, Vec::new());
        }
        if Some(param) == self.output_cache.delay_param
            || Some(param) == self.output_cache.stagger_param
            || Some(param) == self.output_cache.cancel_param
        {
            if let Some(snapshot) = ctx.tree_snapshot() {
                refresh_output_runtime_cache(&mut self.output_cache, snapshot, id);
            } else {
                self.output_cache.dirty = true;
            }
            sync_output_control_visibility(ctx, id);
        }
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        let id = self.id();
        if let Some(param_overrides) = module_command::command_execute_param_overrides(&event, id) {
            if self.output_cache.dirty {
                if let Some(snapshot) = ctx.tree_snapshot() {
                    refresh_output_runtime_cache(&mut self.output_cache, snapshot, id);
                }
            }
            self.schedule
                .on_trigger_cached(ctx, &self.output_cache, param_overrides);
        }
    }

    fn inbox_requires_tree_snapshot(&self, events: &EventFrame) -> bool {
        output_cache_requires_snapshot(&self.output_cache, self.id(), events)
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        let delta = ctx.delta_time.as_secs_f64();
        self.schedule.tick(ctx, delta);
    }

    fn needs_update(&self) -> bool {
        !self.schedule.is_empty()
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::periodic(OUTPUT_SCHEDULE_UPDATE_RATE_HZ)
            .with_compiled_kernel("chataigne.runtime.state-machine-outputs")
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
    }

}

#[cfg(test)]
mod managed_nodes_tests;
