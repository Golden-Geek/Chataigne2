use golden_core::{
    color::Color,
    item, node,
    node::{
        Node, NodeId, NodeMetaPatch, NodeUserPermissions, UserContainerRules, UserContextNode,
        UserCreatableItem, USER_CONTEXT_DEFAULT_LABEL, USER_CONTEXT_ITEM_KIND,
        USER_CONTEXT_NODE_TYPE,
    },
    process_ctx::ProcessCtx,
};

#[node("state", label = "State")]
#[children(
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

#[item(
    "state",
    node = "state",
    from_struct,
    contextualizable = golden_core::node::UserContextHostPolicy::multiplex_contextualizable()
)]
impl Node for StateMachineState {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[USER_CONTEXT_ITEM_KIND]))
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        item_kind == USER_CONTEXT_ITEM_KIND && item_type == USER_CONTEXT_NODE_TYPE
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![
            UserCreatableItem::new(
                USER_CONTEXT_NODE_TYPE,
                USER_CONTEXT_ITEM_KIND,
                USER_CONTEXT_DEFAULT_LABEL,
            )
            .with_select_when_created(false),
        ]
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        (node_type == USER_CONTEXT_NODE_TYPE).then(|| {
            Box::new(UserContextNode::new_with_multiplex(
                USER_CONTEXT_DEFAULT_LABEL,
                true,
            )) as Box<dyn Node>
        })
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        let meta = &mut self.node_data_mut().meta;
        meta.user_permissions = NodeUserPermissions::all();
        meta.can_be_disabled = true;
        if meta.presentation.default_color.is_none() {
            meta.presentation.default_color = Some(Color::new(0.28, 0.56, 0.92, 1.0));
        }
    }

    fn on_meta_changed(&mut self, ctx: &mut ProcessCtx, node: NodeId, patch: NodeMetaPatch) {
        if node != self.id() {
            return;
        }
        let Some(enabled) = patch.enabled else {
            return;
        };
        crate::app::systems_state_machine_transition::reconcile_state_networks(
            ctx,
            enabled.then_some(self.id()),
            (!enabled).then_some(self.id()),
            None,
        );
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[cfg(test)]
mod tests;
