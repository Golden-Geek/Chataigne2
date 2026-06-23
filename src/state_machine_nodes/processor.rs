use std::collections::HashSet;

use golden_alchemist::{ManagedRegionDefinition, SurfaceItemKind};
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
    anode_container_accepts_for_roles, anode_creatable_items_for_roles, create_anode_user_item,
    formula_from_snapshot, node_has_warning, node_warning_detail, node_warning_matches, ANODE_ITEM_KIND,
    FORMULA_WARNING_ID, PROPERTIES_DECL_ID, PROPERTY_FOLDER_NODE_TYPE,
    PROPERTY_MANAGER_NODE_TYPE, PROPERTY_NODE_TYPE,
};
use crate::app::{ConditionManager, FilterChainManager, InputsManager, OutputsManager};

mod catalog;

pub(crate) use self::catalog::{
    FormulaCatalog, FormulaSourceRef, ProcessorFormulaSourceState,
};

const FORMULA_LIBRARY_NODE_TYPE: &str = "alchemist_formula_library";
const FORMULA_NODE_TYPE: &str = "alchemist_formula";
const PROCESSOR_SURFACE_DECL_PREFIX: &str = "surface/";
const PROCESSOR_FORMULA_WARNING_ID: &str = "state_processor_formula";
pub(crate) const PROCESSOR_FORMULA_SOURCE_DECL_ID: &str = "formula_source_key";
pub(crate) const PROCESSOR_MANAGED_REGIONS_DECL_ID: &str = "managed_regions";
pub(crate) const PROCESSOR_MANAGED_REGION_DECL_PREFIX: &str = "managed_region/";
const PROCESSOR_MANAGED_REGION_ROLE_TAG_PREFIX: &str =
    "state_processor.managed_region.role:";
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
        "filter" => Box::new(FilterChainManager::new()),
        "input" => Box::new(InputsManager::new()),
        "output" => Box::new(OutputsManager::new()),
        _ => return None,
    };
    manager.node_data_mut().meta.label = source_node.label.clone();
    manager.node_data_mut().meta.decl_id =
        DeclId(processor_surface_decl_id(source_node.uuid));
    Some(manager)
}

fn is_property_exposed(snapshot: &ProcessTreeSnapshot, source: NodeId) -> bool {
    snapshot
        .find_child_by_decl_id(source, "exposed")
        .and_then(|n| snapshot.node(n))
        .and_then(|n| n.param_value.as_ref())
        .and_then(ParamValue::as_bool)
        .unwrap_or(true)
}

fn processor_surface_child_tree(
    snapshot: &ProcessTreeSnapshot,
    source: NodeId,
) -> Option<NodeTree> {
    match snapshot.node(source)?.node_type.as_str() {
        PROPERTY_NODE_TYPE => {
            if !is_property_exposed(snapshot, source) {
                return None;
            }
            processor_property_parameter(snapshot, source).map(NodeTree::new)
        }
        PROPERTY_MANAGER_NODE_TYPE => {
            processor_property_manager(snapshot, source).map(NodeTree::boxed)
        }
        PROPERTY_FOLDER_NODE_TYPE => {
            let source_node = snapshot.node(source)?;
            let mut folder = StateProcessorFolder::new();
            folder.node_data_mut().meta.label = source_node.label.clone();
            folder.node_data_mut().meta.decl_id =
                DeclId(processor_surface_decl_id(source_node.uuid));
            folder.node_data_mut().meta.user_permissions = locked_instance_permissions();
            let mut tree = NodeTree::new(folder);
            for child in snapshot.child_ids(source) {
                if let Some(child_tree) = processor_surface_child_tree(snapshot, child) {
                    tree.push_child(child_tree);
                }
            }
            Some(tree)
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

pub(crate) fn processor_managed_region_decl_id(region_id: &str) -> String {
    format!("{PROCESSOR_MANAGED_REGION_DECL_PREFIX}{region_id}")
}

fn surface_item_kind_tag(role: SurfaceItemKind) -> &'static str {
    match role {
        SurfaceItemKind::Parameter => "parameter",
        SurfaceItemKind::Condition => "condition",
        SurfaceItemKind::Consequence => "consequence",
        SurfaceItemKind::Input => "input",
        SurfaceItemKind::Filter => "filter",
        SurfaceItemKind::Output => "output",
        SurfaceItemKind::Command => "command",
    }
}

fn surface_item_kind_from_tag(value: &str) -> Option<SurfaceItemKind> {
    match value {
        "parameter" => Some(SurfaceItemKind::Parameter),
        "condition" => Some(SurfaceItemKind::Condition),
        "consequence" => Some(SurfaceItemKind::Consequence),
        "input" => Some(SurfaceItemKind::Input),
        "filter" => Some(SurfaceItemKind::Filter),
        "output" => Some(SurfaceItemKind::Output),
        "command" => Some(SurfaceItemKind::Command),
        _ => None,
    }
}

fn managed_region_tags(definition: &ManagedRegionDefinition) -> Vec<String> {
    definition
        .accepted_roles
        .iter()
        .map(|role| {
            format!(
                "{PROCESSOR_MANAGED_REGION_ROLE_TAG_PREFIX}{}",
                surface_item_kind_tag(*role)
            )
        })
        .collect()
}

fn managed_region_roles_from_tags(tags: &[String]) -> Vec<SurfaceItemKind> {
    tags.iter()
        .filter_map(|tag| tag.strip_prefix(PROCESSOR_MANAGED_REGION_ROLE_TAG_PREFIX))
        .filter_map(surface_item_kind_from_tag)
        .collect()
}

fn processor_managed_region_tree(definition: &ManagedRegionDefinition) -> NodeTree {
    let mut region = StateProcessorManagedRegion::new();
    let meta = &mut region.node_data_mut().meta;
    meta.label = definition.label.clone();
    meta.decl_id = DeclId(processor_managed_region_decl_id(definition.id.as_str()));
    meta.tags = managed_region_tags(definition);
    NodeTree::new(region)
}

fn processor_surface_move_pending(
    ctx: &ProcessCtx,
    node: NodeId,
    new_parent: NodeId,
    new_prev_sibling: Option<NodeId>,
) -> bool {
    ctx.edits.pending.iter().any(|req| {
        matches!(
            &req.edit,
            Edit::MoveNode {
                node: pending,
                new_parent: pending_parent,
                new_prev_sibling: pending_prev_sibling,
            } if *pending == node
                && *pending_parent == new_parent
                && *pending_prev_sibling == new_prev_sibling
        )
    })
}

fn sync_processor_surface_order(
    snapshot: &ProcessTreeSnapshot,
    dest_container: NodeId,
    desired_children: &[NodeId],
    ctx: &mut ProcessCtx,
) {
    if desired_children.len() < 2 {
        return;
    }

    let desired_set = desired_children.iter().copied().collect::<HashSet<_>>();
    let mut current_children = snapshot
        .child_ids(dest_container)
        .into_iter()
        .filter(|child| desired_set.contains(child))
        .collect::<Vec<_>>();

    if current_children == desired_children {
        return;
    }

    let mut previous = None;
    for desired_child in desired_children {
        let Some(current_index) = current_children
            .iter()
            .position(|child| child == desired_child)
        else {
            continue;
        };
        let target_index = previous
            .and_then(|previous| {
                current_children
                    .iter()
                    .position(|child| *child == previous)
                    .map(|index| index + 1)
            })
            .unwrap_or(0);

        if current_index != target_index {
            let child = current_children.remove(current_index);
            let insert_index = if current_index < target_index {
                target_index - 1
            } else {
                target_index
            };
            current_children.insert(insert_index, child);

            if !processor_surface_move_pending(
                ctx,
                *desired_child,
                dest_container,
                previous,
            ) {
                ctx.edits.push(Edit::MoveNode {
                    node: *desired_child,
                    new_parent: dest_container,
                    new_prev_sibling: previous,
                });
            }
        }

        previous = Some(*desired_child);
    }
}

fn reconcile_properties_level(
    snapshot: &ProcessTreeSnapshot,
    source_container: NodeId,
    dest_container: NodeId,
    ctx: &mut ProcessCtx,
) {
    let mut desired = HashSet::new();
    let mut desired_children = Vec::new();
    let mut previous_existing = None;
    for source in snapshot.child_ids(source_container) {
        let Some(source_node) = snapshot.node(source) else {
            continue;
        };
        let decl_id = processor_surface_decl_id(source_node.uuid);
        let Some(expected_tree) = processor_surface_child_tree(snapshot, source) else {
            continue;
        };
        desired.insert(decl_id.clone());
        let Some(existing) =
            snapshot.find_child_by_decl_id(dest_container, &decl_id)
        else {
            let already_queued = ctx.edits.pending.iter().any(|req| {
                if let Edit::AddNodeTree { tree, parent: p, .. } = &req.edit {
                    *p == dest_container
                        && tree.node.node_data().meta.decl_id.0 == decl_id
                } else {
                    false
                }
            });
            if !already_queued {
                ctx.add_child_tree(
                    dest_container,
                    expected_tree,
                    previous_existing,
                );
            }
            continue;
        };
        let Some(existing_node) = snapshot.node(existing) else {
            continue;
        };
        desired_children.push(existing);
        previous_existing = Some(existing);
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
        } else if source_node.node_type == PROPERTY_FOLDER_NODE_TYPE {
            reconcile_properties_level(snapshot, source, existing, ctx);
        }
    }

    sync_processor_surface_order(snapshot, dest_container, &desired_children, ctx);

    for child in snapshot.child_ids(dest_container) {
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

#[node("state_processor_managed_regions", label = "Managed Regions")]
pub struct StateProcessorManagedRegions {}

#[node("state_processor_managed_regions", from_struct)]
impl Node for StateProcessorManagedRegions {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions =
            locked_instance_permissions();
        self.node_data_mut().meta.can_be_disabled = false;
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("state_processor_managed_region", label = "Managed Region")]
pub struct StateProcessorManagedRegion {}

#[node("state_processor_managed_region", from_struct)]
impl Node for StateProcessorManagedRegion {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[ANODE_ITEM_KIND]))
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        let roles = managed_region_roles_from_tags(&self.node_data().meta.tags);
        anode_container_accepts_for_roles(item_type, item_kind, &roles)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        let roles = managed_region_roles_from_tags(&self.node_data().meta.tags);
        anode_creatable_items_for_roles(&roles)
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        create_anode_user_item(node_type)
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        let mut permissions = NodeUserPermissions::all();
        permissions.can_edit_name = false;
        self.node_data_mut().meta.user_permissions = permissions;
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
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

const PROCESSOR_MANAGER_ITEMS_CHANGED_TOPIC: &str =
    "state_processor_manager_items_changed";

impl StateProcessorManager {
    fn refresh_formula_items(&mut self, ctx: &mut ProcessCtx) {
        self.formula_items = ctx
            .tree_snapshot()
            .map(|snapshot| FormulaCatalog::from_snapshot(snapshot).processor_palette_items())
            .unwrap_or_default();
        let all_items = self.user_creatable_items();
        let _ = ctx.emit_custom_payload(
            PROCESSOR_MANAGER_ITEMS_CHANGED_TOPIC,
            Some(self.id()),
            &all_items,
        );
    }
}

fn create_processor_for_formula_type(node_type: &str) -> Option<Box<dyn Node>> {
    let source = FormulaSourceRef::parse_processor_create_type(node_type).ok()?;
    if matches!(source, FormulaSourceRef::Builtin { .. })
        && FormulaCatalog::with_builtins()
            .resolve_builtin(&source)
            .is_err()
    {
        return None;
    }
    let mut processor = StateProcessor::new();
    processor.set_formula_source(source);
    Some(Box::new(processor))
}

#[node("state_processor_folder", label = "Folder")]
pub struct StateProcessorFolder {
    #[state(default = Vec::new())]
    formula_items: Vec<UserCreatableItem>,
}

#[node("state_processor_folder", from_struct)]
impl Node for StateProcessorFolder {
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
        initialize_processor_item(self);
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

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

impl StateProcessorFolder {
    fn refresh_formula_items(&mut self, ctx: &mut ProcessCtx) {
        self.formula_items = ctx
            .tree_snapshot()
            .map(|snapshot| FormulaCatalog::from_snapshot(snapshot).processor_palette_items())
            .unwrap_or_default();
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
    formula_source_key: String = String::new() (
        label = "Formula Source",
        read_only = true,
        show_in_inspector_content = false
    );
    node managed_regions: StateProcessorManagedRegions = StateProcessorManagedRegions::new() (
        label = "Managed Regions",
        show_in_inspector_content = false
    );
)]
pub struct StateProcessor {
    #[state(default = ProcessorFormulaSourceState::default(), persist)]
    formula_source: ProcessorFormulaSourceState,
    #[state(default = None)]
    subscribed_formula: Option<NodeId>,
}

#[node("state_processor", from_struct)]
impl Node for StateProcessor {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        initialize_processor_item(self);
        self.sync_formula_source_key();
        self.reconcile_formula(ctx);
    }

    fn on_node_ready(
        &mut self,
        ctx: &mut ProcessCtx,
        _context: NodeCreationContext,
    ) {
        self.refresh_formula_subscription(ctx);
        self.reconcile_formula(ctx);
    }

    fn on_param_change(
        &mut self,
        ctx: &mut ProcessCtx,
        param: NodeId,
        _old_value: ParamValue,
    ) {
        if param == self.formula.id() {
            self.sync_formula_source_from_reference();
            self.refresh_formula_subscription(ctx);
        }
        self.reconcile_formula(ctx);
    }

    fn on_node_created(&mut self, ctx: &mut ProcessCtx, _node: NodeId) {
        self.reconcile_formula(ctx);
    }

    fn on_node_deleted(&mut self, ctx: &mut ProcessCtx, _node: NodeId) {
        self.refresh_formula_subscription(ctx);
        self.reconcile_formula(ctx);
    }

    fn on_child_added(
        &mut self,
        ctx: &mut ProcessCtx,
        _parent: NodeId,
        _child: NodeId,
    ) {
        self.reconcile_formula(ctx);
    }

    fn on_child_removed(
        &mut self,
        ctx: &mut ProcessCtx,
        _parent: NodeId,
        _child: NodeId,
    ) {
        self.reconcile_formula(ctx);
    }

    fn on_meta_changed(
        &mut self,
        ctx: &mut ProcessCtx,
        _node: NodeId,
        _patch: NodeMetaPatch,
    ) {
        self.reconcile_formula(ctx);
    }

    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        3
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

impl StateProcessor {
    fn set_formula_source(&mut self, source: FormulaSourceRef) {
        self.formula_source = ProcessorFormulaSourceState::from_source(&source);
        match source {
            FormulaSourceRef::ProjectNode(reference) => self.formula.apply_runtime_value(
                &ParamValue::Reference(reference),
            ),
            FormulaSourceRef::Builtin { .. } => {
                self.formula.apply_runtime_value(&ParamValue::Reference(
                    NodeReference::default(),
                ))
            }
        };
        self.sync_formula_source_key();
    }

    fn sync_formula_source_from_reference(&mut self) {
        let reference = self.formula.get_ref();
        self.formula_source = if reference.is_empty() {
            ProcessorFormulaSourceState::Empty
        } else {
            ProcessorFormulaSourceState::from_source(&FormulaSourceRef::ProjectNode(
                reference.clone(),
            ))
        };
        self.sync_formula_source_key();
    }

    fn sync_formula_source_key(&mut self) {
        let value = self
            .formula_source_ref()
            .ok()
            .flatten()
            .map(|source| source.processor_create_type())
            .unwrap_or_default();
        self.formula_source_key.apply_runtime_value(&ParamValue::Str(value));
    }

    fn formula_source_ref(
        &self,
    ) -> Result<Option<FormulaSourceRef>, catalog::FormulaSourceParseError> {
        match self.formula_source.to_source_ref()? {
            Some(source) => Ok(Some(source)),
            None => {
                let reference = self.formula.get_ref();
                if reference.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(FormulaSourceRef::ProjectNode(reference.clone())))
                }
            }
        }
    }

    fn formula_node(&self, snapshot: &ProcessTreeSnapshot) -> Option<NodeId> {
        let FormulaSourceRef::ProjectNode(reference) =
            self.formula_source_ref().ok().flatten()?
        else {
            return None;
        };
        snapshot.node_id_by_uuid(reference.uuid()).filter(|formula| {
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

    fn reconcile_formula(&self, ctx: &mut ProcessCtx) {
        self.reconcile_formula_properties(ctx);
        self.reconcile_formula_managed_regions(ctx);
        self.reconcile_formula_warning(ctx);
    }

    /// Surface a warning on the processor itself when its formula reference is
    /// missing or the referenced formula has compilation errors, so the
    /// problem is visible directly in the processor list without opening the
    /// formula.
    fn reconcile_formula_warning(&self, ctx: &mut ProcessCtx) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };

        let warning = match self.formula_source_ref() {
            Err(error) => Some(("Invalid formula source", error.to_string())),
            Ok(None) => Some((
                "Missing formula",
                "This processor has no formula assigned.".to_owned(),
            )),
            Ok(Some(source @ FormulaSourceRef::Builtin { .. })) => {
                let catalog = FormulaCatalog::from_snapshot(&snapshot);
                catalog
                    .resolve_builtin(&source)
                    .err()
                    .map(|error| ("Missing formula", error.to_string()))
            }
            Ok(Some(FormulaSourceRef::ProjectNode(_))) => match self.formula_node(&snapshot) {
                None => Some((
                    "Missing formula",
                    "The referenced formula could not be found.".to_owned(),
                )),
                Some(formula) => {
                node_has_warning(&snapshot, formula, FORMULA_WARNING_ID).then(|| {
                    let detail =
                        node_warning_detail(&snapshot, formula, FORMULA_WARNING_ID)
                            .unwrap_or_else(|| "The formula has errors.".to_owned());
                    ("Formula has errors", detail)
                })
                }
            },
        };

        match warning {
            None => {
                if node_has_warning(&snapshot, self.id(), PROCESSOR_FORMULA_WARNING_ID) {
                    ctx.clear_node_warning(self.id(), Some(PROCESSOR_FORMULA_WARNING_ID));
                }
            }
            Some((message, detail)) => {
                if !node_warning_matches(
                    &snapshot,
                    self.id(),
                    PROCESSOR_FORMULA_WARNING_ID,
                    message,
                    Some(&detail),
                ) {
                    ctx.set_node_warning_with(
                        self.id(),
                        Some(PROCESSOR_FORMULA_WARNING_ID),
                        message,
                        Some(&detail),
                    );
                }
            }
        }
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
            let already_queued = ctx.edits.pending.iter().any(|req| {
                if let Edit::AddNodeTree { tree, parent: p, .. } = &req.edit {
                    *p == self.id()
                        && tree.node.node_data().meta.decl_id.0 == PROPERTIES_DECL_ID
                } else {
                    false
                }
            });
            if !already_queued {
                ctx.add_child_tree(
                    self.id(),
                    processor_properties_tree(&snapshot, formula),
                    None,
                );
            }
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

        reconcile_properties_level(&snapshot, source_properties, properties, ctx);
    }

    fn managed_region_definitions(
        &self,
        snapshot: &ProcessTreeSnapshot,
    ) -> Vec<ManagedRegionDefinition> {
        match self.formula_source_ref() {
            Ok(Some(source @ FormulaSourceRef::Builtin { .. })) => {
                FormulaCatalog::from_snapshot(snapshot)
                    .resolve_builtin(&source)
                    .map(|formula| formula.surface.managed_regions)
                    .unwrap_or_default()
            }
            Ok(Some(FormulaSourceRef::ProjectNode(_))) => self
                .formula_node(snapshot)
                .and_then(|formula| formula_from_snapshot(snapshot, formula).ok())
                .map(|formula| formula.surface.managed_regions)
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    fn reconcile_formula_managed_regions(&self, ctx: &mut ProcessCtx) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        let Some(regions_root) =
            snapshot.find_child_by_decl_id(self.id(), PROCESSOR_MANAGED_REGIONS_DECL_ID)
        else {
            return;
        };
        let mut desired = HashSet::new();
        for definition in self.managed_region_definitions(&snapshot) {
            let decl_id = processor_managed_region_decl_id(definition.id.as_str());
            desired.insert(decl_id.clone());
            let Some(existing) =
                snapshot.find_child_by_decl_id(regions_root, &decl_id)
            else {
                let already_queued = ctx.edits.pending.iter().any(|req| {
                    if let Edit::AddNodeTree { tree, parent: p, .. } = &req.edit {
                        *p == regions_root
                            && tree.node.node_data().meta.decl_id.0 == decl_id
                    } else {
                        false
                    }
                });
                if !already_queued {
                    ctx.add_child_tree(
                        regions_root,
                        processor_managed_region_tree(&definition),
                        None,
                    );
                }
                continue;
            };
            let Some(existing_node) = snapshot.node(existing) else {
                continue;
            };
            let desired_tags = managed_region_tags(&definition);
            if existing_node.label != definition.label
                || existing_node.tags != desired_tags
            {
                ctx.patch_node_meta(
                    existing,
                    NodeMetaPatch {
                        label: Some(definition.label.clone()),
                        tags: Some(desired_tags),
                        ..NodeMetaPatch::default()
                    },
                );
            }
        }

        for child in snapshot.child_ids(regions_root) {
            let Some(node) = snapshot.node(child) else {
                continue;
            };
            if node.decl_id.starts_with(PROCESSOR_MANAGED_REGION_DECL_PREFIX)
                && !desired.contains(&node.decl_id)
            {
                ctx.edits.push(Edit::RemoveNode { node: child });
            }
        }
    }
}

#[cfg(test)]
mod processor_tests;
