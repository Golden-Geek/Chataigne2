use golden_core::{
    events::{Event, EventKind},
    item, node,
    node::{
        Node, NodeCreationContext, NodeId, NodeReference, NodeUserPermissions, UserContainerRules,
        UserCreatableItem,
    },
    parameter::{ParamValue, ReferenceTargetKind},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

const INPUT_ITEM_KIND: &str = "sm_input";
const OUTPUT_ITEM_KIND: &str = "sm_output";
const GENERIC_OUTPUT_MENU_PATH: &str = "Generic";

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
        show_in_inspector_content = false,
        enum_options = ["all", "any", "none", "at_least", "exactly"]
    );
    operator_count: f64 = 1.0 (
        label = "Count",
        show_in_inspector_content = false
    );
    valid: bool = false (
        label = "Valid",
        read_only = true,
        show_in_inspector_content = false
    );
)]
pub struct ConditionManager {}

#[node("sm_condition_manager", from_struct)]
impl Node for ConditionManager {
    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        crate::app::state_machine_nodes_conditions::sync_condition_operator_visibility(ctx, self.id());
    }

    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        match event.kind {
            EventKind::ChildAdded { .. } | EventKind::ChildRemoved { .. } => u32::MAX,
            _ => 0,
        }
    }

    fn on_child_added(&mut self, ctx: &mut ProcessCtx, parent: NodeId, _child: NodeId) {
        if parent == self.id() {
            crate::app::state_machine_nodes_conditions::sync_condition_operator_visibility_after_child_added(
                ctx,
                self.id(),
                _child,
            );
        }
    }

    fn on_child_removed(&mut self, ctx: &mut ProcessCtx, parent: NodeId, _child: NodeId) {
        if parent == self.id() {
            crate::app::state_machine_nodes_conditions::sync_condition_operator_visibility_after_child_removed(
                ctx,
                self.id(),
                _child,
            );
        }
    }

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

fn output_generic_items() -> Vec<UserCreatableItem> {
    crate::app::declared_user_creatable_items(OUTPUT_ITEM_KIND)
        .into_iter()
        .map(|item| {
            item.with_menu_path([GENERIC_OUTPUT_MENU_PATH])
                .with_select_when_created(false)
        })
        .collect()
}

fn collect_module_roots(snapshot: &ProcessTreeSnapshot) -> Vec<NodeId> {
    let mut modules = Vec::new();
    let mut stack = vec![snapshot.root()];
    while let Some(node_id) = stack.pop() {
        let Some(node) = snapshot.node(node_id) else {
            continue;
        };
        if crate::app::declared_user_item_type_matches(
            &node.node_type,
            crate::app::module::MODULE_ITEM_KIND,
        ) {
            modules.push(node_id);
        }
        let mut children = snapshot.child_ids(node_id);
        children.reverse();
        stack.extend(children);
    }
    modules
}

fn output_module_command_items(
    snapshot: &ProcessTreeSnapshot,
    child_catalog: &dyn Fn(NodeId) -> Vec<UserCreatableItem>,
) -> Vec<UserCreatableItem> {
    collect_module_roots(snapshot)
        .into_iter()
        .flat_map(|module_id| {
            let Some(module) = snapshot.node(module_id) else {
                return Vec::new();
            };
            let Some(command_tester) = snapshot.find_child_by_decl_id(module_id, "command_tester")
            else {
                return Vec::new();
            };
            child_catalog(command_tester)
                .into_iter()
                .filter(|item| item.item_kind == crate::app::module_command::MODULE_COMMAND_ITEM_KIND)
                .map(|item| {
                    let mut menu_path = Vec::with_capacity(item.menu_path.len() + 1);
                    menu_path.push(module.label.clone());
                    menu_path.extend(item.menu_path.iter().cloned());
                    item.with_menu_path(menu_path).with_initial_param(
                        crate::app::module_command::MODULE_COMMAND_TARGET_MODULE_PATH,
                        ParamValue::Reference(NodeReference::new(module.uuid)),
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

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
        output_generic_items()
    }

    fn user_creatable_items_with_context(
        &self,
        snapshot: &ProcessTreeSnapshot,
        _parent: NodeId,
        child_catalog: &dyn Fn(NodeId) -> Vec<UserCreatableItem>,
    ) -> Vec<UserCreatableItem> {
        output_module_command_items(snapshot, child_catalog)
            .into_iter()
            .chain(output_generic_items())
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
