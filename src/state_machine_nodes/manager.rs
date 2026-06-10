use golden_core::{
    node,
    node::{Node, NodeUserPermissions, UserContainerRules, UserCreatableItem},
    process_ctx::ProcessCtx,
};

pub(crate) const STATE_ITEM_KIND: &str = "state";

#[node("state_machine_manager", label = "State Machine")]
pub struct StateMachineManager {}

#[node("state_machine_manager", from_struct)]
impl Node for StateMachineManager {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[STATE_ITEM_KIND]))
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        item_kind == STATE_ITEM_KIND && crate::app::declared_user_item_type_matches(item_type, STATE_ITEM_KIND)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        crate::app::declared_user_creatable_items(STATE_ITEM_KIND)
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        crate::app::create_declared_user_item(node_type, STATE_ITEM_KIND)
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        let mut permissions = NodeUserPermissions::all();
        permissions.can_remove_and_duplicate = false;
        self.node_data_mut().meta.user_permissions = permissions;
    }

    fn on_child_added(&mut self, ctx: &mut ProcessCtx, _parent: golden_core::node::NodeId, _child: golden_core::node::NodeId) {
        crate::app::state_machine_nodes_transition::reconcile_state_networks(ctx, None, None, None);
    }

    fn on_child_removed(
        &mut self,
        ctx: &mut ProcessCtx,
        _parent: golden_core::node::NodeId,
        _child: golden_core::node::NodeId,
    ) {
        crate::app::state_machine_nodes_transition::reconcile_state_networks(ctx, None, None, None);
    }
}

#[cfg(test)]
mod manager_tests;
