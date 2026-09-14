use std::collections::{HashMap, HashSet};

use chataigne_alchemist::{ManagedRegionDefinition, SurfaceItemKind};
use golden_core::{
    edit::{Edit, NodeTree},
    events::{Event, EventFrame, EventKind},
    node,
    node::{
        CONTEXT_LINK_LANE_DEFERRED_TAG, DeclId, EventPropagation, Node, NodeCreationContext, NodeId, NodeMetaPatch,
        NodeReference, NodeUuid, NodeUserPermissions, UserContainerRules,
        UserContextNode, UserCreatableItem, USER_CONTEXT_DEFAULT_LABEL,
        USER_CONTEXT_ITEM_KIND, USER_CONTEXT_NODE_TYPE,
    },
    parameter::{ParamValue, ReferenceTargetKind},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::systems_alchemist_formula::{
    anode_container_accepts_for_roles, anode_creatable_items_for_roles, create_anode_user_item,
    create_anode_user_item_tree, formula_from_snapshot, node_has_warning, node_warning_detail, node_warning_matches,
    ANODE_ITEM_KIND, ANODE_NODE_TYPE, FORMULA_EXTERNAL_BUILTIN_TAG_PREFIX, FORMULA_WARNING_ID, PROPERTIES_DECL_ID,
    PROPERTY_FOLDER_NODE_TYPE, PROPERTY_MANAGER_NODE_TYPE, PROPERTY_NODE_TYPE,
};
use crate::app::{AppEngine, ConditionManager};

mod catalog;
mod conversion;
mod factory;
mod managed_regions;
mod palette;
mod source_schema;
mod surface;

pub(crate) use managed_regions::managed_regions_from_snapshot;
pub(crate) use source_schema::{managed_source_node, managed_source_schema};

use self::catalog::BUILTIN_FORMULA_CONTENT_TAG_PREFIX;
use self::factory::ProcessorTreeTemplates;
use self::surface::processor_surface_child_tree;

pub(crate) use self::catalog::{
    shared_formula_dir_from_snapshot, FormulaCatalog, FormulaSourceRef, ProcessorFormulaSourceState,
};

/// Adds any built-in/shared formulas not yet present in the project's
/// Formula Library. Run on every project load (including brand-new
/// projects, right after they're seeded) so formulas shipped or added to
/// the user's shared folder in a newer app version show up in projects
/// saved by an older one, instead of only ever being baked in at creation
/// time.
pub(crate) fn sync_external_formulas(engine: &mut AppEngine) -> Result<(), String> {
    let started = std::time::Instant::now();
    apply_formula_sync_edits(engine, "startup formula prerequisites")?;
    let prerequisites_elapsed = started.elapsed();

    let snapshot = engine.process_tree_snapshot();
    let Some(library) = find_formula_library(&snapshot) else {
        return Ok(());
    };

    let builtin_candidates = FormulaCatalog::default_builtin_formula_trees()
        .map_err(|error| error.to_string())?;
    let existing_builtins = builtin_formula_nodes(&snapshot, library);
    let candidate_uuids = builtin_candidates
        .iter()
        .map(|tree| tree.node.node_data().meta.uuid)
        .collect::<HashSet<_>>();
    for node_id in &existing_builtins {
        let current = snapshot.node(*node_id).expect("listed builtin should exist");
        if !candidate_uuids.contains(&current.uuid) {
            return Err(format!(
                "builtin formula '{}' ({}) is absent from the current asset catalog; project data was left intact",
                current.label, current.uuid.0
            ));
        }
    }
    let existing_by_uuid = existing_builtins
        .iter()
        .filter_map(|node_id| {
            snapshot
                .node(*node_id)
                .map(|node| (node.uuid, *node_id))
        })
        .collect::<HashMap<_, _>>();
    let mut stale_builtins = existing_builtins.iter().copied().collect::<HashSet<_>>();
    let mut builtin_trees = Vec::new();
    for tree in builtin_candidates {
        let candidate = tree.node.node_data();
        let candidate_content_tag = builtin_formula_content_tag(&candidate.meta.tags);
        let current = existing_by_uuid.get(&candidate.meta.uuid).copied();
        let current_content_tag = current
            .and_then(|node_id| snapshot.node(node_id))
            .and_then(|node| builtin_formula_content_tag(&node.tags));
        if candidate_content_tag.is_some() && candidate_content_tag == current_content_tag {
            if let Some(node_id) = current {
                stale_builtins.remove(&node_id);
            }
        } else {
            builtin_trees.push(tree);
        }
    }

    let shared_sync = if let Some(shared_dir) = shared_formula_dir_from_snapshot(&snapshot) {
        let stale_nodes =
            FormulaCatalog::stale_shared_formula_nodes(&snapshot, library, &shared_dir)
                .map_err(|error| error.to_string())?;
        let missing_trees =
            FormulaCatalog::missing_shared_formula_trees(&snapshot, library, &shared_dir)
                .map_err(|error| error.to_string())?;
        Some((stale_nodes, missing_trees))
    } else {
        None
    };
    let discovery_elapsed = started.elapsed();

    let mut queued_formula_edits = false;
    let mut queued_removals = false;
    let mut formula_trees = builtin_trees;

    for node in stale_builtins {
        queued_formula_edits = true;
        queued_removals = true;
        engine.edits.push(Edit::RemoveNode { node });
    }

    if let Some((stale_nodes, missing_trees)) = shared_sync {
        for node in stale_nodes {
            queued_formula_edits = true;
            queued_removals = true;
            engine.edits.push(Edit::RemoveNode { node });
        }
        formula_trees.extend(missing_trees);
    }

    if queued_removals {
        apply_formula_sync_edits(engine, "external formula removal")?;
    }
    if !formula_trees.is_empty() {
        queued_formula_edits = true;
        engine
            .apply_project_load_node_trees(formula_trees, library, None)
            .map_err(|error| format!("external formula batch insertion failed: {error}"))?;
    }
    let edits_elapsed = started.elapsed();
    move_builtin_formulas_to_front(engine, library)?;
    eprintln!(
        "[formula-sync] prerequisites_ms={} discovery_ms={} edits_ms={} ordering_ms={} total_ms={} queued_edits={}",
        prerequisites_elapsed.as_millis(),
        discovery_elapsed.saturating_sub(prerequisites_elapsed).as_millis(),
        edits_elapsed.saturating_sub(discovery_elapsed).as_millis(),
        started.elapsed().saturating_sub(edits_elapsed).as_millis(),
        started.elapsed().as_millis(),
        queued_formula_edits
    );

    Ok(())
}

fn apply_formula_sync_edits(engine: &mut AppEngine, context: &str) -> Result<(), String> {
    engine
        .apply_project_load_edits()
        .map_err(|error| format!("{context} failed: {error}"))
}

fn move_builtin_formulas_to_front(engine: &mut AppEngine, library: NodeId) -> Result<(), String> {
    let snapshot = engine.process_tree_snapshot();
    if !formula_library_has_late_builtin(&snapshot, library) {
        return Ok(());
    }

    for node in builtin_formula_nodes(&snapshot, library).into_iter().rev() {
        engine.edits.push(Edit::MoveNode {
            node,
            new_parent: library,
            new_prev_sibling: None,
        });
    }
    apply_formula_sync_edits(engine, "formula library ordering")
}

fn formula_library_has_late_builtin(snapshot: &ProcessTreeSnapshot, library: NodeId) -> bool {
    let mut seen_non_builtin = false;
    for child in snapshot.child_ids(library) {
        if is_builtin_formula_node(snapshot, child) {
            if seen_non_builtin {
                return true;
            }
        } else {
            seen_non_builtin = true;
        }
    }
    false
}

fn builtin_formula_nodes(snapshot: &ProcessTreeSnapshot, library: NodeId) -> Vec<NodeId> {
    snapshot
        .child_ids(library)
        .into_iter()
        .filter(|node_id| is_builtin_formula_node(snapshot, *node_id))
        .collect()
}

fn is_builtin_formula_node(snapshot: &ProcessTreeSnapshot, node_id: NodeId) -> bool {
    snapshot.node(node_id).is_some_and(|node| {
        node.node_type == FORMULA_NODE_TYPE
            && node
                .tags
                .iter()
                .any(|tag| tag.starts_with(FORMULA_EXTERNAL_BUILTIN_TAG_PREFIX))
    })
}

fn builtin_formula_content_tag(tags: &[String]) -> Option<&str> {
    tags.iter()
        .find(|tag| tag.starts_with(BUILTIN_FORMULA_CONTENT_TAG_PREFIX))
        .map(String::as_str)
}

const FORMULA_LIBRARY_NODE_TYPE: &str = "alchemist_formula_library";
const FORMULA_NODE_TYPE: &str = "alchemist_formula";
const PROCESSOR_SURFACE_DECL_PREFIX: &str = "surface/";
const PROCESSOR_SURFACE_IDENTITY_TAG_PREFIX: &str = "chataigne.processor.surface.identity:";
const PROCESSOR_FORMULA_WARNING_ID: &str = "state_processor_formula";
pub(crate) const PROCESSOR_FORMULA_SOURCE_DECL_ID: &str = "formula_source_key";
pub(crate) const PROCESSOR_MANAGED_REGIONS_DECL_ID: &str = "managed_regions";
pub(crate) const PROCESSOR_MANAGED_REGION_DECL_PREFIX: &str = "managed_region/";
const PROCESSOR_MANAGED_REGION_ROLE_TAG_PREFIX: &str =
    "state_processor.managed_region.role:";
pub(crate) const PROCESSOR_ITEM_KIND: &str = "state_processor";
pub(crate) const PROCESSOR_FOLDER_ITEM_KIND: &str = "state_processor_folder";
pub(crate) const PROCESSOR_FOLDER_NODE_TYPE: &str = "state_processor_folder";

pub(crate) fn processor_formula_source_ref(
    snapshot: &ProcessTreeSnapshot,
    processor_node: NodeId,
) -> Option<FormulaSourceRef> {
    if let Some(ParamValue::Str(source)) = snapshot
        .find_child_by_decl_id(processor_node, PROCESSOR_FORMULA_SOURCE_DECL_ID)
        .and_then(|node| snapshot.node(node))
        .and_then(|node| node.param_value.as_ref())
        .filter(|value| matches!(value, ParamValue::Str(source) if !source.is_empty()))
    {
        if let Ok(source) = FormulaSourceRef::parse_processor_create_type(source) {
            return Some(source);
        }
    }
    snapshot.find_child_by_decl_id(processor_node, "formula")
        .and_then(|node| snapshot.node(node))
        .and_then(|node| match node.param_value.as_ref()? {
            ParamValue::Reference(reference) => Some(reference.uuid()),
            _ => None,
        })
        .map(FormulaSourceRef::project_uuid)
}

fn processor_container_rules() -> UserContainerRules {
    UserContainerRules::new(&[
        PROCESSOR_ITEM_KIND,
        PROCESSOR_FOLDER_ITEM_KIND,
        USER_CONTEXT_ITEM_KIND,
    ])
}

fn processor_container_accepts(item_type: &str, item_kind: &str) -> bool {
    match item_kind {
        PROCESSOR_ITEM_KIND => {
            item_type == StateProcessor::NODE_TYPE || item_type.starts_with("state_processor:")
        }
        PROCESSOR_FOLDER_ITEM_KIND => item_type == PROCESSOR_FOLDER_NODE_TYPE,
        USER_CONTEXT_ITEM_KIND => item_type == USER_CONTEXT_NODE_TYPE,
        _ => false,
    }
}

fn processor_context_creatable_item(separator_before: bool) -> UserCreatableItem {
    UserCreatableItem::new(
        USER_CONTEXT_NODE_TYPE,
        USER_CONTEXT_ITEM_KIND,
        USER_CONTEXT_DEFAULT_LABEL,
    )
    .with_separator_before(separator_before)
    .with_select_when_created(false)
}

fn create_processor_context_item(node_type: &str) -> Option<Box<dyn Node>> {
    (node_type == USER_CONTEXT_NODE_TYPE).then(|| {
        Box::new(UserContextNode::new_with_multiplex(
            USER_CONTEXT_DEFAULT_LABEL,
            true,
        )) as Box<dyn Node>
    })
}

fn initialize_processor_item(node: &mut dyn Node) {
    node.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
    let tags = &mut node.node_data_mut().meta.tags;
    if !tags.iter().any(|tag| tag == CONTEXT_LINK_LANE_DEFERRED_TAG) {
        tags.push(CONTEXT_LINK_LANE_DEFERRED_TAG.to_owned());
    }
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

fn processor_surface_decl_id_for_source(source_uuid: NodeUuid, tags: &[String]) -> String {
    let identity = tags
        .iter()
        .filter_map(|tag| tag.strip_prefix(PROCESSOR_SURFACE_IDENTITY_TAG_PREFIX))
        .find_map(|text| text.parse::<uuid::Uuid>().ok())
        .map(NodeUuid)
        .unwrap_or(source_uuid);
    processor_surface_decl_id(identity)
}

/// Removes every mirrored property surface node directly under `processor`.
///
/// Property surfaces are flattened to the processor's top level and carry a
/// `surface/` decl-id prefix, so this leaves the processor's own declared
/// children (formula reference, formula source key, managed regions) untouched.
fn remove_processor_surface_children(
    snapshot: &ProcessTreeSnapshot,
    processor: NodeId,
    ctx: &mut ProcessCtx,
) {
    for child in snapshot.child_ids(processor) {
        let Some(node) = snapshot.node(child) else {
            continue;
        };
        if node.decl_id.starts_with(PROCESSOR_SURFACE_DECL_PREFIX) {
            ctx.edits.push(Edit::RemoveNode { node: child });
        }
    }
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

fn processor_managed_regions_tree(definitions: &[ManagedRegionDefinition]) -> NodeTree {
    let mut regions = StateProcessorManagedRegions::new();
    let meta = &mut regions.node_data_mut().meta;
    meta.decl_id = DeclId(PROCESSOR_MANAGED_REGIONS_DECL_ID.to_owned());
    meta.presentation.show_in_inspector_content = false;
    let mut tree = NodeTree::new(regions);
    for definition in definitions {
        tree.push_child(processor_managed_region_tree(definition));
    }
    tree
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
    source_snapshot: &ProcessTreeSnapshot,
    source_container: NodeId,
    dest_snapshot: &ProcessTreeSnapshot,
    dest_container: NodeId,
    ctx: &mut ProcessCtx,
) {
    let mut desired = HashSet::new();
    let mut desired_children = Vec::new();
    let mut previous_existing = None;
    for source in source_snapshot.child_ids(source_container) {
        let Some(source_node) = source_snapshot.node(source) else {
            continue;
        };
        let decl_id = processor_surface_decl_id_for_source(source_node.uuid, &source_node.tags);
        let Some(expected_tree) = processor_surface_child_tree(source_snapshot, source) else {
            continue;
        };
        desired.insert(decl_id.clone());
        let Some(existing) =
            dest_snapshot.find_child_by_decl_id(dest_container, &decl_id)
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
        let Some(existing_node) = dest_snapshot.node(existing) else {
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
        let label_changed = existing_node.label != source_node.label;
        let source_color = source_node
            .presentation
            .color
            .or(source_node.presentation.default_color);
        let color_changed = existing_node.presentation.default_color != source_color;
        if label_changed || color_changed {
            let presentation = color_changed.then(|| {
                let mut presentation = existing_node.presentation.clone();
                presentation.default_color = source_color;
                presentation
            });
            ctx.patch_node_meta(
                existing,
                NodeMetaPatch {
                    label: label_changed.then(|| source_node.label.clone()),
                    presentation,
                    ..NodeMetaPatch::default()
                },
            );
        }
        if source_node.node_type == PROPERTY_NODE_TYPE {
            let Some(source_value) =
                source_snapshot.find_child_by_decl_id(source, "value")
            else {
                continue;
            };
            if let Some(constraints) = source_snapshot
                .node(source_value)
                .and_then(|node| node.param_constraints.clone())
                .filter(|constraints| {
                    dest_snapshot
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
            reconcile_properties_level(
                source_snapshot,
                source,
                dest_snapshot,
                existing,
                ctx,
            );
        }
    }

    sync_processor_surface_order(dest_snapshot, dest_container, &desired_children, ctx);

    for child in dest_snapshot.child_ids(dest_container) {
        let Some(node) = dest_snapshot.node(child) else {
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
pub struct StateProcessorManagedRegion {
    #[state(default = Vec::new())]
    filter_items: Vec<UserCreatableItem>,
    #[state(default = HashSet::new())]
    structural_palette_params: HashSet<NodeId>,
}

#[node("state_processor_managed_region", from_struct)]
impl Node for StateProcessorManagedRegion {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[ANODE_ITEM_KIND]))
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        let roles = managed_region_roles_from_tags(&self.node_data().meta.tags);
        if roles.contains(&SurfaceItemKind::Filter) {
            return item_kind == ANODE_ITEM_KIND
                && (item_type == ANODE_NODE_TYPE
                    || self.filter_items.iter().any(|item| item.node_type == item_type));
        }
        anode_container_accepts_for_roles(item_type, item_kind, &roles)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        let roles = managed_region_roles_from_tags(&self.node_data().meta.tags);
        if roles.contains(&SurfaceItemKind::Filter) {
            return self.filter_items.clone();
        }
        anode_creatable_items_for_roles(&roles)
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        create_anode_user_item(node_type)
    }

    fn create_user_item_tree(&self, node_type: &str) -> Option<NodeTree> {
        create_anode_user_item_tree(node_type)
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        let mut permissions = NodeUserPermissions::all();
        permissions.can_edit_name = false;
        self.node_data_mut().meta.user_permissions = permissions;
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        if !managed_region_roles_from_tags(&self.node_data().meta.tags)
            .contains(&SurfaceItemKind::Filter)
        {
            return;
        }
        let subscriptions = ctx.tree_snapshot().and_then(|snapshot| {
            let regions_root = snapshot.node(self.id())?.parent?;
            let processor = snapshot.node(regions_root)?.parent?;
            let formula_params = ["formula", PROCESSOR_FORMULA_SOURCE_DECL_ID]
                .into_iter()
                .filter_map(|decl_id| snapshot.find_child_by_decl_id(processor, decl_id))
                .collect::<Vec<_>>();
            Some((regions_root, formula_params))
        });
        if let Some((regions_root, formula_params)) = subscriptions {
            ctx.add_event_listener_subtree(self.id(), regions_root, u32::MAX);
            for param in formula_params {
                ctx.add_event_listener(self.id(), param);
            }
        }
        self.refresh_filter_palette(ctx);
    }

    fn on_inbox(&mut self, ctx: &mut ProcessCtx) {
        if filter_palette_events_require_refresh(&ctx.events, &self.structural_palette_params) {
            self.refresh_filter_palette(ctx);
        }
    }

    fn inbox_requires_tree_snapshot(&self, events: &EventFrame) -> bool {
        filter_palette_events_require_refresh(events, &self.structural_palette_params)
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
    #[state(default = ProcessorTreeTemplates::default())]
    tree_templates: ProcessorTreeTemplates,
}

#[node(
    "state_processor_manager",
    from_struct,
    contextualizable = golden_core::node::UserContextHostPolicy::multiplex_contextualizable()
)]
impl Node for StateProcessorManager {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(processor_container_rules())
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        processor_container_accepts(item_type, item_kind)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        let mut items = self.formula_items.clone();
        items.push(
            UserCreatableItem::new(PROCESSOR_FOLDER_NODE_TYPE, PROCESSOR_FOLDER_ITEM_KIND, "Folder")
                .with_separator_before(!items.is_empty()),
        );
        items.push(processor_context_creatable_item(!items.is_empty()));
        items
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        if let Some(context) = create_processor_context_item(node_type) {
            return Some(context);
        }
        if node_type == PROCESSOR_FOLDER_NODE_TYPE {
            return Some(Box::new(StateProcessorFolder::new()));
        }
        create_processor_for_formula_type(node_type)
    }

    fn create_user_item_tree(&self, node_type: &str) -> Option<NodeTree> {
        self.tree_templates.create_tree(node_type, || self.create_user_item(node_type))
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

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        if self.tree_templates.watches(param) {
            self.refresh_formula_items(ctx);
        }
    }

    fn inbox_requires_tree_snapshot(&self, events: &EventFrame) -> bool {
        processor_palette_inbox_requires_tree_snapshot(events, &self.tree_templates)
    }

    fn event_propagation(&self, event: &Event, _depth: u32) -> EventPropagation {
        self.tree_templates.event_propagation(event)
    }
}

impl StateProcessorManager {
    fn refresh_formula_items(&mut self, ctx: &mut ProcessCtx) {
        self.tree_templates = ctx
            .tree_snapshot()
            .map(ProcessorTreeTemplates::from_snapshot)
            .unwrap_or_default();
        self.formula_items = ctx
            .tree_snapshot()
            .map(|snapshot| FormulaCatalog::from_snapshot(snapshot).processor_palette_items())
            .unwrap_or_default();
        let all_items = self.user_creatable_items();
        let _ = ctx.emit_custom_payload(
            golden_core::events::NODE_CREATABLE_ITEMS_CHANGED_TOPIC,
            Some(self.id()),
            &all_items,
        );
    }
}

fn create_processor_for_formula_type(node_type: &str) -> Option<Box<dyn Node>> {
    let source = FormulaSourceRef::parse_processor_create_type(node_type).ok()?;
    let mut processor = StateProcessor::new();
    processor.set_formula_source(source);
    Some(Box::new(processor))
}

#[node("state_processor_folder", label = "Folder")]
pub struct StateProcessorFolder {
    #[state(default = Vec::new())]
    formula_items: Vec<UserCreatableItem>,
    #[state(default = ProcessorTreeTemplates::default())]
    tree_templates: ProcessorTreeTemplates,
}

#[node(
    "state_processor_folder",
    from_struct,
    contextualizable = golden_core::node::UserContextHostPolicy::multiplex_contextualizable()
)]
impl Node for StateProcessorFolder {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(processor_container_rules())
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        processor_container_accepts(item_type, item_kind)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        let mut items = self.formula_items.clone();
        items.push(
            UserCreatableItem::new(PROCESSOR_FOLDER_NODE_TYPE, PROCESSOR_FOLDER_ITEM_KIND, "Folder")
                .with_separator_before(!items.is_empty()),
        );
        items.push(processor_context_creatable_item(!items.is_empty()));
        items
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        if let Some(context) = create_processor_context_item(node_type) {
            return Some(context);
        }
        if node_type == PROCESSOR_FOLDER_NODE_TYPE {
            return Some(Box::new(StateProcessorFolder::new()));
        }
        create_processor_for_formula_type(node_type)
    }

    fn create_user_item_tree(&self, node_type: &str) -> Option<NodeTree> {
        self.tree_templates.create_tree(node_type, || self.create_user_item(node_type))
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

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        if self.tree_templates.watches(param) {
            self.refresh_formula_items(ctx);
        }
    }

    fn inbox_requires_tree_snapshot(&self, events: &EventFrame) -> bool {
        processor_palette_inbox_requires_tree_snapshot(events, &self.tree_templates)
    }

    fn event_propagation(&self, event: &Event, _depth: u32) -> EventPropagation {
        self.tree_templates.event_propagation(event)
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

impl StateProcessorFolder {
    fn refresh_formula_items(&mut self, ctx: &mut ProcessCtx) {
        self.tree_templates = ctx
            .tree_snapshot()
            .map(ProcessorTreeTemplates::from_snapshot)
            .unwrap_or_default();
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
        reference_allow_projections = false,
        show_in_inspector_content = false
    );
    formula_source_key: String = String::new() (
        label = "Formula Source",
        read_only = true,
        show_in_inspector_content = false
    );
    convert_to_formula: ParamValue = ParamValue::Trigger() (
        label = "Convert to Formula",
        show_in_inspector_content = false
    );
)]
pub struct StateProcessor {
    #[state(default = ProcessorFormulaSourceState::default(), persist)]
    formula_source: ProcessorFormulaSourceState,
    #[state(default = None)]
    subscribed_formula: Option<NodeId>,
    #[state(default = HashSet::new())]
    condition_valid_params: HashSet<NodeId>,
}

fn processor_palette_inbox_requires_tree_snapshot(
    events: &EventFrame,
    templates: &ProcessorTreeTemplates,
) -> bool {
    events.iter().any(|event| {
        matches!(
            event.kind,
            EventKind::NodeCreated { .. }
                | EventKind::NodeDeleted { .. }
                | EventKind::MetaChanged { .. }
                | EventKind::ChildAdded { .. }
                | EventKind::ChildRemoved { .. }
        ) || matches!(event.kind, EventKind::ParamChanged { param, .. } if templates.watches(param))
    })
}

fn filter_palette_events_require_refresh(
    events: &EventFrame,
    structural_params: &HashSet<NodeId>,
) -> bool {
    events.iter().any(|event| match &event.kind {
        EventKind::ParamChanged { param, .. } => structural_params.contains(param),
        EventKind::Custom(_) => false,
        _ => true,
    })
}

impl StateProcessorManagedRegion {
    fn refresh_filter_palette(&mut self, ctx: &mut ProcessCtx) {
        let Some(snapshot) = ctx.tree_snapshot() else {
            return;
        };
        let params = palette::structural_palette_params(snapshot, self.id());
        let items = palette::filter_palette_from_snapshot(snapshot, self.id());
        self.structural_palette_params = params;
        if self.filter_items != items {
            self.filter_items = items;
            let _ = ctx.emit_latest_custom_payload(
                golden_core::events::NODE_CREATABLE_ITEMS_CHANGED_TOPIC,
                Some(self.id()),
                &self.filter_items,
            );
        }
    }
}

#[node(
    "state_processor",
    from_struct,
    contextualizable = golden_core::node::UserContextHostPolicy::multiplex_contextualizable()
)]
impl Node for StateProcessor {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[USER_CONTEXT_ITEM_KIND]))
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        item_kind == USER_CONTEXT_ITEM_KIND && item_type == USER_CONTEXT_NODE_TYPE
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![processor_context_creatable_item(false)]
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        create_processor_context_item(node_type)
    }

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
        self.refresh_condition_valid_params(ctx);
        self.reconcile_formula(ctx);
    }

    fn on_param_change(
        &mut self,
        ctx: &mut ProcessCtx,
        param: NodeId,
        _old_value: ParamValue,
    ) {
        if param == self.convert_to_formula.id() {
            self.convert_mapping_to_formula(ctx);
            return;
        }
        if self.condition_valid_params.contains(&param) {
            return;
        }
        if param == self.formula.id() {
            self.sync_formula_source_from_reference();
            self.refresh_formula_subscription(ctx);
        }
        self.reconcile_formula(ctx);
    }

    fn on_node_created(&mut self, ctx: &mut ProcessCtx, _node: NodeId) {
        self.refresh_condition_valid_params(ctx);
        self.reconcile_formula(ctx);
    }

    fn on_node_deleted(&mut self, ctx: &mut ProcessCtx, _node: NodeId) {
        self.refresh_condition_valid_params(ctx);
        self.refresh_formula_subscription(ctx);
        self.reconcile_formula(ctx);
    }

    fn on_child_added(
        &mut self,
        ctx: &mut ProcessCtx,
        _parent: NodeId,
        _child: NodeId,
    ) {
        self.refresh_condition_valid_params(ctx);
        self.reconcile_formula(ctx);
    }

    fn on_child_removed(
        &mut self,
        ctx: &mut ProcessCtx,
        _parent: NodeId,
        _child: NodeId,
    ) {
        self.refresh_condition_valid_params(ctx);
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

    fn inbox_requires_tree_snapshot(&self, events: &EventFrame) -> bool {
        events.iter().any(|event| match &event.kind {
            EventKind::Custom(_) => false,
            EventKind::ParamChanged { param, .. } => !self.condition_valid_params.contains(param),
            _ => true,
        })
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

impl StateProcessor {
    fn refresh_condition_valid_params(&mut self, ctx: &ProcessCtx) {
        let Some(snapshot) = ctx.tree_snapshot() else {
            return;
        };
        self.condition_valid_params = Self::condition_valid_params_in_snapshot(snapshot, self.id());
    }

    fn condition_valid_params_in_snapshot(snapshot: &ProcessTreeSnapshot, processor: NodeId) -> HashSet<NodeId> {
        let mut valid_params = HashSet::new();
        for child in snapshot.child_ids_slice(processor) {
            if snapshot
                .node(*child)
                .is_some_and(|node| node.node_type == ConditionManager::NODE_TYPE)
            {
                if let Some(valid) = snapshot.find_child_by_decl_id(*child, "valid") {
                    valid_params.insert(valid);
                }
            }
        }
        valid_params
    }

    fn set_formula_source(&mut self, source: FormulaSourceRef) {
        self.formula_source = ProcessorFormulaSourceState::from_source(&source);
        let FormulaSourceRef::ProjectNode(reference) = source;
        self.formula
            .apply_runtime_value(&ParamValue::Reference(reference));
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
        let FormulaSourceRef::ProjectNode(reference) = self.formula_source_ref().ok().flatten()?;
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

    fn reconcile_formula(&mut self, ctx: &mut ProcessCtx) {
        self.reconcile_formula_properties(ctx);
        self.reconcile_formula_managed_regions(ctx);
        self.reconcile_formula_warning(ctx);
        self.reconcile_formula_icon(ctx);
    }

    /// Mirrors the referenced formula's icon onto the processor itself, so a
    /// processor built from a formula with a custom icon (e.g. a built-in
    /// formula with a sibling `.svg`/`.png`) shows the same icon wherever the
    /// processor is displayed.
    fn reconcile_formula_icon(&mut self, ctx: &mut ProcessCtx) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        let icon = self
            .formula_node(&snapshot)
            .and_then(|formula| snapshot.node(formula))
            .and_then(|formula| formula.presentation.icon.clone());
        if self.node_data().meta.presentation.icon != icon {
            self.node_data_mut().meta.presentation.icon = icon;
        }
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

        // Properties are mirrored flat at the processor's top level (no
        // intermediate "Properties" folder), so the processor node itself is
        // the destination container for the formula's property surfaces.
        let Ok(Some(FormulaSourceRef::ProjectNode(_))) = self.formula_source_ref() else {
            remove_processor_surface_children(&snapshot, self.id(), ctx);
            return;
        };
        let Some(formula) = self.formula_node(&snapshot) else {
            return;
        };
        let Some(source_properties) =
            snapshot.find_child_by_decl_id(formula, PROPERTIES_DECL_ID)
        else {
            remove_processor_surface_children(&snapshot, self.id(), ctx);
            return;
        };
        reconcile_properties_level(
            &snapshot,
            source_properties,
            &snapshot,
            self.id(),
            ctx,
        );
    }

    fn reconcile_formula_managed_regions(&self, ctx: &mut ProcessCtx) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        let regions_root = snapshot.find_child_by_decl_id(self.id(), PROCESSOR_MANAGED_REGIONS_DECL_ID);
        let definitions = match self.formula_source_ref() {
            Ok(Some(FormulaSourceRef::ProjectNode(_))) => {
                let Some(formula) = self.formula_node(&snapshot) else {
                    return;
                };
                let Ok(formula) = formula_from_snapshot(&snapshot, formula) else {
                    // A project Formula may still be materializing on load. Its
                    // managed instance data must not be removed on a transient
                    // parse failure.
                    return;
                };
                formula.surface.managed_regions
            }
            // A missing source can be transient while a sparse project is
            // materializing. Keep authored items until a valid Formula can
            // identify which regions actually need to change.
            _ => {
                if regions_root.is_none() {
                    ctx.add_child_tree(self.id(), processor_managed_regions_tree(&[]), None);
                }
                return;
            }
        };
        let Some(regions_root) = regions_root else {
            ctx.add_child_tree(self.id(), processor_managed_regions_tree(&definitions), None);
            return;
        };
        if definitions.is_empty() {
            // Sparse load can expose a Formula before its managed-region
            // metadata is populated. Empty metadata is not enough evidence
            // to discard authored processor items.
            return;
        }
        let mut desired = HashSet::new();
        for definition in definitions {
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
mod tests;
