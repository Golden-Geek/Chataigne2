use golden_core::{
    item, node,
    node::{Node, NodeReference, NodeUserPermissions, UserContainerRules, UserCreatableItem},
    process_ctx::ProcessCtx,
};

/// Reads any parameter source, projects the value if needed, and compares it against
/// a reference. Comparator options and the reference widget adapt to the projected
/// value type at the UI layer; the authored data model stores the full field set.
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

/// Condition group: combines child conditions with a reduce operator (all/any/none).
/// Acts as a single condition to its parent container while itself accepting
/// nested `sm_condition` items — including other groups — making the condition
/// tree fully recursive.
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
mod state_machine_conditions_tests;
