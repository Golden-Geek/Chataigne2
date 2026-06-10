use golden_core::{
    item, node,
    node::{Node, NodeReference, NodeUserPermissions, UserContainerRules, UserCreatableItem},
    process_ctx::ProcessCtx,
};

// ---------------------------------------------------------------------------
// InputValueCondition
// ---------------------------------------------------------------------------

/// Reads any parameter source, projects the value if needed, and compares it
/// against a reference. Projection and comparator options adapt at the UI layer
/// based on the selected source's value type.
#[node("sm_input_value_condition", label = "Input Value")]
#[children(
    invert: bool = false (
        label = "Invert"
    );
    validation_delay_ms: f64 = 0.0 (
        label = "Validation Delay (ms)"
    );
    invalidation_delay_ms: f64 = 0.0 (
        label = "Invalidation Delay (ms)"
    );
    output_mode: golden_core::parameter::Enum = "normal" (
        label = "Output Mode",
        enum_options = [
            "normal",
            "toggle_on_valid",
            "toggle_on_invalid",
            "pulse_on_valid",
            "pulse_on_invalid"
        ]
    );
    pulse_duration_ms: f64 = 200.0 (
        label = "Pulse Duration (ms)",
        show_in_inspector_content = false
    );
    source: NodeReference (
        label = "Source",
        reference_target_kind = golden_core::parameter::ReferenceTargetKind::ParameterOnly
    );
    projection: golden_core::parameter::Enum = "auto" (
        label = "Projection",
        enum_options = [
            "auto",
            "number",
            "bool",
            "vec2_magnitude",
            "vec2_x",
            "vec2_y",
            "vec3_magnitude",
            "vec3_x",
            "vec3_y",
            "vec3_z",
            "color_luminance",
            "color_red",
            "color_green",
            "color_blue",
            "color_alpha",
            "enum_value",
            "string_value"
        ]
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
            "ends_with"
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
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

// ---------------------------------------------------------------------------
// InputNodeCondition
// ---------------------------------------------------------------------------

/// Asks a target node whether a named condition endpoint holds.
///
/// Bool endpoints evaluate directly. Value endpoints pipe their output through
/// the same comparator/reference mechanism as InputValueCondition.
/// Available endpoints are discovered from the target node's condition provider
/// registry at runtime; `endpoint_id` stores the selected endpoint key.
#[node("sm_input_node_condition", label = "Input Node")]
#[children(
    invert: bool = false (
        label = "Invert"
    );
    validation_delay_ms: f64 = 0.0 (
        label = "Validation Delay (ms)"
    );
    invalidation_delay_ms: f64 = 0.0 (
        label = "Invalidation Delay (ms)"
    );
    output_mode: golden_core::parameter::Enum = "normal" (
        label = "Output Mode",
        enum_options = [
            "normal",
            "toggle_on_valid",
            "toggle_on_invalid",
            "pulse_on_valid",
            "pulse_on_invalid"
        ]
    );
    pulse_duration_ms: f64 = 200.0 (
        label = "Pulse Duration (ms)",
        show_in_inspector_content = false
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
            "ends_with"
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
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

// ---------------------------------------------------------------------------
// ScriptCondition
// ---------------------------------------------------------------------------

/// Condition evaluated by a user-authored script. The script reads context and
/// parameter values and sets the condition valid or invalid. No direct side
/// effects are allowed from within a script condition.
#[node("sm_script_condition", label = "Script")]
#[children(
    invert: bool = false (
        label = "Invert"
    );
    validation_delay_ms: f64 = 0.0 (
        label = "Validation Delay (ms)"
    );
    invalidation_delay_ms: f64 = 0.0 (
        label = "Invalidation Delay (ms)"
    );
    output_mode: golden_core::parameter::Enum = "normal" (
        label = "Output Mode",
        enum_options = [
            "normal",
            "toggle_on_valid",
            "toggle_on_invalid",
            "pulse_on_valid",
            "pulse_on_invalid"
        ]
    );
    pulse_duration_ms: f64 = 200.0 (
        label = "Pulse Duration (ms)",
        show_in_inspector_content = false
    );
    script: String = String::new() (
        label = "Script",
        show_in_inspector_content = false
    );
)]
pub struct ScriptCondition {}

#[item("sm_condition", node = "sm_script_condition", from_struct)]
impl Node for ScriptCondition {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

// ---------------------------------------------------------------------------
// ConditionGroup
// ---------------------------------------------------------------------------

/// Combines child conditions with a reduce operator (all/any/none).
/// Acts as a single condition to its parent container while itself accepting
/// nested `sm_condition` items — including other groups — making the tree
/// fully recursive.
#[node("sm_condition_group", label = "Condition Group")]
#[children(
    invert: bool = false (
        label = "Invert"
    );
    operator: golden_core::parameter::Enum = "all" (
        label = "Operator",
        enum_options = ["all", "any", "none"]
    );
    validation_delay_ms: f64 = 0.0 (
        label = "Validation Delay (ms)"
    );
    invalidation_delay_ms: f64 = 0.0 (
        label = "Invalidation Delay (ms)"
    );
    output_mode: golden_core::parameter::Enum = "normal" (
        label = "Output Mode",
        enum_options = [
            "normal",
            "toggle_on_valid",
            "toggle_on_invalid",
            "pulse_on_valid",
            "pulse_on_invalid"
        ]
    );
    pulse_duration_ms: f64 = 200.0 (
        label = "Pulse Duration (ms)",
        show_in_inspector_content = false
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

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[cfg(test)]
mod conditions_tests;
