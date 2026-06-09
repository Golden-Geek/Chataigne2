use golden_core::{
    item, node,
    node::{Node, NodeUserPermissions},
    parameter::ParamValue,
    process_ctx::ProcessCtx,
};

#[node("state", label = "State")]
#[children(
    active: bool = true (
        label = "Active",
        description = "Whether this State is active in its connected State Network.",
        callback = Self::active_changed
    );
    description: String = String::new() (
        label = "Description",
        description = "User-authored description for this State."
    );
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
    node processors: crate::app::StateProcessorManager = crate::app::StateProcessorManager::new() (
        label = "Processors",
        description = "Processors evaluated while this State is active.",
        collapsed = false
    );
    node transitions: crate::app::StateTransitionManager = crate::app::StateTransitionManager::new() (
        label = "Transitions",
        description = "Outgoing transitions from this State.",
        collapsed = true,
        show_in_nested_inspector = false
    );
)]
pub struct StateMachineState {}

impl StateMachineState {
    fn active_changed(&mut self, ctx: &mut ProcessCtx, _old_value: ParamValue) {
        let preferred_active = self.active.get().then_some(self.id());
        let forced_inactive = (!self.active.get()).then_some(self.id());
        crate::app::state_machine_transition::reconcile_state_networks(
            ctx,
            preferred_active,
            forced_inactive,
            None,
        );
    }
}

#[item("state", node = "state", from_struct)]
impl Node for StateMachineState {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[cfg(test)]
mod state_machine_state_tests;
