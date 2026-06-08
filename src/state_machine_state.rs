use golden_core::{item, node, node::Node, process_ctx::ProcessCtx};

#[node("state", label = "State")]
#[children(
    x: f64 = 0.0 (
        label = "Canvas X",
        description = "Horizontal position in the State Machine canvas.",
        show_in_inspector_content = false
    );
    y: f64 = 0.0 (
        label = "Canvas Y",
        description = "Vertical position in the State Machine canvas.",
        show_in_inspector_content = false
    );
    width: f64 = 13.0 (
        label = "Canvas Width",
        description = "Width in the State Machine canvas.",
        show_in_inspector_content = false
    );
    height: f64 = 8.0 (
        label = "Canvas Height",
        description = "Height in the State Machine canvas.",
        show_in_inspector_content = false
    );
)]
pub struct StateMachineState {}

#[item("state", node = "state", from_struct)]
impl Node for StateMachineState {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = golden_core::node::NodeUserPermissions::all();
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}
