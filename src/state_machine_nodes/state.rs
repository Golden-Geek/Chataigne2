use golden_core::{
    color::Color,
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
    position: golden_core::parameter::Vec2 = (0.0, 0.0) (
        label = "Canvas Position",
        description = "Position in the State Machine canvas.",
        show_in_inspector_content = false
    );
    size: golden_core::parameter::Vec2 = (13.0, 8.0) (
        label = "Canvas Size",
        description = "Custom size in the State Machine canvas. Disable to use automatic sizing.",
        enabled = false,
        can_be_disabled = true,
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
        crate::app::state_machine_nodes_transition::reconcile_state_networks(
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
        let meta = &mut self.node_data_mut().meta;
        meta.user_permissions = NodeUserPermissions::all();
        if meta.presentation.color.is_none() {
            meta.presentation.color = Some(Color::new(0.28, 0.56, 0.92, 1.0));
        }
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[cfg(test)]
mod state_tests;
