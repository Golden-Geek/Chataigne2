use golden_core::{
    edit::{Edit, NodeTree},
    events::Event,
    node,
    node::{
        DeclId, Node, NodeCreationContext, NodeId, NodeMetaPatch,
        NodeReference, NodeUuid, NodeUserPermissions, UserContainerRules,
        UserCreatableItem,
    },
    parameter::{
        ParamValue, Parameter, ParameterChangeCheck, ReferenceTargetKind,
    },
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::state_machine_nodes_formula::{
    PROPERTIES_DECL_ID, PROPERTY_MANAGER_NODE_TYPE, PROPERTY_NODE_TYPE,
};
use crate::app::{
    ConditionManager, ConsequencesManager, FilterChainManager, InputsManager,
    OutputsManager,
};

const FORMULA_LIBRARY_NODE_TYPE: &str = "alchemist_formula_library";
const FORMULA_NODE_TYPE: &str = "alchemist_formula";
const PROCESSOR_SURFACE_DECL_PREFIX: &str = "surface/";
pub(crate) const PROCESSOR_ITEM_KIND: &str = "state_processor";
pub(crate) const PROCESSOR_FOLDER_ITEM_KIND: &str = "state_processor_folder";
pub(crate) const PROCESSOR_FOLDER_NODE_TYPE: &str = "state_processor_folder";

fn processor_container_rules() -> UserContainerRules {
    UserContainerRules::new(&[PROCESSOR_ITEM_KIND, PROCESSOR_FOLDER_ITEM_KIND])
}

fn processor_container_accepts(item_type: &str, item_kind: &str) -> bool {
    match item_kind {
        PROCESSOR_ITEM_KIND => {
            item_type == StateProcessor::NODE_TYPE || item_type.starts_with("state_processor:")
        }
        PROCESSOR_FOLDER_ITEM_KIND => item_type == PROCESSOR_FOLDER_NODE_TYPE,
        _ => false,
    }
}

fn initialize_processor_item(node: &mut dyn Node) {
    node.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
}

fn build_formula_items(snapshot: &ProcessTreeSnapshot, library: NodeId) -> Vec<UserCreatableItem> {
    snapshot
        .child_ids(library)
        .into_iter()
        .filter_map(|formula_id| {
            let formula = snapshot.node(formula_id)?;
            (formula.node_type == FORMULA_NODE_TYPE).then(|| {
                UserCreatableItem::new(
                    format!("state_processor:{}", formula.uuid.0),
                    PROCESSOR_ITEM_KIND,
                    &formula.label,
                )
            })
        })
        .collect()
}

fn find_formula_library(snapshot: &ProcessTreeSnapshot) -> Option<NodeId> {
    snapshot
        .child_ids(snapshot.root())
        .into_iter()
        .find(|node| {
            snapshot
                .node(*node)
                .is_some_and(|snapshot_node| snapshot_node.node_type == FORMULA_LIBRARY_NODE_TYPE)
        })
}

fn locked_instance_permissions() -> NodeUserPermissions {
    let mut permissions = NodeUserPermissions::all();
    permissions.can_remove_and_duplicate = false;
    permissions.can_edit_name = false;
    permissions
}

fn processor_surface_decl_id(source_uuid: NodeUuid) -> String {
    format!("{PROCESSOR_SURFACE_DECL_PREFIX}{}", source_uuid.0)
}

fn processor_property_parameter(
    snapshot: &ProcessTreeSnapshot,
    source: NodeId,
) -> Option<Parameter> {
    let source_node = snapshot.node(source)?;
    let value_id = snapshot.find_child_by_decl_id(source, "value")?;
    let value = snapshot.node(value_id)?.param_value.clone()?;
    let mut parameter = Parameter::new(
        &source_node.label,
        value,
        ParameterChangeCheck::ValueChange,
    );
    parameter.node_data_mut().meta.decl_id =
        DeclId(processor_surface_decl_id(source_node.uuid));
    if let Some(constraints) = snapshot
        .node(value_id)
        .and_then(|value| value.param_constraints.clone())
    {
        parameter.constraints = constraints;
    }
    Some(parameter)
}

fn processor_property_manager(
    snapshot: &ProcessTreeSnapshot,
    source: NodeId,
) -> Option<Box<dyn Node>> {
    let source_node = snapshot.node(source)?;
    let role = snapshot
        .find_child_by_decl_id(source, "role")
        .and_then(|role| snapshot.node(role))
        .and_then(|role| role.param_value.as_ref())
        .and_then(ParamValue::as_str)?;
    let mut manager: Box<dyn Node> = match role.as_str() {
        "condition" => Box::new(ConditionManager::new()),
        "consequence" => Box::new(ConsequencesManager::new()),
        "input" => Box::new(InputsManager::new()),
        "filter" => Box::new(FilterChainManager::new()),
        "output" => Box::new(OutputsManager::new()),
        _ => return None,
    };
    manager.node_data_mut().meta.label = source_node.label.clone();
    manager.node_data_mut().meta.decl_id =
        DeclId(processor_surface_decl_id(source_node.uuid));
    Some(manager)
}

fn processor_surface_child_tree(
    snapshot: &ProcessTreeSnapshot,
    source: NodeId,
) -> Option<NodeTree> {
    match snapshot.node(source)?.node_type.as_str() {
        PROPERTY_NODE_TYPE => {
            processor_property_parameter(snapshot, source).map(NodeTree::new)
        }
        PROPERTY_MANAGER_NODE_TYPE => {
            processor_property_manager(snapshot, source).map(NodeTree::boxed)
        }
        _ => None,
    }
}

fn processor_properties_tree(
    snapshot: &ProcessTreeSnapshot,
    formula: NodeId,
) -> NodeTree {
    let mut properties = StateProcessorProperties::new();
    properties.node_data_mut().meta.decl_id =
        DeclId(PROPERTIES_DECL_ID.to_owned());
    let mut tree = NodeTree::new(properties);
    if let Some(source_properties) =
        snapshot.find_child_by_decl_id(formula, PROPERTIES_DECL_ID)
    {
        for source in snapshot.child_ids(source_properties) {
            if let Some(child) =
                processor_surface_child_tree(snapshot, source)
            {
                tree.push_child(child);
            }
        }
    }
    tree
}

#[node("state_processor_properties", label = "Properties")]
pub struct StateProcessorProperties {}

#[node("state_processor_properties", from_struct)]
impl Node for StateProcessorProperties {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions =
            locked_instance_permissions();
        self.node_data_mut().meta.can_be_disabled = false;
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node(
    "state_processor_manager",
    label = "Processors",
    presentation = golden_core::node::PresentationHint {
        show_in_nested_inspector: false,
        ..Default::default()
    }
)]
pub struct StateProcessorManager {
    #[state(default = Vec::new())]
    formula_items: Vec<UserCreatableItem>,
}

#[node("state_processor_manager", from_struct)]
impl Node for StateProcessorManager {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(processor_container_rules())
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        processor_container_accepts(item_type, item_kind)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        let mut items = self.formula_items.clone();
        items.push(UserCreatableItem::new(
            PROCESSOR_FOLDER_NODE_TYPE,
            PROCESSOR_FOLDER_ITEM_KIND,
            "Folder",
        ));
        items
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        if node_type == PROCESSOR_FOLDER_NODE_TYPE {
            return Some(Box::new(StateProcessorFolder::new()));
        }
        create_processor_for_formula_type(node_type)
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        let mut permissions = NodeUserPermissions::all();
        permissions.can_remove_and_duplicate = false;
        self.node_data_mut().meta.user_permissions = permissions;
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        let Some(snapshot) = ctx.tree_snapshot() else {
            return;
        };
        let root = snapshot.root();
        let library = find_formula_library(snapshot);
        ctx.add_event_listener_subtree(self.id(), root, 1);
        if let Some(library) = library {
            ctx.add_event_listener_subtree(self.id(), library, 2);
        }
        self.refresh_formula_items(ctx);
    }

    fn on_node_created(&mut self, ctx: &mut ProcessCtx, node: NodeId) {
        let is_library = ctx.tree_snapshot().is_some_and(|snapshot| {
            snapshot
                .node(node)
                .is_some_and(|snapshot_node| snapshot_node.node_type == FORMULA_LIBRARY_NODE_TYPE)
        });
        if is_library {
            ctx.add_event_listener_subtree(self.id(), node, 2);
        }
        self.refresh_formula_items(ctx);
    }

    fn on_node_deleted(&mut self, ctx: &mut ProcessCtx, _node: NodeId) {
        self.refresh_formula_items(ctx);
    }

    fn on_meta_changed(&mut self, ctx: &mut ProcessCtx, _node: NodeId, _patch: NodeMetaPatch) {
        self.refresh_formula_items(ctx);
    }

    fn on_child_added(&mut self, ctx: &mut ProcessCtx, _parent: NodeId, _child: NodeId) {
        self.refresh_formula_items(ctx);
    }

    fn on_child_removed(&mut self, ctx: &mut ProcessCtx, _parent: NodeId, _child: NodeId) {
        self.refresh_formula_items(ctx);
    }
}

impl StateProcessorManager {
    fn refresh_formula_items(&mut self, ctx: &ProcessCtx) {
        self.formula_items = ctx
            .tree_snapshot()
            .and_then(|snapshot| {
                find_formula_library(snapshot)
                    .map(|library| build_formula_items(snapshot, library))
            })
            .unwrap_or_default();
    }
}

fn create_processor_for_formula_type(node_type: &str) -> Option<Box<dyn Node>> {
    let formula_uuid = node_type
        .strip_prefix("state_processor:")?
        .parse::<uuid::Uuid>()
        .ok()
        .map(NodeUuid)?;
    let mut processor = StateProcessor::new();
    processor.formula.apply_runtime_value(&ParamValue::Reference(
        NodeReference::new(formula_uuid),
    ));
    Some(Box::new(processor))
}

#[node("state_processor_folder", label = "Folder")]
pub struct StateProcessorFolder {}

#[node("state_processor_folder", from_struct)]
impl Node for StateProcessorFolder {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(processor_container_rules())
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        processor_container_accepts(item_type, item_kind)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![UserCreatableItem::new(
            PROCESSOR_FOLDER_NODE_TYPE,
            PROCESSOR_FOLDER_ITEM_KIND,
            "Folder",
        )]
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        if node_type == PROCESSOR_FOLDER_NODE_TYPE {
            return Some(Box::new(StateProcessorFolder::new()));
        }
        create_processor_for_formula_type(node_type)
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        initialize_processor_item(self);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("state_processor", label = "Processor")]
#[children(
    formula: NodeReference (
        label = "Formula",
        reference_target_kind = ReferenceTargetKind::AnyNode,
        reference_allowed_node_types = vec![FORMULA_NODE_TYPE.to_owned()],
        reference_allow_projections = false
    );
)]
pub struct StateProcessor {
    #[state(default = None)]
    subscribed_formula: Option<NodeId>,
}

#[node("state_processor", from_struct)]
impl Node for StateProcessor {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        initialize_processor_item(self);
        self.reconcile_formula_properties(ctx);
    }

    fn on_node_ready(
        &mut self,
        ctx: &mut ProcessCtx,
        _context: NodeCreationContext,
    ) {
        self.refresh_formula_subscription(ctx);
        self.reconcile_formula_properties(ctx);
    }

    fn on_param_change(
        &mut self,
        ctx: &mut ProcessCtx,
        param: NodeId,
        _old_value: ParamValue,
    ) {
        if param == self.formula.id() {
            self.refresh_formula_subscription(ctx);
        }
        self.reconcile_formula_properties(ctx);
    }

    fn on_node_created(&mut self, ctx: &mut ProcessCtx, _node: NodeId) {
        self.reconcile_formula_properties(ctx);
    }

    fn on_node_deleted(&mut self, ctx: &mut ProcessCtx, _node: NodeId) {
        self.refresh_formula_subscription(ctx);
        self.reconcile_formula_properties(ctx);
    }

    fn on_child_added(
        &mut self,
        ctx: &mut ProcessCtx,
        _parent: NodeId,
        _child: NodeId,
    ) {
        self.reconcile_formula_properties(ctx);
    }

    fn on_child_removed(
        &mut self,
        ctx: &mut ProcessCtx,
        _parent: NodeId,
        _child: NodeId,
    ) {
        self.reconcile_formula_properties(ctx);
    }

    fn on_meta_changed(
        &mut self,
        ctx: &mut ProcessCtx,
        _node: NodeId,
        _patch: NodeMetaPatch,
    ) {
        self.reconcile_formula_properties(ctx);
    }

    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        3
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

impl StateProcessor {
    fn formula_node(
        &self,
        snapshot: &ProcessTreeSnapshot,
    ) -> Option<NodeId> {
        let reference = self.formula.get_ref();
        if reference.is_empty() {
            return None;
        }
        snapshot
            .node_id_by_uuid(reference.uuid())
            .filter(|formula| {
                snapshot
                    .node(*formula)
                    .is_some_and(|node| node.node_type == FORMULA_NODE_TYPE)
            })
    }

    fn refresh_formula_subscription(&mut self, ctx: &mut ProcessCtx) {
        let next = ctx
            .tree_snapshot()
            .and_then(|snapshot| self.formula_node(snapshot));
        if self.subscribed_formula == next {
            return;
        }
        if let Some(previous) = self.subscribed_formula {
            ctx.remove_event_listener_subtree(self.id(), previous, 3);
        }
        if let Some(next) = next {
            ctx.add_event_listener_subtree(self.id(), next, 3);
        }
        self.subscribed_formula = next;
    }

    fn reconcile_formula_properties(&self, ctx: &mut ProcessCtx) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        let formula = self.formula_node(&snapshot);
        let existing_properties =
            snapshot.find_child_by_decl_id(self.id(), PROPERTIES_DECL_ID);

        let Some(formula) = formula else {
            if let Some(properties) = existing_properties {
                ctx.edits.push(Edit::RemoveNode { node: properties });
            }
            return;
        };

        let Some(properties) = existing_properties else {
            ctx.add_child_tree(
                self.id(),
                processor_properties_tree(&snapshot, formula),
                None,
            );
            return;
        };
        let Some(source_properties) =
            snapshot.find_child_by_decl_id(formula, PROPERTIES_DECL_ID)
        else {
            for child in snapshot.child_ids(properties) {
                ctx.edits.push(Edit::RemoveNode { node: child });
            }
            return;
        };

        let mut desired = std::collections::HashSet::new();
        for source in snapshot.child_ids(source_properties) {
            let Some(source_node) = snapshot.node(source) else {
                continue;
            };
            let decl_id = processor_surface_decl_id(source_node.uuid);
            let Some(expected_tree) =
                processor_surface_child_tree(&snapshot, source)
            else {
                continue;
            };
            desired.insert(decl_id.clone());
            let Some(existing) =
                snapshot.find_child_by_decl_id(properties, &decl_id)
            else {
                ctx.add_child_tree(properties, expected_tree, None);
                continue;
            };
            let Some(existing_node) = snapshot.node(existing) else {
                continue;
            };
            if existing_node.node_type != expected_tree.node_type() {
                ctx.edits.push(Edit::ReplaceNode {
                    node: existing,
                    new_node: expected_tree.node,
                });
                continue;
            }
            if existing_node.label != source_node.label {
                ctx.patch_node_meta(
                    existing,
                    NodeMetaPatch {
                        label: Some(source_node.label.clone()),
                        ..NodeMetaPatch::default()
                    },
                );
            }
            if source_node.node_type == PROPERTY_NODE_TYPE {
                let Some(source_value) =
                    snapshot.find_child_by_decl_id(source, "value")
                else {
                    continue;
                };
                if let Some(constraints) = snapshot
                    .node(source_value)
                    .and_then(|node| node.param_constraints.clone())
                    .filter(|constraints| {
                        snapshot
                            .node(existing)
                            .and_then(|node| node.param_constraints.as_ref())
                            != Some(constraints)
                    })
                {
                    ctx.edits.push(Edit::SetParamConstraints {
                        node: existing,
                        constraints,
                    });
                }
            }
        }

        for child in snapshot.child_ids(properties) {
            let Some(node) = snapshot.node(child) else {
                continue;
            };
            if node.decl_id.starts_with(PROCESSOR_SURFACE_DECL_PREFIX)
                && !desired.contains(&node.decl_id)
            {
                ctx.edits.push(Edit::RemoveNode { node: child });
            }
        }
    }
}

#[cfg(test)]
mod processor_tests;
