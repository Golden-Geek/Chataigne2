use golden_core::{
    item, node,
    node::{Node, NodeReference, NodeUserPermissions},
    process_ctx::ProcessCtx,
};

/// Condition that compares a numeric parameter to a threshold.
#[node("sm_number_compare_condition", label = "Number Compare")]
#[children(
    source: NodeReference (
        label = "Source",
        reference_target_kind = golden_core::parameter::ReferenceTargetKind::ParameterOnly
    );
    comparator: golden_core::parameter::Enum = "greater_than" (
        label = "Comparator",
        enum_options = [
            "greater_than",
            "less_than",
            "greater_than_or_equal",
            "less_than_or_equal",
            "equal",
            "not_equal"
        ]
    );
    threshold: f64 = 0.0 (
        label = "Threshold"
    );
)]
pub struct NumberCompareCondition {}

#[item("sm_condition", node = "sm_number_compare_condition", from_struct)]
impl Node for NumberCompareCondition {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

/// Condition that checks a boolean parameter against an expected value.
#[node("sm_bool_condition", label = "Boolean")]
#[children(
    source: NodeReference (
        label = "Source",
        reference_target_kind = golden_core::parameter::ReferenceTargetKind::ParameterOnly
    );
    expected: bool = true (
        label = "Expected Value"
    );
)]
pub struct BoolCondition {}

#[item("sm_condition", node = "sm_bool_condition", from_struct)]
impl Node for BoolCondition {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}
