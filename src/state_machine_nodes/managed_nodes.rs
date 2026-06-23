use golden_core::{
    item, node,
    node::{Node, NodeReference, NodeUserPermissions, UserContainerRules, UserCreatableItem},
    parameter::ReferenceTargetKind,
    process_ctx::ProcessCtx,
};

const INPUT_ITEM_KIND: &str = "sm_input";
const OUTPUT_ITEM_KIND: &str = "sm_output";

fn locked_manager_permissions() -> NodeUserPermissions {
    let mut p = NodeUserPermissions::all();
    p.can_remove_and_duplicate = false;
    p.can_edit_name = false;
    p
}

#[node("sm_condition_manager", label = "Conditions")]
#[children(
    operator: golden_core::parameter::Enum = "all" (
        label = "Operator",
        enum_options = ["all", "any", "none", "at_least", "exactly"]
    );
    operator_count: f64 = 1.0 (
        label = "Count",
        show_in_inspector_content = false
    );
    empty_policy: golden_core::parameter::Enum = "invalid" (
        label = "Empty Policy",
        enum_options = ["invalid", "valid"]
    );
    disabled_policy: golden_core::parameter::Enum = "ignore" (
        label = "Disabled Child Policy",
        enum_options = ["ignore", "treat_as_invalid", "treat_as_valid"]
    );
    error_policy: golden_core::parameter::Enum = "treat_as_invalid" (
        label = "Error Policy",
        enum_options = ["treat_as_invalid", "block_and_report"]
    );
)]
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
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[INPUT_ITEM_KIND]))
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        item_kind == INPUT_ITEM_KIND
            && crate::app::declared_user_item_type_matches(item_type, INPUT_ITEM_KIND)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        crate::app::declared_user_creatable_items(INPUT_ITEM_KIND)
            .into_iter()
            .map(|item| item.with_select_when_created(false))
            .collect()
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        crate::app::create_declared_user_item(node_type, INPUT_ITEM_KIND)
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = locked_manager_permissions();
        self.node_data_mut().meta.can_be_disabled = false;
    }
}

#[node("sm_input_source", label = "Input Source")]
#[children(
    source: NodeReference (
        label = "Source",
        reference_target_kind = ReferenceTargetKind::ParameterOnly
    );
)]
pub struct InputSource {}

#[item("sm_input", node = "sm_input_source", from_struct)]
impl Node for InputSource {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
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
        Some(UserContainerRules::new(&[
            OUTPUT_ITEM_KIND,
            crate::app::module_command::MODULE_COMMAND_ITEM_KIND,
        ]))
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        matches!(item_kind, OUTPUT_ITEM_KIND)
            && crate::app::declared_user_item_type_matches(item_type, OUTPUT_ITEM_KIND)
            || item_kind == crate::app::module_command::MODULE_COMMAND_ITEM_KIND
                && crate::app::declared_user_item_type_matches(
                    item_type,
                    crate::app::module_command::MODULE_COMMAND_ITEM_KIND,
                )
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        crate::app::declared_user_creatable_items(OUTPUT_ITEM_KIND)
            .into_iter()
            .map(|item| item.with_select_when_created(false))
            .chain(
                crate::app::declared_user_creatable_items(
                    crate::app::module_command::MODULE_COMMAND_ITEM_KIND,
                )
                .into_iter()
                .map(|item| item.with_select_when_created(false)),
            )
            .collect()
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        crate::app::create_declared_user_item(node_type, OUTPUT_ITEM_KIND).or_else(|| {
            crate::app::create_declared_user_item(
                node_type,
                crate::app::module_command::MODULE_COMMAND_ITEM_KIND,
            )
        })
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = locked_manager_permissions();
        self.node_data_mut().meta.can_be_disabled = false;
    }
}

#[cfg(test)]
mod managed_nodes_tests;
