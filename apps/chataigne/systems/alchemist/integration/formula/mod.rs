use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
};

use chataigne_alchemist::{
    ANodeFieldPath, ANodeId, ANodeInstance, ANodeTypeId, AlchemistFormula,
    AlchemistGraphDomain, ColorValue, CompileCtx, DiagnosticOrigin,
    DiagnosticSeverity, FormulaContextContract, FormulaId, FormulaPropertyDecl,
    FormulaPropertyId, FormulaPropertySchema, FormulaSurface, InputSocketRef,
    ManagedRegionDefinition, OutputSocketRef, ParamUiHints,
    SignatureCtx, StableRef, SurfaceItem, SurfaceItemId, SurfaceItemKind,
    SurfaceSection, SurfaceSectionId, SurfaceSource, TriggerValue,
    TypeBindingSource, TypeBindings, TypeConstraint, TypeSolveCtx,
    ValueTypeId, ValueTypeSpec, PROCESS_ON_INPUT_CHANGE_ONLY_CONFIG,
    SEND_ON_OUTPUT_CHANGE_ONLY_CONFIG, compile_graph, solve_document_types,
};
use golden_values::Value as RuntimeValue;
use golden_core::{
    color::Color,
    edit::{Edit, NodeTree},
    engine::NodeExecutionRule,
    events::{Event, EventFrame, EventKind},
    item, node,
    node::{
        DeclId, Folder, GRADIENT_NODE_TYPE, GradientNode, GradientStop, Node,
        NodeCreationContext, NodeId, NodeMetaPatch, NodeReference,
        NodeUserPermissions, NodeUuid, UserContainerRules, UserCreatableItem,
        gradient_from_snapshot,
    },
    parameter::{
        CssValue, ParamValue, Parameter, ParameterChangeCheck,
        ParameterConstraintPolicy, ParameterEnumOption, ParameterEventBehaviour,
        ReferenceTargetKind,
    },
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};

use crate::app::{
    systems_alchemist_processor::{shared_formula_dir_from_snapshot, FormulaCatalog},
    AppNode,
};

mod anode;
mod construction;
mod external_files;
mod library;
mod properties;
mod reconcile;
mod snapshot;
mod value_bridge;

pub use anode::{AlchemistANode, AlchemistInputSocket, AlchemistOutputSocket};
pub use library::FormulaLibrary;
pub(crate) use construction::{
    anode_container_accepts_for_roles, anode_creatable_items_for_roles,
    create_anode_user_item, create_anode_user_item_tree, external_formula_tree_for_path,
};
pub(crate) use snapshot::{
    anode_from_snapshot, formula_from_snapshot, formula_from_snapshot_cached,
    local_signature_bindings, node_has_warning, node_warning_detail, node_warning_matches,
    ANodeMaterializationCache,
};
pub(crate) use value_bridge::{constraint_value_type, param_to_runtime_value, runtime_value_to_param};
pub(crate) use value_bridge::formula_runtime_param_change_requires_rematerialization;
pub(crate) use value_bridge::{
    constant_anode_for_value_param, is_constant_value_param,
    same_type_numeric_change_param, same_type_numeric_changes_for_param,
};
#[cfg(test)]
pub(crate) use library::reset_shared_formula_watcher_for_test;
pub use properties::{
    AlchemistPropertiesManager, AlchemistProperty, AlchemistPropertyFolder,
    AlchemistPropertyManager,
};
use anode::{
    input_socket_matches, input_socket_tree, output_socket_matches, output_socket_tree,
    value_type_parameter,
};
use properties::{PROPERTY_MANAGER_ROLES, formula_surface_from_snapshot, properties_tree, property_value_type};
use external_files::*;
use construction::*;
use snapshot::*;
use value_bridge::*;

pub(crate) const FORMULA_ITEM_KIND: &str = "alchemist_formula";
pub(crate) const FORMULA_FOLDER_ITEM_KIND: &str =
    "alchemist_formula_folder";
pub(crate) const FORMULA_FOLDER_NODE_TYPE: &str =
    "alchemist_formula_folder";
pub(crate) const FORMULA_EXTERNAL_FILE_CREATE_TYPE: &str =
    "alchemist_formula:external_file";
const FORMULA_LIBRARY_FILE_WATCH_RATE_HZ: u32 = 4;
pub(crate) const FORMULA_EXTERNAL_FILE_DECL_ID: &str =
    "external_formula_file";
pub(crate) const FORMULA_EXTERNAL_FILE_TAG: &str =
    "chataigne.formula.external.file";
pub(crate) const FORMULA_EXTERNAL_BUILTIN_TAG_PREFIX: &str =
    "chataigne.formula.external.builtin:";
pub(crate) const FORMULA_EXTERNAL_READ_ONLY_TAG: &str =
    "chataigne.formula.external.read_only";
pub(crate) const FORMULA_EXTERNAL_SOURCE_DECL_ID: &str =
    "external_formula_source";
pub(crate) const FORMULA_EXTERNAL_DELETE_FILE_DECL_ID: &str =
    "external_formula_delete_file";
pub(crate) const FORMULA_COPY_SOURCE_DECL_ID: &str = "formula_copy_source";
pub(crate) const ANODE_ITEM_KIND: &str = "alchemist_anode";
pub(crate) const CONNECTION_ITEM_KIND: &str = "alchemist_connection";
pub(crate) const ANODE_CREATE_PREFIX: &str = "alchemist_anode:";
pub(crate) const ANODE_MANAGED_VARIANT_SEPARATOR: &str = "@managed/";
pub(crate) const PROPERTY_ITEM_KIND: &str = "alchemist_property";
pub(crate) const PROPERTY_MANAGER_ITEM_KIND: &str =
    "alchemist_property_manager";
pub(crate) const PROPERTY_FOLDER_ITEM_KIND: &str = "alchemist_property_folder";
pub(crate) const PROPERTY_CREATE_PREFIX: &str = "alchemist_property:";
pub(crate) const PROPERTY_MANAGER_CREATE_PREFIX: &str =
    "alchemist_property_manager:";

pub(crate) const ANODE_NODE_TYPE: &str = "alchemist_anode";
const CONNECTION_NODE_TYPE: &str = "alchemist_connection";
pub(crate) const PROPERTY_MANAGER_NODE_TYPE: &str =
    "alchemist_property_manager";
pub(crate) const PROPERTY_FOLDER_NODE_TYPE: &str =
    "alchemist_property_folder";
pub(crate) const PROPERTY_NODE_TYPE: &str = "alchemist_property";
const PROPERTY_ANODE_TYPE: &str = "property";

pub(crate) const PROPERTIES_DECL_ID: &str = "properties";
pub(crate) const FORMULA_MANAGED_REGIONS_JSON_DECL_ID: &str =
    "managed_regions_json";
const ANODE_TYPE_TAG_PREFIX: &str = "alchemist.anode.type:";
const PROPERTY_TYPE_TAG_PREFIX: &str = "alchemist.property.type:";
const PROPERTY_MANAGER_ROLE_TAG_PREFIX: &str = "alchemist.manager.role:";
const ANODE_POSITION_DECL_ID: &str = "position";
const ANODE_SIZE_DECL_ID: &str = "size";
pub(crate) const FORMULA_WARNING_ID: &str = "alchemist_formula";
const FORMULA_EXTERNAL_FILE_WARNING_ID: &str =
    "alchemist_formula_external_file";
const ANODE_FORMULA_DIAGNOSTIC_WARNING_ID: &str =
    "alchemist_formula_diagnostic";

#[node("alchemist_connection", label = "Connection")]
#[children(
    source_node: NodeReference (
        label = "Source",
        reference_target_kind = ReferenceTargetKind::AnyNode,
        reference_allowed_node_types = vec![ANODE_NODE_TYPE.to_owned()],
        reference_allow_projections = false
    );
    source_socket: String = String::new() (label = "Source Socket");
    target_node: NodeReference (
        label = "Target",
        reference_target_kind = ReferenceTargetKind::AnyNode,
        reference_allowed_node_types = vec![ANODE_NODE_TYPE.to_owned()],
        reference_allow_projections = false
    );
    target_socket: String = String::new() (label = "Target Socket");
)]
pub struct AlchemistConnection {}

#[node("alchemist_connection", from_struct)]
impl Node for AlchemistConnection {
    fn user_item_kind(&self) -> &str {
        CONNECTION_ITEM_KIND
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
        self.node_data_mut().meta.can_be_disabled = false;
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("alchemist_formula", label = "Formula")]
#[children(
    is_valid: bool = false (label = "Valid", read_only = true, show_in_inspector_content = false);
    diagnostics_json: String = String::from("[]") (
        label = "Diagnostics",
        read_only = true,
        show_in_inspector_content = false
    );
    managed_regions_json: String = String::new() (
        label = "Managed Regions Metadata",
        read_only = true,
        show_in_inspector_content = false
    );
)]
pub struct AlchemistFormulaDefinition {
    #[state(default = ANodeMaterializationCache::default())]
    anode_materialization: ANodeMaterializationCache,
    #[state(default = HashMap::new())]
    numeric_constant_value_params: HashMap<NodeId, NodeId>,
}

const FORMULA_BULK_INBOX_THRESHOLD: usize = 32;

fn formula_inbox_requires_bulk(events: &EventFrame) -> bool {
    events.len() >= FORMULA_BULK_INBOX_THRESHOLD
        || events
            .iter()
            .filter(|event| matches!(&event.kind, EventKind::ChildAdded { .. } | EventKind::ChildRemoved { .. }))
            .nth(1)
            .is_some()
}

impl AlchemistFormulaDefinition {
    fn index_constant_value_params(&mut self, snapshot: &ProcessTreeSnapshot) {
        self.numeric_constant_value_params.clear();
        for anode in snapshot.child_ids(self.id()) {
            if let Some(param) = constant_value_param_from_snapshot(snapshot, anode) {
                self.numeric_constant_value_params.insert(param, anode);
            }
        }
    }

    fn dispatch_bulk_inbox(&mut self, ctx: &mut ProcessCtx) {
        let numeric_value_changes = ctx
            .events
            .iter()
            .filter_map(|event| match &event.kind {
                EventKind::ParamChanged { param, .. } => {
                    Some((*param, same_type_numeric_change_param(event) == Some(*param)))
                }
                _ => None,
            })
            .fold(HashMap::<NodeId, bool>::new(), |mut changes, (param, numeric)| {
                *changes.entry(param).or_insert(true) &= numeric;
                changes
            });
        let param_changes = ctx
            .events
            .iter()
            .filter_map(|event| match &event.kind {
                EventKind::ParamChanged {
                    param, old_value, ..
                } => Some((*param, old_value.clone())),
                _ => None,
            })
            .collect::<Vec<_>>();
        let meta_changes = ctx
            .events
            .iter()
            .filter_map(|event| match &event.kind {
                EventKind::MetaChanged { node, patch } => Some((*node, patch.clone())),
                _ => None,
            })
            .collect::<Vec<_>>();
        let has_child_added = ctx
            .events
            .iter()
            .any(|event| matches!(&event.kind, EventKind::ChildAdded { .. }));
        let has_child_removed = ctx
            .events
            .iter()
            .any(|event| matches!(&event.kind, EventKind::ChildRemoved { .. }));

        let mut needs_reconcile = has_child_added || has_child_removed;
        let mut needs_property_getters = false;
        let mut needs_save = needs_reconcile;

        for (param, _old_value) in param_changes {
            if self.is_formula_copy_source_param(ctx, param) && self.copy_formula_from_source(ctx) {
                continue;
            }
            if self.is_external_formula_source_param(ctx, param)
                && self.write_external_formula_from_source(ctx)
            {
                continue;
            }
            if self.is_external_formula_delete_file_param(ctx, param)
                && self.delete_external_formula_file_if_requested(ctx)
            {
                continue;
            }
            if self.is_external_formula_file_param(ctx, param)
                && self.sync_external_formula_file(ctx)
            {
                continue;
            }
            if self.is_formula_internal_param(param) {
                continue;
            }
            if (self.numeric_constant_value_params.contains_key(&param)
                && numeric_value_changes.get(&param) == Some(&true))
                || constant_numeric_value_change_keeps_signature(ctx, param)
            {
                needs_save = true;
                continue;
            }
            if ctx
                .tree_snapshot()
                .is_some_and(|snapshot| is_anode_layout_node(snapshot, self.id(), param))
            {
                needs_save = true;
            } else {
                needs_reconcile = true;
                needs_save = true;
            }
        }

        let latest_formula_label = meta_changes
            .iter()
            .rev()
            .find(|(node, patch)| *node == self.id() && patch.label.is_some())
            .and_then(|(_, patch)| patch.label.clone());
        for (node, _) in &meta_changes {
            if ctx
                .tree_snapshot()
                .is_some_and(|snapshot| is_anode_layout_node(snapshot, self.id(), *node))
            {
                needs_save = true;
                continue;
            }
            if ctx.tree_snapshot().is_some_and(|snapshot| {
                snapshot
                    .node(*node)
                    .is_some_and(|node| node.node_type == PROPERTY_NODE_TYPE)
            }) {
                needs_property_getters = true;
            }
            needs_reconcile = true;
            needs_save = true;
        }

        if has_child_removed {
            self.remove_dangling_connections(ctx);
        }
        if needs_property_getters {
            self.sync_property_getters(ctx);
        }
        let trace = std::env::var_os("GOLDEN_PERF_TRACE").is_some();
        let sync_started = trace.then(std::time::Instant::now);
        let materialized_formula = needs_reconcile.then(|| self.sync_anode_sockets(ctx, None)).flatten();
        let sync_us = sync_started.map(|started| started.elapsed().as_micros()).unwrap_or(0);
        let validate_started = trace.then(std::time::Instant::now);
        if needs_reconcile {
            self.validate(ctx, materialized_formula);
        }
        if let Some(started) = validate_started {
            eprintln!(
                "[formula] bulk_inbox events={} reconcile={} sync_us={} validate_us={}",
                ctx.events.len(),
                needs_reconcile,
                sync_us,
                started.elapsed().as_micros()
            );
        }
        if has_child_added && self.is_read_only_external_formula() {
            self.enforce_external_formula_permissions(ctx);
            self.schedule_external_formula_permission_enforcement(ctx);
        }
        if !needs_save {
            return;
        }

        let shared_formula_rename = latest_formula_label
            .as_deref()
            .and_then(|label| self.rename_shared_formula_file_for_label(ctx, label));
        match shared_formula_rename {
            Some(SharedFormulaFileRename::Renamed(path)) => {
                self.save_external_formula_file_to_path(ctx, path.as_path());
            }
            Some(SharedFormulaFileRename::Blocked) => {}
            None => self.save_external_formula_file(ctx),
        }
    }
}

#[item("alchemist_formula", node = "alchemist_formula", from_struct)]
impl Node for AlchemistFormulaDefinition {
    fn inbox_requires_tree_snapshot(&self, events: &EventFrame) -> bool {
        self.is_external_file_formula()
            || events.is_empty()
            || !events.iter().all(|event| {
                same_type_numeric_change_param(event)
                    .is_some_and(|param| self.numeric_constant_value_params.contains_key(&param))
            })
    }

    fn init(&mut self, ctx: &mut ProcessCtx) {
        if self.is_read_only_external_formula() {
            self.node_data_mut().meta.user_permissions = NodeUserPermissions::none();
        } else {
            self.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
        }
        self.node_data_mut().meta.can_be_disabled = false;
        self.reconcile_properties(ctx);
    }

    fn on_node_ready(
        &mut self,
        ctx: &mut ProcessCtx,
        _context: NodeCreationContext,
    ) {
        self.anode_materialization.invalidate();
        if let Some(snapshot) = ctx.tree_snapshot() {
            self.index_constant_value_params(snapshot);
        }
        self.reconcile_external_formula_file_parameter(ctx);
        self.reconcile_external_formula_operation_parameters(ctx);
        self.reconcile_formula_copy_source_parameter(ctx);
        if self.copy_formula_from_source(ctx) {
            return;
        }
        if self.write_external_formula_from_source(ctx) {
            return;
        }
        if self.sync_external_formula_file(ctx) {
            return;
        }
        self.reconcile_properties(ctx);
        self.sync_property_getters(ctx);
        let materialized_formula = self.sync_anode_sockets(ctx, None);
        self.validate(ctx, materialized_formula);
        self.enforce_external_formula_permissions(ctx);
        self.schedule_external_formula_permission_enforcement(ctx);
    }

    fn on_inbox(&mut self, ctx: &mut ProcessCtx) {
        let formula_id = self.id();
        if let Some(snapshot) = ctx.tree_snapshot() {
            self.index_constant_value_params(snapshot);
            self.anode_materialization.observe_events(snapshot, formula_id, &ctx.events);
        } else {
            for event in &ctx.events {
                let Some(anode) = same_type_numeric_change_param(event)
                    .and_then(|param| self.numeric_constant_value_params.get(&param))
                else {
                    self.anode_materialization.invalidate();
                    break;
                };
                self.anode_materialization.mark_dirty(*anode);
            }
        }
        if !formula_inbox_requires_bulk(&ctx.events) {
            self.dispatch_inbox(ctx);
            return;
        }
        self.dispatch_bulk_inbox(ctx);
    }

    fn on_param_change(
        &mut self,
        ctx: &mut ProcessCtx,
        param: NodeId,
        _old_value: ParamValue,
    ) {
        if self.is_formula_copy_source_param(ctx, param)
            && self.copy_formula_from_source(ctx)
        {
            return;
        }
        if self.is_external_formula_source_param(ctx, param)
            && self.write_external_formula_from_source(ctx)
        {
            return;
        }
        if self.is_external_formula_delete_file_param(ctx, param)
            && self.delete_external_formula_file_if_requested(ctx)
        {
            return;
        }
        if self.is_external_formula_file_param(ctx, param)
            && self.sync_external_formula_file(ctx)
        {
            return;
        }
        if self.is_formula_internal_param(param) {
            return;
        }
        if (self.numeric_constant_value_params.contains_key(&param)
            && same_type_numeric_changes_for_param(&ctx.events, param))
            || constant_numeric_value_change_keeps_signature(ctx, param)
        {
            self.save_external_formula_file(ctx);
            return;
        }
        if ctx
            .tree_snapshot()
            .is_some_and(|snapshot| is_anode_layout_node(snapshot, self.id(), param))
        {
            self.save_external_formula_file(ctx);
            return;
        }
        let skip_anode = ctx.tree_snapshot().and_then(|snapshot| {
            let child = direct_child_under(snapshot, self.id(), param)?;
            snapshot
                .node(child)
                .is_some_and(|node| node.node_type == ANODE_NODE_TYPE)
                .then_some(child)
                .filter(|anode| {
                    !is_anode_type_variable_config_param(snapshot, *anode, param)
                })
        });
        let materialized_formula = self.sync_anode_sockets(ctx, skip_anode);
        self.validate(ctx, materialized_formula);
        self.save_external_formula_file(ctx);
    }

    fn on_child_added(
        &mut self,
        ctx: &mut ProcessCtx,
        _parent: NodeId,
        _child: NodeId,
    ) {
        let materialized_formula = self.sync_anode_sockets(ctx, None);
        self.validate(ctx, materialized_formula);
        if self.is_read_only_external_formula() {
            self.enforce_external_formula_subtree_permissions(ctx, _child);
            self.schedule_external_formula_permission_enforcement(ctx);
        }
        self.save_external_formula_file(ctx);
    }

    fn on_child_removed(
        &mut self,
        ctx: &mut ProcessCtx,
        _parent: NodeId,
        _child: NodeId,
    ) {
        self.remove_dangling_connections(ctx);
        let materialized_formula = self.sync_anode_sockets(ctx, None);
        self.validate(ctx, materialized_formula);
        self.save_external_formula_file(ctx);
    }

    fn on_meta_changed(
        &mut self,
        ctx: &mut ProcessCtx,
        node: NodeId,
        patch: NodeMetaPatch,
    ) {
        if ctx
            .tree_snapshot()
            .is_some_and(|snapshot| is_anode_layout_node(snapshot, self.id(), node))
        {
            self.save_external_formula_file(ctx);
            return;
        }
        let shared_formula_rename = if node == self.id() {
            patch
                .label
                .as_deref()
                .and_then(|label| self.rename_shared_formula_file_for_label(ctx, label))
        } else {
            None
        };
        if ctx.tree_snapshot().is_some_and(|snapshot| {
            snapshot
                .node(node)
                .is_some_and(|node| node.node_type == PROPERTY_NODE_TYPE)
        }) {
            self.sync_property_getters(ctx);
        }
        let materialized_formula = self.sync_anode_sockets(ctx, None);
        self.validate(ctx, materialized_formula);
        match shared_formula_rename {
            Some(SharedFormulaFileRename::Renamed(path)) => {
                self.save_external_formula_file_to_path(ctx, path.as_path());
            }
            Some(SharedFormulaFileRename::Blocked) => {}
            None => self.save_external_formula_file(ctx),
        }
    }

    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        8
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[
            ANODE_ITEM_KIND,
            CONNECTION_ITEM_KIND,
        ]))
    }

    fn user_container_accepts_item(
        &self,
        item_type: &str,
        item_kind: &str,
    ) -> bool {
        (item_kind == ANODE_ITEM_KIND
            && (item_type == ANODE_NODE_TYPE
                || item_type.starts_with(ANODE_CREATE_PREFIX)))
            || (item_type == CONNECTION_NODE_TYPE
                && item_kind == CONNECTION_ITEM_KIND)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        let mut items = anode_creatable_items_for_roles(&[]);
        items.push(
            UserCreatableItem::new(
                CONNECTION_NODE_TYPE,
                CONNECTION_ITEM_KIND,
                "Connection",
            )
            .with_menu_path(["Graph"]),
        );
        items
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        if node_type == CONNECTION_NODE_TYPE {
            return Some(Box::new(AlchemistConnection::new()));
        }
        create_anode_user_item(node_type)
    }

    fn create_user_item_tree(&self, node_type: &str) -> Option<NodeTree> {
        if node_type == CONNECTION_NODE_TYPE {
            return Some(NodeTree::new(AlchemistConnection::new()));
        }
        create_anode_user_item_tree(node_type)
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("alchemist_formula_folder", label = "Folder")]
pub struct AlchemistFormulaFolder {}

#[node("alchemist_formula_folder", from_struct)]
impl Node for AlchemistFormulaFolder {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(formula_container_rules())
    }

    fn user_container_accepts_item(
        &self,
        item_type: &str,
        item_kind: &str,
    ) -> bool {
        formula_container_accepts(item_type, item_kind)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        formula_container_creatable_items()
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        create_formula_container_item(node_type)
    }

    fn create_user_item_tree(&self, node_type: &str) -> Option<NodeTree> {
        create_formula_container_item_tree(node_type)
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
        self.node_data_mut().meta.can_be_disabled = false;
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[cfg(test)]
mod tests;
