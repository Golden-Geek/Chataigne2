use golden_core::{
    events::{Event, EventKind},
    item, node,
    node::{
        Node, NodeCreationContext, NodeId, NodeMetaPatch, NodeReference, NodeUserPermissions,
        UserContainerRules, UserCreatableItem,
    },
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

// ─── Shared init for all leaf conditions ─────────────────────────────────────
//
// All leaf conditions share the same `init` body. This macro removes that
// repetition. `project_create`, `project_encode_data`, and other Node
// boilerplate are provided by the `#[item(..., from_struct)]` proc-macro.

macro_rules! leaf_condition_init {
    () => {
        fn init(&mut self, _ctx: &mut ProcessCtx) {
            self.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
        }
    };
}

// ─── InputValueCondition ─────────────────────────────────────────────────────

/// Legacy condition wrapper retained for tree/UI persistence.
///
/// Runtime typing, projections, comparator choices, and reference visibility
/// must be supplied by that ANode declaration and lowering path. A Processor
/// must not interpret this node directly.
#[node("sm_input_value_condition", label = "Input Value")]
#[children(
    valid: bool = false (
        label = "Valid",
        read_only = true,
        show_in_inspector_content = false
    );
    toggle_mode: bool = false (
        label = "Toggle Mode",
        show_in_inspector_content = false
    );
    folder(advanced, label = "Advanced", collapsed = false) {
        validation_delay_s: f64 = 0.0 [0.0..] (
            label = "Validation Delay",
            widget = "time_slider"
        );
        invalidation_delay_s: f64 = 0.0 [0.0..] (
            label = "Invalidation Delay",
            widget = "time_slider"
        );
    }
    source: NodeReference (
        label = "Source",
        reference_target_kind = golden_core::parameter::ReferenceTargetKind::ParameterOnly
    );
    source_projection: golden_core::parameter::Enum = "none" (
        label = "Projection",
        show_in_inspector_content = false,
        enum_options = ["none"]
    );
    comparator: golden_core::parameter::Enum = "equal" (
        label = "Comparator",
        show_in_inspector_content = false,
        enum_options = [
            "equal",
            "not_equal",
            "greater_than",
            "greater_than_or_equal",
            "less_than",
            "less_than_or_equal",
            "between",
            "outside",
            "is_true",
            "is_false",
            "contains",
            "does_not_contain",
            "starts_with",
            "ends_with",
            "regex_match",
            "value_changed"
        ]
    );
    reference: f64 = 0.0 (
        label = "Reference",
        show_in_inspector_content = false
    );
    reference_max: f64 = 1.0 (
        label = "Reference Max",
        show_in_inspector_content = false
    );
    reference_string: String = String::new() (
        label = "Reference (String)",
        show_in_inspector_content = false
    );
)]
pub struct InputValueCondition {}

#[item("sm_condition", node = "sm_input_value_condition", from_struct)]
impl Node for InputValueCondition {
    leaf_condition_init!();
}

// ─── InputNodeCondition ──────────────────────────────────────────────────────

/// Legacy condition wrapper retained for tree/UI persistence.
///
/// Runtime endpoint lookup and comparison behavior must be supplied by a
/// managed condition ANode and lowering path. A Processor must not interpret
/// this node directly.
#[node("sm_input_node_condition", label = "Input Node")]
#[children(
    toggle_mode: bool = false (
        label = "Toggle Mode"
    );
    validation_delay_s: f64 = 0.0 (
        label = "Validation Delay (s)"
    );
    invalidation_delay_s: f64 = 0.0 (
        label = "Invalidation Delay (s)"
    );
    provider_node: NodeReference (
        label = "Node"
    );
    endpoint_id: String = String::new() (
        label = "Endpoint"
    );
    comparator: golden_core::parameter::Enum = "equal" (
        label = "Comparator",
        enum_options = [
            "equal",
            "not_equal",
            "greater_than",
            "greater_than_or_equal",
            "less_than",
            "less_than_or_equal",
            "between",
            "outside",
            "is_true",
            "is_false",
            "contains",
            "starts_with",
            "ends_with",
            "value_changed"
        ]
    );
    reference: f64 = 0.0 (
        label = "Reference"
    );
    reference_max: f64 = 1.0 (
        label = "Reference Max"
    );
    reference_string: String = String::new() (
        label = "Reference (String)",
        show_in_inspector_content = false
    );
)]
pub struct InputNodeCondition {}

#[item("sm_condition", node = "sm_input_node_condition", from_struct)]
impl Node for InputNodeCondition {
    leaf_condition_init!();
}

// ─── ScriptCondition ─────────────────────────────────────────────────────────

/// Legacy script-condition wrapper retained for tree/UI persistence.
///
/// Script execution must happen behind a managed condition ANode or host
/// scripting boundary. A Processor must not interpret this node directly.
#[node("sm_script_condition", label = "Script")]
#[children(
    toggle_mode: bool = false (
        label = "Toggle Mode"
    );
    validation_delay_s: f64 = 0.0 (
        label = "Validation Delay (s)"
    );
    invalidation_delay_s: f64 = 0.0 (
        label = "Invalidation Delay (s)"
    );
    script: String = String::new() (
        label = "Script",
        show_in_inspector_content = false
    );
)]
pub struct ScriptCondition {}

#[item("sm_condition", node = "sm_script_condition", from_struct)]
impl Node for ScriptCondition {
    leaf_condition_init!();
}

// ─── ConditionGroup ──────────────────────────────────────────────────────────

/// Legacy condition-group wrapper retained for tree/UI persistence.
///
/// Runtime reduction behavior must be supplied by a managed condition ANode and
/// lowering path. This wrapper only owns authored child organization.
#[node("sm_condition_group", label = "Condition Group")]
#[children(
    toggle_mode: bool = false (
        label = "Toggle Mode"
    );
    operator: golden_core::parameter::Enum = "all" (
        label = "Operator",
        show_in_inspector_content = false,
        enum_options = ["all", "any", "none", "at_least", "exactly"]
    );
    operator_count: f64 = 1.0 (
        label = "Count",
        show_in_inspector_content = false
    );
    validation_delay_s: f64 = 0.0 (
        label = "Validation Delay (s)"
    );
    invalidation_delay_s: f64 = 0.0 (
        label = "Invalidation Delay (s)"
    );
)]
pub struct ConditionGroup {}

#[item("sm_condition", node = "sm_condition_group", from_struct)]
impl Node for ConditionGroup {
    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        sync_condition_operator_visibility(ctx, self.id());
    }

    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        match event.kind {
            EventKind::ChildAdded { .. } | EventKind::ChildRemoved { .. } => u32::MAX,
            _ => 0,
        }
    }

    fn on_child_added(&mut self, ctx: &mut ProcessCtx, parent: NodeId, _child: NodeId) {
        if parent == self.id() {
            sync_condition_operator_visibility_after_child_added(ctx, self.id(), _child);
        }
    }

    fn on_child_removed(&mut self, ctx: &mut ProcessCtx, parent: NodeId, _child: NodeId) {
        if parent == self.id() {
            sync_condition_operator_visibility_after_child_removed(ctx, self.id(), _child);
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
        crate::app::declared_user_creatable_items("sm_condition")
            .into_iter()
            .map(|item| item.with_select_when_created(false))
            .collect()
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        crate::app::create_declared_user_item(node_type, "sm_condition")
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
    }
}

pub(crate) fn sync_condition_operator_visibility(ctx: &mut ProcessCtx, group: NodeId) {
    sync_condition_operator_visibility_with_change(ctx, group, ConditionChildChange::None);
}

pub(crate) fn sync_condition_operator_visibility_after_child_added(
    ctx: &mut ProcessCtx,
    group: NodeId,
    child: NodeId,
) {
    sync_condition_operator_visibility_with_change(ctx, group, ConditionChildChange::Added(child));
}

pub(crate) fn sync_condition_operator_visibility_after_child_removed(
    ctx: &mut ProcessCtx,
    group: NodeId,
    child: NodeId,
) {
    sync_condition_operator_visibility_with_change(ctx, group, ConditionChildChange::Removed(child));
}

fn sync_condition_operator_visibility_with_change(
    ctx: &mut ProcessCtx,
    group: NodeId,
    change: ConditionChildChange,
) {
    let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
        return;
    };
    let snapshot = snapshot_arc.as_ref();
    let visible = condition_item_count_after_change(snapshot, group, change) >= 2;
    let Some(operator_id) = snapshot.find_child_by_decl_id(group, "operator") else {
        return;
    };
    let Some(operator) = snapshot.node(operator_id) else {
        return;
    };
    if operator.presentation.show_in_inspector_content == visible {
        return;
    }

    let mut presentation = operator.presentation.clone();
    presentation.show_in_inspector_content = visible;
    ctx.patch_node_meta(
        operator_id,
        NodeMetaPatch {
            presentation: Some(presentation),
            ..NodeMetaPatch::default()
        },
    );
}

#[derive(Clone, Copy)]
enum ConditionChildChange {
    None,
    Added(NodeId),
    Removed(NodeId),
}

fn condition_item_count_after_change(
    snapshot: &ProcessTreeSnapshot,
    group: NodeId,
    change: ConditionChildChange,
) -> usize {
    let child_ids = snapshot.child_ids(group);
    let count = child_ids
        .iter()
        .filter(|child| condition_item_in_snapshot(snapshot, **child))
        .count()
        as isize;
    let adjusted = match change {
        ConditionChildChange::None => count,
        ConditionChildChange::Added(child) if !child_ids.contains(&child) => count + 1,
        ConditionChildChange::Removed(child)
            if child_ids.contains(&child) && condition_item_in_snapshot(snapshot, child) =>
        {
            count - 1
        }
        _ => count,
    };
    adjusted.max(0) as usize
}

fn condition_item_in_snapshot(snapshot: &ProcessTreeSnapshot, child: NodeId) -> bool {
    snapshot
        .node(child)
        .is_some_and(|node| node.param_value.is_none())
}

#[cfg(test)]
mod conditions_tests;
