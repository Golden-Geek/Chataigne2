use golden_core::{
    item, node,
    node::{Node, NodeReference, NodeUserPermissions},
    process_ctx::ProcessCtx,
};

/// Consequence that sends a command when triggered.
#[node("sm_send_command_consequence", label = "Send Command")]
#[children(
    target: NodeReference (
        label = "Target Command"
    );
)]
pub struct SendCommandConsequence {}

#[item("sm_consequence", node = "sm_send_command_consequence", from_struct)]
impl Node for SendCommandConsequence {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

/// Consequence that sets a parameter value when triggered.
#[node("sm_set_value_consequence", label = "Set Value")]
#[children(
    target: NodeReference (
        label = "Target Parameter",
        reference_target_kind = golden_core::parameter::ReferenceTargetKind::ParameterOnly
    );
    value: f64 = 0.0 (
        label = "Value"
    );
)]
pub struct SetValueConsequence {}

#[item("sm_consequence", node = "sm_set_value_consequence", from_struct)]
impl Node for SetValueConsequence {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}
