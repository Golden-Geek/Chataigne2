use golden_core::{
    item, node,
    node::{DeclId, Node, NodeId, NodeUserPermissions, UserContainerRules, UserCreatableItem},
    process_ctx::ProcessCtx,
};

pub(crate) const CUSTOM_FORMULA_ITEM_KIND: &str = "alchemist_formula_custom";

/// ANode slot that instantiates a ConditionManager under the processor.
#[node("sm_anode_conditions", label = "Conditions Slot")]
pub struct ConditionsManagerANode {}

#[node("sm_anode_conditions", from_struct)]
impl Node for ConditionsManagerANode {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::none();
        self.node_data_mut().meta.can_be_disabled = false;
    }
}

/// ANode slot that instantiates a ConsequencesManager under the processor.
#[node("sm_anode_consequences", label = "Consequences Slot")]
pub struct ConsequencesANode {}

#[node("sm_anode_consequences", from_struct)]
impl Node for ConsequencesANode {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::none();
        self.node_data_mut().meta.can_be_disabled = false;
    }
}

/// ANode slot that instantiates an InputsManager under the processor.
#[node("sm_anode_inputs", label = "Inputs Slot")]
pub struct InputsANode {}

#[node("sm_anode_inputs", from_struct)]
impl Node for InputsANode {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::none();
        self.node_data_mut().meta.can_be_disabled = false;
    }
}

/// ANode slot that instantiates a FilterChainManager under the processor.
#[node("sm_anode_filter_chain", label = "Filter Chain Slot")]
pub struct FilterChainANode {}

#[node("sm_anode_filter_chain", from_struct)]
impl Node for FilterChainANode {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::none();
        self.node_data_mut().meta.can_be_disabled = false;
    }
}

/// ANode slot that instantiates an OutputsManager under the processor.
#[node("sm_anode_outputs", label = "Outputs Slot")]
pub struct OutputsANode {}

#[node("sm_anode_outputs", from_struct)]
impl Node for OutputsANode {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::none();
        self.node_data_mut().meta.can_be_disabled = false;
    }
}

/// Dispatches ANode instantiation: for each ANode child in a formula, creates the
/// corresponding manager node under the processor. The processor never names these
/// types directly — the formula drives what gets created.
pub(crate) fn instantiate_anode_for_processor(
    anode_type: &str,
    anode_label: &str,
    anode_decl_id: &str,
    processor_id: NodeId,
    ctx: &mut ProcessCtx,
) {
    fn set_meta(node: &mut dyn Node, label: &str, decl_id: &str) {
        node.node_data_mut().meta.label = label.to_string();
        node.node_data_mut().meta.decl_id = DeclId(decl_id.to_string());
    }

    let child: Option<Box<dyn Node>> = match anode_type {
        ConditionsManagerANode::NODE_TYPE => {
            let mut n = crate::app::ConditionManager::new();
            set_meta(&mut n, anode_label, anode_decl_id);
            Some(Box::new(n))
        }
        ConsequencesANode::NODE_TYPE => {
            let mut n = crate::app::ConsequencesManager::new();
            set_meta(&mut n, anode_label, anode_decl_id);
            Some(Box::new(n))
        }
        InputsANode::NODE_TYPE => {
            let mut n = crate::app::InputsManager::new();
            set_meta(&mut n, anode_label, anode_decl_id);
            Some(Box::new(n))
        }
        FilterChainANode::NODE_TYPE => {
            let mut n = crate::app::FilterChainManager::new();
            set_meta(&mut n, anode_label, anode_decl_id);
            Some(Box::new(n))
        }
        OutputsANode::NODE_TYPE => {
            let mut n = crate::app::OutputsManager::new();
            set_meta(&mut n, anode_label, anode_decl_id);
            Some(Box::new(n))
        }
        _ => None,
    };

    if let Some(node) = child {
        ctx.add_child_boxed(processor_id, node, None);
    }
}

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

/// Built-in Action formula. Defines Conditions + True/False Consequences slots.
#[node("alchemist_formula_action", label = "Action")]
#[children(
    node conditions: ConditionsManagerANode = ConditionsManagerANode::new() (
        label = "Conditions"
    );
    node true_consequences: ConsequencesANode = ConsequencesANode::new() (
        label = "True Consequences"
    );
    node false_consequences: ConsequencesANode = ConsequencesANode::new() (
        label = "False Consequences"
    );
)]
pub struct ActionBuiltinFormula {}

#[node("alchemist_formula_action", from_struct)]
impl Node for ActionBuiltinFormula {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::none();
        self.node_data_mut().meta.can_be_disabled = false;
    }
}

/// Built-in Mapping formula. Defines Inputs + Filter Chain + Outputs slots.
#[node("alchemist_formula_mapping", label = "Mapping")]
#[children(
    node inputs: InputsANode = InputsANode::new() (
        label = "Inputs"
    );
    node filter_chain: FilterChainANode = FilterChainANode::new() (
        label = "Filter Chain"
    );
    node outputs: OutputsANode = OutputsANode::new() (
        label = "Outputs"
    );
)]
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
mod formula_tests;
