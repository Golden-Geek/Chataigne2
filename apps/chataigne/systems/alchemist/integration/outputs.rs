use golden_core::{
    item, node,
    node::{Node, NodeReference, NodeUserPermissions},
    process_ctx::ProcessCtx,
};

/// Output that sends a command.
#[node("sm_send_command_output", label = "Send Command")]
#[children(
    target: NodeReference (
        label = "Target Command"
    );
)]
pub struct SendCommandOutput {}

#[item("sm_output", node = "sm_send_command_output", from_struct)]
impl Node for SendCommandOutput {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}
