use golden_core::{
    node,
    node::{Node, NodeUserPermissions, UserContainerRules, UserCreatableItem},
    process_ctx::ProcessCtx,
};

fn locked_manager_permissions() -> NodeUserPermissions {
    let mut p = NodeUserPermissions::all();
    p.can_remove_and_duplicate = false;
    p.can_edit_name = false;
    p
}

#[node("sm_condition_manager", label = "Conditions")]
pub struct ConditionManager {}

#[node("sm_condition_manager", from_struct)]
impl Node for ConditionManager {
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
        self.node_data_mut().meta.user_permissions = locked_manager_permissions();
        self.node_data_mut().meta.can_be_disabled = false;
    }
}

#[node("sm_consequences_manager", label = "Consequences")]
pub struct ConsequencesManager {}

#[node("sm_consequences_manager", from_struct)]
impl Node for ConsequencesManager {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&["sm_consequence"]))
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        item_kind == "sm_consequence"
            && crate::app::declared_user_item_type_matches(item_type, "sm_consequence")
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        crate::app::declared_user_creatable_items("sm_consequence")
            .into_iter()
            .map(|item| item.with_select_when_created(false))
            .collect()
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        crate::app::create_declared_user_item(node_type, "sm_consequence")
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = locked_manager_permissions();
        self.node_data_mut().meta.can_be_disabled = false;
    }
}

#[node("sm_inputs_manager", label = "Inputs")]
pub struct InputsManager {}

#[node("sm_inputs_manager", from_struct)]
impl Node for InputsManager {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = locked_manager_permissions();
        self.node_data_mut().meta.can_be_disabled = false;
    }
}

#[node("sm_filter_chain_manager", label = "Filter Chain")]
pub struct FilterChainManager {}

#[node("sm_filter_chain_manager", from_struct)]
impl Node for FilterChainManager {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&["sm_filter"]))
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        item_kind == "sm_filter"
            && crate::app::declared_user_item_type_matches(item_type, "sm_filter")
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        crate::app::declared_user_creatable_items("sm_filter")
            .into_iter()
            .map(|item| item.with_select_when_created(false))
            .collect()
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        crate::app::create_declared_user_item(node_type, "sm_filter")
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = locked_manager_permissions();
        self.node_data_mut().meta.can_be_disabled = false;
    }
}

#[node("sm_outputs_manager", label = "Outputs")]
pub struct OutputsManager {}

#[node("sm_outputs_manager", from_struct)]
impl Node for OutputsManager {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&["sm_output"]))
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        item_kind == "sm_output"
            && crate::app::declared_user_item_type_matches(item_type, "sm_output")
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        crate::app::declared_user_creatable_items("sm_output")
            .into_iter()
            .map(|item| item.with_select_when_created(false))
            .collect()
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        crate::app::create_declared_user_item(node_type, "sm_output")
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = locked_manager_permissions();
        self.node_data_mut().meta.can_be_disabled = false;
    }
}

#[cfg(test)]
mod state_machine_managed_nodes_tests;
