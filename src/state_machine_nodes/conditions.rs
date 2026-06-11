use golden_core::{
    item, node,
    node::{Node, NodeReference, NodeUserPermissions, UserContainerRules, UserCreatableItem},
    process_ctx::ProcessCtx,
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

/// Reads any parameter source and compares its (optionally projected) value
/// against a reference. Projection is set on the `source` NodeReference;
/// comparator options adapt to the resolved source type at runtime via the
/// `valid_comparators` script property emitted by StateProcessor.
#[node("sm_input_value_condition", label = "Input Value")]
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
    source: NodeReference (
        label = "Source",
        reference_target_kind = golden_core::parameter::ReferenceTargetKind::ParameterOnly
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
pub struct InputValueCondition {}

#[item("sm_condition", node = "sm_input_value_condition", from_struct)]
impl Node for InputValueCondition {
    leaf_condition_init!();
}

// ─── InputNodeCondition ──────────────────────────────────────────────────────

/// Asks a target node whether a named condition endpoint holds.
/// Available endpoints are discovered from the target node's condition
/// provider registry; `endpoint_id` stores the selected key.
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

/// Condition evaluated by a user-authored script. The script reads context
/// and parameter values and sets the condition valid or invalid.
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

/// Combines child conditions with a reduce operator. Acts as a single condition
/// to its parent while itself accepting nested `sm_condition` items —
/// enabling fully recursive condition trees.
#[node("sm_condition_group", label = "Condition Group")]
#[children(
    toggle_mode: bool = false (
        label = "Toggle Mode"
    );
    operator: golden_core::parameter::Enum = "all" (
        label = "Operator",
        enum_options = ["all", "any", "none", "at_least", "exactly"]
    );
    operator_count: f64 = 1.0 (
        label = "Count",
        show_in_inspector_content = false
    );
    empty_policy: golden_core::parameter::Enum = "invalid" (
        label = "Empty Policy",
        enum_options = ["invalid", "valid"]
    );
    disabled_policy: golden_core::parameter::Enum = "ignore" (
        label = "Disabled Child Policy",
        enum_options = ["ignore", "treat_as_invalid", "treat_as_valid"]
    );
    error_policy: golden_core::parameter::Enum = "treat_as_invalid" (
        label = "Error Policy",
        enum_options = ["treat_as_invalid", "block_and_report"]
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

#[cfg(test)]
mod conditions_tests;
