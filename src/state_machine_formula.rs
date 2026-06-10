use golden_core::{
    item, node,
    node::{Node, NodeUserPermissions, UserContainerRules, UserCreatableItem},
    process_ctx::ProcessCtx,
};

pub(crate) const CUSTOM_FORMULA_ITEM_KIND: &str = "alchemist_formula_custom";

/// Project-level container for all Alchemist Formulas.
///
/// Built-in formulas (Action, Mapping) are fixed non-removable children.
/// Users can add custom formulas via the + menu.
#[node("alchemist_formula_library", label = "Formulas")]
#[children(
    node action: ActionBuiltinFormula = ActionBuiltinFormula::new() (
        label = "Action",
        description = "Built-in Action formula. Non-editable graph."
    );
    node mapping: MappingBuiltinFormula = MappingBuiltinFormula::new() (
        label = "Mapping",
        description = "Built-in Mapping formula. Non-editable graph."
    );
)]
pub struct FormulaLibrary {}

#[node("alchemist_formula_library", from_struct)]
impl Node for FormulaLibrary {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[CUSTOM_FORMULA_ITEM_KIND]))
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        item_kind == CUSTOM_FORMULA_ITEM_KIND
            && crate::app::declared_user_item_type_matches(item_type, CUSTOM_FORMULA_ITEM_KIND)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        crate::app::declared_user_creatable_items(CUSTOM_FORMULA_ITEM_KIND)
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        crate::app::create_declared_user_item(node_type, CUSTOM_FORMULA_ITEM_KIND)
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        let mut permissions = NodeUserPermissions::all();
        permissions.can_remove_and_duplicate = false;
        self.node_data_mut().meta.user_permissions = permissions;
    }
}

/// Built-in Action formula node. Non-user-editable.
///
/// The formula graph is defined in the `golden_alchemist` crate and cannot be
/// modified by users. Processors of type Action implicitly use this formula.
#[node("alchemist_formula_action", label = "Action")]
pub struct ActionBuiltinFormula {}

#[node("alchemist_formula_action", from_struct)]
impl Node for ActionBuiltinFormula {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::none();
        self.node_data_mut().meta.can_be_disabled = false;
    }
}

/// Built-in Mapping formula node. Non-user-editable.
///
/// The formula graph is defined in the `golden_alchemist` crate and cannot be
/// modified by users. Processors of type Mapping implicitly use this formula.
#[node("alchemist_formula_mapping", label = "Mapping")]
pub struct MappingBuiltinFormula {}

#[node("alchemist_formula_mapping", from_struct)]
impl Node for MappingBuiltinFormula {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::none();
        self.node_data_mut().meta.can_be_disabled = false;
    }
}

/// User-created custom formula.
///
/// The authored graph is stored as JSON and opened in the Alchemist graph editor.
/// Custom formulas can expose any combination of managed ANodes (ConditionsManager,
/// ConsequencesManager, FilterChain, etc.) to generate processor child nodes.
#[node("alchemist_formula_custom", label = "Custom Formula")]
#[children(
    authored_graph: String = String::new() (
        label = "Authored Graph",
        description = "Serialized Alchemist graph for this custom formula.",
        show_in_inspector_content = false
    );
)]
pub struct CustomAlchemistFormula {}

#[item("alchemist_formula_custom", node = "alchemist_formula_custom", from_struct)]
impl Node for CustomAlchemistFormula {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[cfg(test)]
mod state_machine_formula_tests;
