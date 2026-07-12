#[cfg(test)]
mod spatializer_tests;

use std::collections::{HashMap, HashSet};

use golden_core::{
    edit::NodeTree,
    events::{CustomEvent, Event, EventFrame, EventKind},
    node,
    node::{
        DeclId, Folder, Node, NodeHandle, NodeId, NodeMetaPatch, NodeUuid, UserContainerRules,
        UserCreatableItem,
    },
    parameter::{
        Enum, ParamValue, Parameter, ParameterChangeCheck, ParameterEnumOption, RangeConstraint,
    },
    process_ctx::{ProcessCtx, ProcessTreeNodeSnapshot, ProcessTreeSnapshot},
};

const SPATIALIZER_SOURCE_ITEM_KIND: &str = "spatializer_source";
const SPATIALIZER_SOURCE_ITEM_LABEL: &str = "Source";
const SPATIALIZER_TARGET_ITEM_KIND: &str = "spatializer_target";
const SPATIALIZER_TARGET_ITEM_LABEL: &str = "Target";

const SPATIALIZER_DIMENSION_2D: &str = "2d";
const SPATIALIZER_DIMENSION_3D: &str = "3d";
const SPATIALIZER_MODE_VORONOI: &str = "voronoi";
const SPATIALIZER_MODE_TARGET_RADIUS: &str = "targetRadius";
const SPATIALIZER_MODE_SOURCE_RADIUS: &str = "sourceRadius";
const SPATIALIZER_MODE_OVERLAP: &str = "overlap";
const SPATIALIZER_VALUE_LAYOUT_SOURCE_CENTRIC: &str = "sourceCentric";
const SPATIALIZER_VALUE_LAYOUT_TARGET_CENTRIC: &str = "targetCentric";

const SPATIALIZER_SOURCE_ENABLEMENT_CHANGED_EVENT: &str =
    "spatializer.sources.itemEnablementChanged";
const SPATIALIZER_TARGET_ENABLEMENT_CHANGED_EVENT: &str =
    "spatializer.targets.itemEnablementChanged";

const POSITION_2D_DECL_ID: &str = "position_2d";
const POSITION_3D_DECL_ID: &str = "position_3d";
const RADIUS_DECL_ID: &str = "radius";
const FREEZE_RADIUS_DECL_ID: &str = "freeze_radius";
const VALUE_TARGET_DECL_PREFIX: &str = "spatializer_target";
const VALUE_SOURCE_DECL_PREFIX: &str = "spatializer_source";
const VORONOI_TIE_EPSILON: f64 = 1.0e-9;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SpatializerDimension {
    Two,
    Three,
}

impl SpatializerDimension {
    fn position_decl_id(self) -> &'static str {
        match self {
            Self::Two => POSITION_2D_DECL_ID,
            Self::Three => POSITION_3D_DECL_ID,
        }
    }

    fn stale_position_decl_id(self) -> &'static str {
        match self {
            Self::Two => POSITION_3D_DECL_ID,
            Self::Three => POSITION_2D_DECL_ID,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SpatializerMode {
    Voronoi,
    TargetRadius,
    SourceRadius,
    Overlap,
}

impl SpatializerMode {
    fn source_radius_required(self) -> bool {
        matches!(self, Self::SourceRadius | Self::Overlap)
    }

    fn target_radius_required(self) -> bool {
        matches!(self, Self::TargetRadius | Self::Overlap)
    }

    fn target_freeze_radius_required(self) -> bool {
        matches!(self, Self::Voronoi)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SpatializerValueLayout {
    SourceCentric,
    TargetCentric,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct SpatialPoint {
    x: f64,
    y: f64,
    z: f64,
}

impl SpatialPoint {
    fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    fn distance_to(self, other: Self, dimension: SpatializerDimension) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = if dimension == SpatializerDimension::Three {
            self.z - other.z
        } else {
            0.0
        };
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

#[derive(Clone, Debug)]
struct SpatializerEndpointConfig {
    item_id: NodeId,
    item_uuid: NodeUuid,
    value_decl_id: String,
    label: String,
    enabled: bool,
    position: SpatialPoint,
    radius: f64,
    freeze_radius: f64,
}

#[derive(Clone, Debug)]
struct SpatializerConfig {
    dimension: SpatializerDimension,
    mode: SpatializerMode,
    value_layout: SpatializerValueLayout,
    sources: Vec<SpatializerEndpointConfig>,
    targets: Vec<SpatializerEndpointConfig>,
}

struct FloatChildSpec {
    decl_id: &'static str,
    default_value: f64,
    parameter: fn(f64) -> Parameter,
}

struct SpatializerValueMatrixState<'a> {
    folders_by_outer: &'a mut HashMap<NodeId, NodeId>,
    outputs_by_pair: &'a mut HashMap<(NodeId, NodeId), NodeId>,
    output_nodes: &'a mut HashSet<NodeId>,
    pending_folders: &'a mut HashSet<NodeId>,
    pending_pairs: &'a mut HashSet<(NodeId, NodeId)>,
}

struct SpatializerValuePairState<'a> {
    values_by_target: &'a HashMap<NodeId, HashMap<NodeId, f64>>,
    outputs_by_pair: &'a HashMap<(NodeId, NodeId), NodeId>,
    next_outputs_by_pair: &'a mut HashMap<(NodeId, NodeId), NodeId>,
    next_output_nodes: &'a mut HashSet<NodeId>,
    pending_pairs: &'a mut HashSet<(NodeId, NodeId)>,
    active_pairs: &'a mut HashSet<(NodeId, NodeId)>,
}

#[node("spatializer_module", label = "Spatializer")]
#[children(
    folder(connection) {
        [base_children];
    }
    folder(parameters) {
        dimensions: Enum = SPATIALIZER_DIMENSION_2D (
            label = "Dimensions",
            description = "Number of spatial dimensions used to compare source and target positions.",
            enum_options = spatializer_dimension_options()
        );
        mode: Enum = SPATIALIZER_MODE_VORONOI (
            label = "Mode",
            description = "Influence model used to compute the source by target value matrix.",
            enum_options = spatializer_mode_options()
        );
        value_layout: Enum = SPATIALIZER_VALUE_LAYOUT_SOURCE_CENTRIC (
            label = "Value Layout",
            description = "Tree orientation used by the generated source by target value matrix.",
            enum_options = spatializer_value_layout_options()
        );
        node sources: SpatializerSourceList = SpatializerSourceList::new() (
            label = "Sources",
            description = "Create all source points managed by this module."
        );
        node targets: SpatializerTargetList = SpatializerTargetList::new() (
            label = "Targets",
            description = "Create all target points evaluated by this module."
        );
        [base_children];
    }
    folder(values) {
        [base_children];
    }
)]
pub struct SpatializerModule {
    base: crate::app::ModuleBase,
    value_folders_by_outer: HashMap<NodeId, NodeId>,
    value_outputs_by_pair: HashMap<(NodeId, NodeId), NodeId>,
    value_output_nodes: HashSet<NodeId>,
    pending_endpoint_positions: HashSet<NodeId>,
    pending_endpoint_radii: HashSet<NodeId>,
    pending_endpoint_freeze_radii: HashSet<NodeId>,
    pending_value_folders: HashSet<NodeId>,
    pending_value_pairs: HashSet<(NodeId, NodeId)>,
    synced_value_layout: Option<SpatializerValueLayout>,
    config_dirty: bool,
}

impl SpatializerModule {
    pub fn create() -> Self {
        Self::new(
            crate::app::ModuleBase::new(),
            HashMap::new(),
            HashMap::new(),
            HashSet::new(),
            HashSet::new(),
            HashSet::new(),
            HashSet::new(),
            HashSet::new(),
            HashSet::new(),
            None,
            true,
        )
    }

    fn sync_configuration(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        let dimension = self.current_dimension(snapshot);
        let mode = self.current_mode(snapshot);
        let value_layout = self.current_value_layout(snapshot);
        if self
            .synced_value_layout
            .is_some_and(|synced_layout| synced_layout != value_layout)
        {
            self.clear_generated_value_tracking();
        }
        let deduped_parameters = self.dedupe_declared_parameter_children(ctx, snapshot);
        let structure_dirty = self.sync_endpoint_items(ctx, snapshot, dimension, mode);

        let config = self.collect_config(snapshot, dimension, mode, value_layout);
        let Some(values_root) = self.values_root() else {
            self.clear_value_tracking();
            self.config_dirty = deduped_parameters || structure_dirty;
            return;
        };

        let mut value_state = SpatializerValueMatrixState {
            folders_by_outer: &mut self.value_folders_by_outer,
            outputs_by_pair: &mut self.value_outputs_by_pair,
            output_nodes: &mut self.value_output_nodes,
            pending_folders: &mut self.pending_value_folders,
            pending_pairs: &mut self.pending_value_pairs,
        };
        let waiting_for_values = sync_value_matrix(
            ctx,
            snapshot,
            values_root,
            &config,
            &mut value_state,
        );
        self.synced_value_layout = Some(value_layout);
        self.config_dirty = deduped_parameters || structure_dirty || waiting_for_values;
    }

    fn dedupe_declared_parameter_children(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
    ) -> bool {
        let Some(parameters_id) = self.base.parameters_id() else {
            return false;
        };

        let mut changed = false;
        for decl_tail in ["dimensions", "mode", "value_layout", "sources", "targets"] {
            let child_ids = direct_child_ids_by_decl_tail(snapshot, parameters_id, decl_tail);
            for duplicate_id in child_ids.into_iter().skip(1) {
                NodeHandle::new(duplicate_id).remove(ctx);
                changed = true;
            }
        }
        changed
    }

    fn sync_endpoint_items(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        dimension: SpatializerDimension,
        mode: SpatializerMode,
    ) -> bool {
        let mut changed = false;
        let mut active_items = HashSet::new();
        if let Some(source_list) = self.source_list_id(snapshot) {
            for source_id in snapshot.child_ids(source_list) {
                if spatializer_source_item(snapshot, source_id).is_none() {
                    continue;
                }
                active_items.insert(source_id);
                changed |= sync_position_child(
                    ctx,
                    snapshot,
                    source_id,
                    dimension,
                    &mut self.pending_endpoint_positions,
                );
                changed |= sync_radius_child(
                    ctx,
                    snapshot,
                    source_id,
                    mode.source_radius_required(),
                    &mut self.pending_endpoint_radii,
                );
            }
        }
        if let Some(target_list) = self.target_list_id(snapshot) {
            for target_id in snapshot.child_ids(target_list) {
                if spatializer_target_item(snapshot, target_id).is_none() {
                    continue;
                }
                active_items.insert(target_id);
                changed |= sync_position_child(
                    ctx,
                    snapshot,
                    target_id,
                    dimension,
                    &mut self.pending_endpoint_positions,
                );
                changed |= sync_radius_child(
                    ctx,
                    snapshot,
                    target_id,
                    mode.target_radius_required(),
                    &mut self.pending_endpoint_radii,
                );
                changed |= sync_freeze_radius_child(
                    ctx,
                    snapshot,
                    target_id,
                    mode.target_freeze_radius_required(),
                    &mut self.pending_endpoint_freeze_radii,
                );
            }
        }
        self.pending_endpoint_positions
            .retain(|item_id| active_items.contains(item_id));
        self.pending_endpoint_radii
            .retain(|item_id| active_items.contains(item_id));
        self.pending_endpoint_freeze_radii
            .retain(|item_id| active_items.contains(item_id));
        changed
    }

    fn collect_config(
        &self,
        snapshot: &ProcessTreeSnapshot,
        dimension: SpatializerDimension,
        mode: SpatializerMode,
        value_layout: SpatializerValueLayout,
    ) -> SpatializerConfig {
        SpatializerConfig {
            dimension,
            mode,
            value_layout,
            sources: self.collect_sources(snapshot, dimension),
            targets: self.collect_targets(snapshot, dimension),
        }
    }

    fn collect_sources(
        &self,
        snapshot: &ProcessTreeSnapshot,
        dimension: SpatializerDimension,
    ) -> Vec<SpatializerEndpointConfig> {
        let Some(list_id) = self.source_list_id(snapshot) else {
            return Vec::new();
        };

        snapshot
            .child_ids(list_id)
            .into_iter()
            .filter_map(|item_id| {
                spatializer_source_config(snapshot, item_id, dimension, VALUE_SOURCE_DECL_PREFIX)
            })
            .collect()
    }

    fn collect_targets(
        &self,
        snapshot: &ProcessTreeSnapshot,
        dimension: SpatializerDimension,
    ) -> Vec<SpatializerEndpointConfig> {
        let Some(list_id) = self.target_list_id(snapshot) else {
            return Vec::new();
        };

        snapshot
            .child_ids(list_id)
            .into_iter()
            .filter_map(|item_id| {
                spatializer_target_config(snapshot, item_id, dimension, VALUE_TARGET_DECL_PREFIX)
            })
            .collect()
    }

    fn source_list_id(&self, snapshot: &ProcessTreeSnapshot) -> Option<NodeId> {
        let parameters_id = self.base.parameters_id()?;
        snapshot.find_child(parameters_id, "sources")
    }

    fn target_list_id(&self, snapshot: &ProcessTreeSnapshot) -> Option<NodeId> {
        let parameters_id = self.base.parameters_id()?;
        snapshot.find_child(parameters_id, "targets")
    }

    fn values_root(&self) -> Option<NodeId> {
        self.base.values_id()
    }

    fn current_dimension(&self, snapshot: &ProcessTreeSnapshot) -> SpatializerDimension {
        let value = self
            .base
            .parameters_id()
            .and_then(|parameters_id| child_value(snapshot, parameters_id, "dimensions"))
            .and_then(|value| value.as_enum())
            .unwrap_or_else(|| self.dimensions.get_ref().as_str().to_string());
        parse_spatializer_dimension(value.as_str())
    }

    fn current_mode(&self, snapshot: &ProcessTreeSnapshot) -> SpatializerMode {
        let value = self
            .base
            .parameters_id()
            .and_then(|parameters_id| child_value(snapshot, parameters_id, "mode"))
            .and_then(|value| value.as_enum())
            .unwrap_or_else(|| self.mode.get_ref().as_str().to_string());
        parse_spatializer_mode(value.as_str())
    }

    fn current_value_layout(&self, snapshot: &ProcessTreeSnapshot) -> SpatializerValueLayout {
        let value = self
            .base
            .parameters_id()
            .and_then(|parameters_id| child_value(snapshot, parameters_id, "value_layout"))
            .and_then(|value| value.as_enum())
            .unwrap_or_else(|| self.value_layout.get_ref().as_str().to_string());
        parse_spatializer_value_layout(value.as_str())
    }

    fn ensure_default_endpoint_items(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
    ) -> bool {
        let mut changed = false;
        if let Some(source_list) = self.source_list_id(snapshot) {
            if snapshot.child_ids(source_list).is_empty() {
                ctx.add_user_item_boxed(source_list, Box::new(SpatializerSourceItem::new()), None);
                changed = true;
            }
        }
        if let Some(target_list) = self.target_list_id(snapshot) {
            if snapshot.child_ids(target_list).is_empty() {
                ctx.add_user_item_boxed(target_list, Box::new(SpatializerTargetItem::new()), None);
                changed = true;
            }
        }
        changed
    }

    fn clear_value_tracking(&mut self) {
        self.clear_generated_value_tracking();
        self.pending_endpoint_positions.clear();
        self.pending_endpoint_radii.clear();
        self.pending_endpoint_freeze_radii.clear();
    }

    fn clear_generated_value_tracking(&mut self) {
        self.value_folders_by_outer.clear();
        self.value_outputs_by_pair.clear();
        self.value_output_nodes.clear();
        self.pending_value_folders.clear();
        self.pending_value_pairs.clear();
        self.synced_value_layout = None;
    }

    fn is_value_output(&self, node_id: NodeId) -> bool {
        self.value_output_nodes.contains(&node_id)
    }

    fn handle_endpoint_enablement_event(&mut self, ctx: &mut ProcessCtx, event: &CustomEvent) {
        if event.topic != SPATIALIZER_SOURCE_ENABLEMENT_CHANGED_EVENT
            && event.topic != SPATIALIZER_TARGET_ENABLEMENT_CHANGED_EVENT
        {
            return;
        }
        let Some(origin) = event.origin else {
            return;
        };
        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            self.config_dirty = true;
            return;
        };
        let snapshot = snapshot_arc.as_ref();
        let under_sources = self
            .source_list_id(snapshot)
            .is_some_and(|list| node_is_descendant_or_self(snapshot, origin, list));
        let under_targets = self
            .target_list_id(snapshot)
            .is_some_and(|list| node_is_descendant_or_self(snapshot, origin, list));
        if !under_sources && !under_targets {
            return;
        }
        self.config_dirty = true;
        self.sync_configuration(ctx, snapshot);
    }
}

#[golden_core::item(
    "module",
    node = "spatializer_module",
    via = base,
    from_struct,
    menu_path = ["Generators"]
)]
impl Node for SpatializerModule {
    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        match event.kind {
            EventKind::ParamChanged { .. }
            | EventKind::ChildAdded { .. }
            | EventKind::ChildRemoved { .. }
            | EventKind::ChildReplaced { .. }
            | EventKind::MetaChanged { .. }
            | EventKind::Custom(_) => u32::MAX,
            _ => 1,
        }
    }

    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);
        self.base
            .set_data_capabilities(ctx, crate::app::module::ModuleDataCapabilities::new(true, false));
        self.base.set_connected(ctx, true);
        crate::app::module::enable_module_authoring(self.node_data_mut());
        if let Some(snapshot_arc) = ctx.tree_snapshot_arc() {
            let snapshot = snapshot_arc.as_ref();
            if self.ensure_default_endpoint_items(ctx, snapshot) {
                self.config_dirty = true;
                return;
            }
            self.sync_configuration(ctx, snapshot);
        }
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        if self.config_dirty {
            if let Some(snapshot_arc) = ctx.tree_snapshot_arc() {
                self.sync_configuration(ctx, snapshot_arc.as_ref());
            }
        }
    }

    fn needs_update(&self) -> bool {
        self.config_dirty
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        self.config_dirty
    }

    fn inbox_requires_tree_snapshot(&self, events: &EventFrame) -> bool {
        events.iter().any(|event| match &event.kind {
            EventKind::ParamChanged { param, .. } => !self.is_value_output(*param),
            EventKind::ChildAdded { .. }
            | EventKind::ChildRemoved { .. }
            | EventKind::ChildReplaced { .. }
            | EventKind::Custom(_) => true,
            EventKind::MetaChanged { patch, .. } => patch.label.is_some(),
            _ => false,
        })
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, old_value: ParamValue) {
        if self.is_value_output(param) {
            return;
        }
        self.config_dirty = true;
        if let Some(snapshot_arc) = ctx.tree_snapshot_arc() {
            let snapshot = snapshot_arc.as_ref();
            let needs_followup_sync =
                node_decl_tail_matches(snapshot, param, "dimensions")
                    || node_decl_tail_matches(snapshot, param, "mode")
                    || node_decl_tail_matches(snapshot, param, "value_layout");
            self.sync_configuration(ctx, snapshot);
            self.base
                .emit_script_param_callback(ctx, snapshot, param, &old_value);
            if needs_followup_sync {
                self.config_dirty = true;
            }
        }
    }

    fn on_meta_changed(&mut self, ctx: &mut ProcessCtx, node: NodeId, patch: NodeMetaPatch) {
        if patch.label.is_none() {
            return;
        }

        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            self.config_dirty = true;
            return;
        };
        let snapshot = snapshot_arc.as_ref();
        let under_sources = self
            .source_list_id(snapshot)
            .is_some_and(|list| node_is_descendant_or_self(snapshot, node, list));
        let under_targets = self
            .target_list_id(snapshot)
            .is_some_and(|list| node_is_descendant_or_self(snapshot, node, list));
        if !under_sources && !under_targets {
            return;
        }

        self.config_dirty = true;
        self.sync_configuration(ctx, snapshot);
        self.config_dirty = true;
    }

    fn on_child_added(&mut self, ctx: &mut ProcessCtx, _parent: NodeId, _child: NodeId) {
        self.config_dirty = true;
        if let Some(snapshot_arc) = ctx.tree_snapshot_arc() {
            self.sync_configuration(ctx, snapshot_arc.as_ref());
        }
    }

    fn on_child_removed(&mut self, ctx: &mut ProcessCtx, _parent: NodeId, child: NodeId) {
        self.config_dirty = true;
        self.value_folders_by_outer.retain(|_, value| *value != child);
        self.value_outputs_by_pair.retain(|_, value| *value != child);
        self.value_output_nodes.remove(&child);
        self.pending_value_folders.remove(&child);
        self.pending_value_pairs
            .retain(|(target, source)| *target != child && *source != child);
        if let Some(snapshot_arc) = ctx.tree_snapshot_arc() {
            self.sync_configuration(ctx, snapshot_arc.as_ref());
        }
    }

    fn on_effective_enabled_changed(&mut self, _ctx: &mut ProcessCtx, _enabled: bool) {
        self.config_dirty = true;
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        self.handle_endpoint_enablement_event(ctx, &event);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

#[node("spatializer_source_list", label = "Sources")]
pub struct SpatializerSourceList {}

#[node("spatializer_source_list", from_struct)]
impl Node for SpatializerSourceList {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_manager_authoring(self.node_data_mut());
        self.node_data_mut().meta.can_be_disabled = false;
    }

    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[SPATIALIZER_SOURCE_ITEM_KIND]))
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![
            UserCreatableItem::new(
                SpatializerSourceItem::NODE_TYPE,
                SPATIALIZER_SOURCE_ITEM_KIND,
                SPATIALIZER_SOURCE_ITEM_LABEL,
            )
            .with_select_when_created(false),
        ]
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        (node_type == SpatializerSourceItem::NODE_TYPE || node_type == SPATIALIZER_SOURCE_ITEM_KIND)
            .then(|| Box::new(SpatializerSourceItem::new()) as Box<dyn Node>)
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("spatializer_target_list", label = "Targets")]
pub struct SpatializerTargetList {}

#[node("spatializer_target_list", from_struct)]
impl Node for SpatializerTargetList {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_manager_authoring(self.node_data_mut());
        self.node_data_mut().meta.can_be_disabled = false;
    }

    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[SPATIALIZER_TARGET_ITEM_KIND]))
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![
            UserCreatableItem::new(
                SpatializerTargetItem::NODE_TYPE,
                SPATIALIZER_TARGET_ITEM_KIND,
                SPATIALIZER_TARGET_ITEM_LABEL,
            )
            .with_select_when_created(false),
        ]
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        (node_type == SpatializerTargetItem::NODE_TYPE || node_type == SPATIALIZER_TARGET_ITEM_KIND)
            .then(|| Box::new(SpatializerTargetItem::new()) as Box<dyn Node>)
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("spatializer_source", label = "Source")]
pub struct SpatializerSourceItem {}

#[node("spatializer_source", from_struct)]
impl Node for SpatializerSourceItem {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }

    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, _enabled: bool) {
        ctx.emit_custom_event(CustomEvent::new(
            SPATIALIZER_SOURCE_ENABLEMENT_CHANGED_EVENT,
            Some(self.id()),
            serde_json::Value::Null,
        ));
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("spatializer_target", label = "Target")]
pub struct SpatializerTargetItem {}

#[node("spatializer_target", from_struct)]
impl Node for SpatializerTargetItem {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }

    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, _enabled: bool) {
        ctx.emit_custom_event(CustomEvent::new(
            SPATIALIZER_TARGET_ENABLEMENT_CHANGED_EVENT,
            Some(self.id()),
            serde_json::Value::Null,
        ));
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

fn spatializer_dimension_options() -> Vec<ParameterEnumOption> {
    vec![
        enum_option(SPATIALIZER_DIMENSION_2D, "2D", 0),
        enum_option(SPATIALIZER_DIMENSION_3D, "3D", 1),
    ]
}

fn spatializer_mode_options() -> Vec<ParameterEnumOption> {
    vec![
        enum_option(SPATIALIZER_MODE_VORONOI, "Voronoi", 0),
        enum_option(SPATIALIZER_MODE_TARGET_RADIUS, "Target Radius", 1),
        enum_option(SPATIALIZER_MODE_SOURCE_RADIUS, "Source Radius", 2),
        enum_option(SPATIALIZER_MODE_OVERLAP, "Overlap", 3),
    ]
}

fn spatializer_value_layout_options() -> Vec<ParameterEnumOption> {
    vec![
        enum_option(
            SPATIALIZER_VALUE_LAYOUT_SOURCE_CENTRIC,
            "Source Centric",
            0,
        ),
        enum_option(
            SPATIALIZER_VALUE_LAYOUT_TARGET_CENTRIC,
            "Target Centric",
            1,
        ),
    ]
}

fn enum_option(variant_id: &str, label: &str, ordering: i32) -> ParameterEnumOption {
    ParameterEnumOption {
        variant_id: variant_id.to_string(),
        value: ParamValue::Enum(variant_id.to_string()),
        label: label.to_string(),
        tags: Vec::new(),
        ordering: Some(ordering),
    }
}

fn parse_spatializer_dimension(value: &str) -> SpatializerDimension {
    match value {
        SPATIALIZER_DIMENSION_3D => SpatializerDimension::Three,
        _ => SpatializerDimension::Two,
    }
}

fn parse_spatializer_mode(value: &str) -> SpatializerMode {
    match value {
        SPATIALIZER_MODE_TARGET_RADIUS => SpatializerMode::TargetRadius,
        SPATIALIZER_MODE_SOURCE_RADIUS => SpatializerMode::SourceRadius,
        SPATIALIZER_MODE_OVERLAP => SpatializerMode::Overlap,
        _ => SpatializerMode::Voronoi,
    }
}

fn parse_spatializer_value_layout(value: &str) -> SpatializerValueLayout {
    let normalized = value
        .trim()
        .chars()
        .filter(|ch| !matches!(ch, ' ' | '_' | '-'))
        .flat_map(char::to_lowercase)
        .collect::<String>();
    match normalized.as_str() {
        "targetcentric" => SpatializerValueLayout::TargetCentric,
        _ => SpatializerValueLayout::SourceCentric,
    }
}

fn spatializer_source_item(
    snapshot: &ProcessTreeSnapshot,
    item_id: NodeId,
) -> Option<&ProcessTreeNodeSnapshot> {
    let item = snapshot.node(item_id)?;
    (item.node_type == SpatializerSourceItem::NODE_TYPE).then_some(item)
}

fn spatializer_target_item(
    snapshot: &ProcessTreeSnapshot,
    item_id: NodeId,
) -> Option<&ProcessTreeNodeSnapshot> {
    let item = snapshot.node(item_id)?;
    (item.node_type == SpatializerTargetItem::NODE_TYPE).then_some(item)
}

fn spatializer_source_config(
    snapshot: &ProcessTreeSnapshot,
    item_id: NodeId,
    dimension: SpatializerDimension,
    value_prefix: &str,
) -> Option<SpatializerEndpointConfig> {
    spatializer_endpoint_config(
        spatializer_source_item(snapshot, item_id)?,
        snapshot,
        item_id,
        dimension,
        value_prefix,
    )
}

fn spatializer_target_config(
    snapshot: &ProcessTreeSnapshot,
    item_id: NodeId,
    dimension: SpatializerDimension,
    value_prefix: &str,
) -> Option<SpatializerEndpointConfig> {
    spatializer_endpoint_config(
        spatializer_target_item(snapshot, item_id)?,
        snapshot,
        item_id,
        dimension,
        value_prefix,
    )
}

fn spatializer_endpoint_config(
    item: &ProcessTreeNodeSnapshot,
    snapshot: &ProcessTreeSnapshot,
    item_id: NodeId,
    dimension: SpatializerDimension,
    value_prefix: &str,
) -> Option<SpatializerEndpointConfig> {
    Some(SpatializerEndpointConfig {
        item_id,
        item_uuid: item.uuid,
        value_decl_id: value_decl_id(value_prefix, item.uuid),
        label: item.label.clone(),
        enabled: item.enabled,
        position: child_position(snapshot, item_id, dimension, SpatialPoint::new(0.0, 0.0, 0.0)),
        radius: child_float(snapshot, item_id, RADIUS_DECL_ID, 1.0).max(0.0),
        freeze_radius: child_float(snapshot, item_id, FREEZE_RADIUS_DECL_ID, 0.0).max(0.0),
    })
}

fn sync_position_child(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    item_id: NodeId,
    dimension: SpatializerDimension,
    pending_positions: &mut HashSet<NodeId>,
) -> bool {
    let desired_decl = dimension.position_decl_id();
    let stale_decl = dimension.stale_position_decl_id();
    let desired_ids = direct_child_ids_by_decl(snapshot, item_id, desired_decl);
    let stale_ids = direct_child_ids_by_decl(snapshot, item_id, stale_decl);
    let desired_id = desired_ids.first().copied();
    let stale_id = stale_ids.first().copied();
    let position = desired_id
        .and_then(|node_id| child_param_value(snapshot, node_id))
        .and_then(param_value_to_point)
        .or_else(|| {
            stale_id
                .and_then(|node_id| child_param_value(snapshot, node_id))
                .and_then(param_value_to_point)
        })
        .unwrap_or_else(|| SpatialPoint::new(0.0, 0.0, 0.0));

    let mut changed = false;
    for stale_id in stale_ids {
        NodeHandle::new(stale_id).remove(ctx);
        changed = true;
    }
    for duplicate_id in desired_ids.into_iter().skip(1) {
        NodeHandle::new(duplicate_id).remove(ctx);
        changed = true;
    }

    match desired_id {
        Some(node_id) => {
            pending_positions.remove(&item_id);
            let needs_replace = snapshot
                .node(node_id)
                .and_then(|node| node.param_value.as_ref())
                .is_none_or(|value| !position_value_matches_dimension(value, dimension));
            if needs_replace {
                NodeHandle::new(node_id).replace_with(ctx, position_param(dimension, position));
                changed = true;
            }
        }
        None => {
            if pending_positions.insert(item_id) {
                ctx.add_child_tree(item_id, NodeTree::new(position_param(dimension, position)), None);
            }
            changed = true;
        }
    }

    changed
}

fn sync_radius_child(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    item_id: NodeId,
    required: bool,
    pending_radii: &mut HashSet<NodeId>,
) -> bool {
    sync_float_child(
        ctx,
        snapshot,
        item_id,
        required,
        FloatChildSpec {
            decl_id: RADIUS_DECL_ID,
            default_value: 1.0,
            parameter: radius_param,
        },
        pending_radii,
    )
}

fn sync_freeze_radius_child(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    item_id: NodeId,
    required: bool,
    pending_freeze_radii: &mut HashSet<NodeId>,
) -> bool {
    sync_float_child(
        ctx,
        snapshot,
        item_id,
        required,
        FloatChildSpec {
            decl_id: FREEZE_RADIUS_DECL_ID,
            default_value: 0.0,
            parameter: freeze_radius_param,
        },
        pending_freeze_radii,
    )
}

fn sync_float_child(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    item_id: NodeId,
    required: bool,
    spec: FloatChildSpec,
    pending: &mut HashSet<NodeId>,
) -> bool {
    let existing_ids = direct_child_ids_by_decl(snapshot, item_id, spec.decl_id);
    let existing_id = existing_ids.first().copied();
    let mut changed = false;

    for duplicate_id in existing_ids.into_iter().skip(1) {
        NodeHandle::new(duplicate_id).remove(ctx);
        changed = true;
    }

    if !required {
        pending.remove(&item_id);
        if let Some(existing_id) = existing_id {
            NodeHandle::new(existing_id).remove(ctx);
            changed = true;
        }
        return changed;
    }

    let radius = existing_id
        .and_then(|node_id| child_param_value(snapshot, node_id))
        .and_then(|value| value.as_float())
        .filter(|value| value.is_finite())
        .unwrap_or(spec.default_value)
        .max(0.0);

    match existing_id {
        Some(node_id) => {
            pending.remove(&item_id);
            let needs_replace = snapshot
                .node(node_id)
                .and_then(|node| node.param_value.as_ref())
                .is_none_or(|value| !matches!(value, ParamValue::Float(_)));
            if needs_replace {
                NodeHandle::new(node_id).replace_with(ctx, (spec.parameter)(radius));
                true
            } else {
                changed
            }
        }
        None => {
            if pending.insert(item_id) {
                ctx.add_child_tree(item_id, NodeTree::new((spec.parameter)(radius)), None);
            }
            true
        }
    }
}

fn sync_value_matrix(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    root_id: NodeId,
    config: &SpatializerConfig,
    state: &mut SpatializerValueMatrixState<'_>,
) -> bool {
    match config.value_layout {
        SpatializerValueLayout::SourceCentric => sync_source_centric_value_matrix(
            ctx, snapshot, root_id, config, state,
        ),
        SpatializerValueLayout::TargetCentric => sync_target_centric_value_matrix(
            ctx, snapshot, root_id, config, state,
        ),
    }
}

fn sync_target_centric_value_matrix(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    root_id: NodeId,
    config: &SpatializerConfig,
    state: &mut SpatializerValueMatrixState<'_>,
) -> bool {
    let value_folders_by_outer = &mut *state.folders_by_outer;
    let value_outputs_by_pair = &mut *state.outputs_by_pair;
    let value_output_nodes = &mut *state.output_nodes;
    let pending_value_folders = &mut *state.pending_folders;
    let pending_value_pairs = &mut *state.pending_pairs;
    let existing_by_decl = child_ids_by_decl(snapshot, root_id);
    let values_by_target = spatializer_values_by_target(config);
    let mut used_outer_folders = HashSet::new();
    let mut next_value_folders_by_outer = HashMap::new();
    let mut next_value_outputs_by_pair = HashMap::new();
    let mut next_value_output_nodes = HashSet::new();
    let mut active_outer_folders = HashSet::new();
    let mut active_pairs = HashSet::new();
    let mut waiting_for_values = false;

    for target in &config.targets {
        active_outer_folders.insert(target.item_id);
        let existing_folder_id = value_folders_by_outer
            .get(&target.item_id)
            .copied()
            .filter(|node_id| snapshot.node(*node_id).is_some())
            .or_else(|| existing_by_decl.get(target.value_decl_id.as_str()).copied());

        match existing_folder_id {
            Some(folder_id) => {
                pending_value_folders.remove(&target.item_id);
                used_outer_folders.insert(folder_id);
                next_value_folders_by_outer.insert(target.item_id, folder_id);
                if snapshot
                    .node(folder_id)
                    .is_some_and(|node| node.label != target.label)
                {
                    ctx.patch_node_meta(
                        folder_id,
                        NodeMetaPatch {
                            label: Some(target.label.clone()),
                            ..Default::default()
                        },
                    );
                }
                let mut pair_state = SpatializerValuePairState {
                    values_by_target: &values_by_target,
                    outputs_by_pair: value_outputs_by_pair,
                    next_outputs_by_pair: &mut next_value_outputs_by_pair,
                    next_output_nodes: &mut next_value_output_nodes,
                    pending_pairs: pending_value_pairs,
                    active_pairs: &mut active_pairs,
                };
                waiting_for_values |= sync_source_values_for_target(
                    ctx,
                    snapshot,
                    folder_id,
                    config,
                    target,
                    &mut pair_state,
                );
            }
            None => {
                waiting_for_values = true;
                if pending_value_folders.insert(target.item_id) {
                    ctx.add_child_tree(
                        root_id,
                        target_values_tree(config, target, &values_by_target),
                        None,
                    );
                }
                for source in &config.sources {
                    active_pairs.insert((target.item_id, source.item_id));
                }
            }
        }
    }

    for child_id in snapshot.child_ids(root_id) {
        if used_outer_folders.contains(&child_id) {
            continue;
        }
        let Some(child) = snapshot.node(child_id) else {
            continue;
        };
        if child.decl_id.starts_with(VALUE_TARGET_DECL_PREFIX)
            || child.decl_id.starts_with(VALUE_SOURCE_DECL_PREFIX)
        {
            NodeHandle::new(child_id).remove(ctx);
        }
    }

    pending_value_folders.retain(|outer_id| active_outer_folders.contains(outer_id));
    pending_value_pairs.retain(|pair| active_pairs.contains(pair));
    *value_folders_by_outer = next_value_folders_by_outer;
    *value_outputs_by_pair = next_value_outputs_by_pair;
    *value_output_nodes = next_value_output_nodes;
    waiting_for_values
}

fn sync_source_centric_value_matrix(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    root_id: NodeId,
    config: &SpatializerConfig,
    state: &mut SpatializerValueMatrixState<'_>,
) -> bool {
    let value_folders_by_outer = &mut *state.folders_by_outer;
    let value_outputs_by_pair = &mut *state.outputs_by_pair;
    let value_output_nodes = &mut *state.output_nodes;
    let pending_value_folders = &mut *state.pending_folders;
    let pending_value_pairs = &mut *state.pending_pairs;
    let existing_by_decl = child_ids_by_decl(snapshot, root_id);
    let values_by_target = spatializer_values_by_target(config);
    let mut used_outer_folders = HashSet::new();
    let mut next_value_folders_by_outer = HashMap::new();
    let mut next_value_outputs_by_pair = HashMap::new();
    let mut next_value_output_nodes = HashSet::new();
    let mut active_outer_folders = HashSet::new();
    let mut active_pairs = HashSet::new();
    let mut waiting_for_values = false;

    for source in &config.sources {
        active_outer_folders.insert(source.item_id);
        let existing_folder_id = value_folders_by_outer
            .get(&source.item_id)
            .copied()
            .filter(|node_id| snapshot.node(*node_id).is_some())
            .or_else(|| existing_by_decl.get(source.value_decl_id.as_str()).copied());

        match existing_folder_id {
            Some(folder_id) => {
                pending_value_folders.remove(&source.item_id);
                used_outer_folders.insert(folder_id);
                next_value_folders_by_outer.insert(source.item_id, folder_id);
                if snapshot
                    .node(folder_id)
                    .is_some_and(|node| node.label != source.label)
                {
                    ctx.patch_node_meta(
                        folder_id,
                        NodeMetaPatch {
                            label: Some(source.label.clone()),
                            ..Default::default()
                        },
                    );
                }
                let mut pair_state = SpatializerValuePairState {
                    values_by_target: &values_by_target,
                    outputs_by_pair: value_outputs_by_pair,
                    next_outputs_by_pair: &mut next_value_outputs_by_pair,
                    next_output_nodes: &mut next_value_output_nodes,
                    pending_pairs: pending_value_pairs,
                    active_pairs: &mut active_pairs,
                };
                waiting_for_values |= sync_target_values_for_source(
                    ctx,
                    snapshot,
                    folder_id,
                    config,
                    source,
                    &mut pair_state,
                );
            }
            None => {
                waiting_for_values = true;
                if pending_value_folders.insert(source.item_id) {
                    ctx.add_child_tree(
                        root_id,
                        source_values_tree(config, source, &values_by_target),
                        None,
                    );
                }
                for target in &config.targets {
                    active_pairs.insert((target.item_id, source.item_id));
                }
            }
        }
    }

    for child_id in snapshot.child_ids(root_id) {
        if used_outer_folders.contains(&child_id) {
            continue;
        }
        let Some(child) = snapshot.node(child_id) else {
            continue;
        };
        if child.decl_id.starts_with(VALUE_SOURCE_DECL_PREFIX)
            || child.decl_id.starts_with(VALUE_TARGET_DECL_PREFIX)
        {
            NodeHandle::new(child_id).remove(ctx);
        }
    }

    pending_value_folders.retain(|outer_id| active_outer_folders.contains(outer_id));
    pending_value_pairs.retain(|pair| active_pairs.contains(pair));
    *value_folders_by_outer = next_value_folders_by_outer;
    *value_outputs_by_pair = next_value_outputs_by_pair;
    *value_output_nodes = next_value_output_nodes;
    waiting_for_values
}

fn sync_source_values_for_target(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    folder_id: NodeId,
    config: &SpatializerConfig,
    target: &SpatializerEndpointConfig,
    state: &mut SpatializerValuePairState<'_>,
) -> bool {
    let existing_by_decl = child_ids_by_decl(snapshot, folder_id);
    let mut used_source_values = HashSet::new();
    let mut waiting_for_values = false;

    for source in &config.sources {
        let pair = (target.item_id, source.item_id);
        state.active_pairs.insert(pair);
        let value = value_for_pair(state.values_by_target, target.item_id, source.item_id);
        let existing_value_id = state
            .outputs_by_pair
            .get(&pair)
            .copied()
            .filter(|node_id| snapshot.node(*node_id).is_some())
            .or_else(|| existing_by_decl.get(source.value_decl_id.as_str()).copied());

        match existing_value_id {
            Some(value_id) => {
                state.pending_pairs.remove(&pair);
                used_source_values.insert(value_id);
                state.next_outputs_by_pair.insert(pair, value_id);
                state.next_output_nodes.insert(value_id);
                update_source_value(ctx, snapshot, value_id, target, source, value);
            }
            None => {
                waiting_for_values = true;
                if state.pending_pairs.insert(pair) {
                    ctx.add_child_tree(folder_id, source_value_tree(target, source, value), None);
                }
            }
        }
    }

    for child_id in snapshot.child_ids(folder_id) {
        if used_source_values.contains(&child_id) {
            continue;
        }
        let Some(child) = snapshot.node(child_id) else {
            continue;
        };
        if child.decl_id.starts_with(VALUE_SOURCE_DECL_PREFIX) {
            NodeHandle::new(child_id).remove(ctx);
        }
    }

    waiting_for_values
}

fn sync_target_values_for_source(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    folder_id: NodeId,
    config: &SpatializerConfig,
    source: &SpatializerEndpointConfig,
    state: &mut SpatializerValuePairState<'_>,
) -> bool {
    let existing_by_decl = child_ids_by_decl(snapshot, folder_id);
    let mut used_target_values = HashSet::new();
    let mut waiting_for_values = false;

    for target in &config.targets {
        let pair = (target.item_id, source.item_id);
        state.active_pairs.insert(pair);
        let value = value_for_pair(state.values_by_target, target.item_id, source.item_id);
        let existing_value_id = state
            .outputs_by_pair
            .get(&pair)
            .copied()
            .filter(|node_id| snapshot.node(*node_id).is_some())
            .or_else(|| existing_by_decl.get(target.value_decl_id.as_str()).copied());

        match existing_value_id {
            Some(value_id) => {
                state.pending_pairs.remove(&pair);
                used_target_values.insert(value_id);
                state.next_outputs_by_pair.insert(pair, value_id);
                state.next_output_nodes.insert(value_id);
                update_target_value(ctx, snapshot, value_id, source, target, value);
            }
            None => {
                waiting_for_values = true;
                if state.pending_pairs.insert(pair) {
                    ctx.add_child_tree(folder_id, target_value_tree(source, target, value), None);
                }
            }
        }
    }

    for child_id in snapshot.child_ids(folder_id) {
        if used_target_values.contains(&child_id) {
            continue;
        }
        let Some(child) = snapshot.node(child_id) else {
            continue;
        };
        if child.decl_id.starts_with(VALUE_TARGET_DECL_PREFIX) {
            NodeHandle::new(child_id).remove(ctx);
        }
    }

    waiting_for_values
}

fn update_source_value(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    value_id: NodeId,
    target: &SpatializerEndpointConfig,
    source: &SpatializerEndpointConfig,
    value: f64,
) {
    update_output_value(
        ctx,
        snapshot,
        value_id,
        source.label.as_str(),
        value,
        source_value_param(target, source, value),
    );
}

fn update_target_value(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    value_id: NodeId,
    source: &SpatializerEndpointConfig,
    target: &SpatializerEndpointConfig,
    value: f64,
) {
    update_output_value(
        ctx,
        snapshot,
        value_id,
        target.label.as_str(),
        value,
        target_value_param(source, target, value),
    );
}

fn update_output_value(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    value_id: NodeId,
    label: &str,
    value: f64,
    replacement: Parameter,
) {
    let Some(node) = snapshot.node(value_id) else {
        return;
    };
    if node.label != label {
        ctx.patch_node_meta(
            value_id,
            NodeMetaPatch {
                label: Some(label.to_string()),
                ..Default::default()
            },
        );
    }

    let constraints = unit_value_constraints();
    if !matches!(node.param_value.as_ref(), Some(ParamValue::Float(_)))
        || node.param_constraints.as_ref().map(|constraints| &constraints.range)
            != Some(&constraints.range)
    {
        NodeHandle::new(value_id).replace_with(ctx, replacement);
        return;
    }

    ctx.set_param(value_id, ParamValue::Float(value));
}

fn target_values_tree(
    config: &SpatializerConfig,
    target: &SpatializerEndpointConfig,
    values_by_target: &HashMap<NodeId, HashMap<NodeId, f64>>,
) -> NodeTree {
    let mut tree = NodeTree::new(value_folder(
        target.label.as_str(),
        target.value_decl_id.as_str(),
        target_folder_uuid(target.item_uuid),
    ));
    for source in &config.sources {
        tree.push_child(source_value_tree(
            target,
            source,
            value_for_pair(values_by_target, target.item_id, source.item_id),
        ));
    }
    tree
}

fn source_value_tree(
    target: &SpatializerEndpointConfig,
    source: &SpatializerEndpointConfig,
    value: f64,
) -> NodeTree {
    NodeTree::new(source_value_param(target, source, value))
}

fn source_values_tree(
    config: &SpatializerConfig,
    source: &SpatializerEndpointConfig,
    values_by_target: &HashMap<NodeId, HashMap<NodeId, f64>>,
) -> NodeTree {
    let mut tree = NodeTree::new(value_folder(
        source.label.as_str(),
        source.value_decl_id.as_str(),
        source_folder_uuid(source.item_uuid),
    ));
    for target in &config.targets {
        tree.push_child(target_value_tree(
            source,
            target,
            value_for_pair(values_by_target, target.item_id, source.item_id),
        ));
    }
    tree
}

fn target_value_tree(
    source: &SpatializerEndpointConfig,
    target: &SpatializerEndpointConfig,
    value: f64,
) -> NodeTree {
    NodeTree::new(target_value_param(source, target, value))
}

fn value_folder(label: &str, decl_id: &str, uuid: NodeUuid) -> Folder {
    let mut folder = Folder::new(label);
    crate::app::module::enable_module_authoring(folder.node_data_mut());
    let meta = &mut folder.node_data_mut().meta;
    meta.uuid = uuid;
    meta.decl_id = DeclId(decl_id.to_string());
    meta.short_name = decl_id.to_string();
    meta.user_permissions.can_edit_name = false;
    folder
}

fn source_value_param(
    target: &SpatializerEndpointConfig,
    source: &SpatializerEndpointConfig,
    value: f64,
) -> Parameter {
    let mut parameter = read_only_param(
        source.label.as_str(),
        source.value_decl_id.as_str(),
        source_value_uuid(target.item_uuid, source.item_uuid),
        ParamValue::Float(value),
    );
    parameter.constraints = unit_value_constraints();
    parameter
}

fn target_value_param(
    source: &SpatializerEndpointConfig,
    target: &SpatializerEndpointConfig,
    value: f64,
) -> Parameter {
    let mut parameter = read_only_param(
        target.label.as_str(),
        target.value_decl_id.as_str(),
        target_value_uuid(source.item_uuid, target.item_uuid),
        ParamValue::Float(value),
    );
    parameter.constraints = unit_value_constraints();
    parameter
}

fn read_only_param(label: &str, decl_id: &str, uuid: NodeUuid, value: ParamValue) -> Parameter {
    let mut parameter = Parameter::new(label, value, ParameterChangeCheck::ValueChange);
    parameter.read_only = true;
    crate::app::module::enable_module_authoring(parameter.node_data_mut());
    let meta = &mut parameter.node_data_mut().meta;
    meta.uuid = uuid;
    meta.decl_id = DeclId(decl_id.to_string());
    meta.short_name = decl_id.to_string();
    meta.user_permissions.can_edit_name = false;
    parameter
}

fn position_param(dimension: SpatializerDimension, position: SpatialPoint) -> Parameter {
    let value = match dimension {
        SpatializerDimension::Two => ParamValue::Vec2(position.x, position.y),
        SpatializerDimension::Three => ParamValue::Vec3(position.x, position.y, position.z),
    };
    let mut parameter = Parameter::new("Position", value, ParameterChangeCheck::ValueChange);
    crate::app::module::enable_module_authoring(parameter.node_data_mut());
    let meta = &mut parameter.node_data_mut().meta;
    meta.decl_id = DeclId(dimension.position_decl_id().to_string());
    meta.short_name = dimension.position_decl_id().to_string();
    parameter
}

fn radius_param(radius: f64) -> Parameter {
    let mut parameter = Parameter::new(
        "Radius",
        ParamValue::Float(radius.max(0.0)),
        ParameterChangeCheck::ValueChange,
    );
    parameter.constraints.range = RangeConstraint::uniform(Some(0.0), None);
    crate::app::module::enable_module_authoring(parameter.node_data_mut());
    let meta = &mut parameter.node_data_mut().meta;
    meta.decl_id = DeclId(RADIUS_DECL_ID.to_string());
    meta.short_name = RADIUS_DECL_ID.to_string();
    parameter
}

fn freeze_radius_param(radius: f64) -> Parameter {
    let mut parameter = Parameter::new(
        "Freeze Radius",
        ParamValue::Float(radius.max(0.0)),
        ParameterChangeCheck::ValueChange,
    );
    parameter.constraints.range = RangeConstraint::uniform(Some(0.0), None);
    crate::app::module::enable_module_authoring(parameter.node_data_mut());
    let meta = &mut parameter.node_data_mut().meta;
    meta.decl_id = DeclId(FREEZE_RADIUS_DECL_ID.to_string());
    meta.short_name = FREEZE_RADIUS_DECL_ID.to_string();
    parameter
}

fn unit_value_constraints() -> golden_core::parameter::ParameterConstraints {
    golden_core::parameter::ParameterConstraints {
        range: RangeConstraint::uniform(Some(0.0), Some(1.0)),
        ..Default::default()
    }
}

#[cfg(test)]
fn spatializer_value(
    config: &SpatializerConfig,
    target: &SpatializerEndpointConfig,
    source: &SpatializerEndpointConfig,
) -> f64 {
    value_for_pair(
        &spatializer_values_by_target(config),
        target.item_id,
        source.item_id,
    )
}

fn spatializer_values_by_target(
    config: &SpatializerConfig,
) -> HashMap<NodeId, HashMap<NodeId, f64>> {
    match config.mode {
        SpatializerMode::Voronoi => voronoi_values_by_target(config),
        SpatializerMode::TargetRadius => config
            .targets
            .iter()
            .map(|target| {
                (
                    target.item_id,
                    scalar_spatializer_values_for_target(config, target, |distance, _| {
                        radius_influence(distance, target.radius)
                    }),
                )
            })
            .collect(),
        SpatializerMode::SourceRadius => config
            .targets
            .iter()
            .map(|target| {
                (
                    target.item_id,
                    scalar_spatializer_values_for_target(config, target, |distance, source| {
                        radius_influence(distance, source.radius)
                    }),
                )
            })
            .collect(),
        SpatializerMode::Overlap => config
            .targets
            .iter()
            .map(|target| {
                (
                    target.item_id,
                    scalar_spatializer_values_for_target(config, target, |distance, source| {
                        overlap_value(distance, source.radius, target.radius)
                    }),
                )
            })
            .collect(),
    }
}

fn value_for_pair(
    values_by_target: &HashMap<NodeId, HashMap<NodeId, f64>>,
    target_id: NodeId,
    source_id: NodeId,
) -> f64 {
    values_by_target
        .get(&target_id)
        .and_then(|values| values.get(&source_id))
        .copied()
        .unwrap_or(0.0)
}

fn scalar_spatializer_values_for_target(
    config: &SpatializerConfig,
    target: &SpatializerEndpointConfig,
    value_for_source: impl Fn(f64, &SpatializerEndpointConfig) -> f64,
) -> HashMap<NodeId, f64> {
    if !target.enabled {
        return HashMap::new();
    }

    config
        .sources
        .iter()
        .filter(|source| source.enabled)
        .map(|source| {
            let distance = source.position.distance_to(target.position, config.dimension);
            (source.item_id, value_for_source(distance, source))
        })
        .collect()
}

fn voronoi_values_by_target(config: &SpatializerConfig) -> HashMap<NodeId, HashMap<NodeId, f64>> {
    let mut values_by_target: HashMap<NodeId, HashMap<NodeId, f64>> = config
        .targets
        .iter()
        .map(|target| (target.item_id, HashMap::new()))
        .collect();

    for source in config.sources.iter().filter(|source| source.enabled) {
        for (target_id, value) in voronoi_target_values_for_source(config, source) {
            values_by_target
                .entry(target_id)
                .or_default()
                .insert(source.item_id, value);
        }
    }

    values_by_target
}

fn voronoi_target_values_for_source(
    config: &SpatializerConfig,
    source: &SpatializerEndpointConfig,
) -> HashMap<NodeId, f64> {
    let target_indices = enabled_target_indices(config);
    if target_indices.is_empty() {
        return HashMap::new();
    }

    let frozen_indices = frozen_target_indices(config, source.position, &target_indices);
    if !frozen_indices.is_empty() {
        return split_between_targets(config, &frozen_indices);
    }

    let Some(current_index) = nearest_target_index(config, source.position, &target_indices) else {
        return HashMap::new();
    };
    let current = &config.targets[current_index];
    let current_distance = current.position.distance_to(source.position, config.dimension);
    let current_distance = (current_distance - current.freeze_radius.max(0.0)).max(0.0);
    if current_distance <= VORONOI_TIE_EPSILON {
        return split_between_targets(config, &[current_index]);
    }

    let mut raw_values = vec![(
        current.item_id,
        morpher_raw_weight_from_distance(current_distance),
    )];
    let mut boundary_entries = if config.dimension == SpatializerDimension::Two {
        morpher_boundary_entries_2d(config, source.position, current_index, &target_indices)
    } else {
        morpher_boundary_entries_nd(config, source.position, current_index, &target_indices)
    };
    if boundary_entries.is_empty() && target_indices.len() > 1 {
        boundary_entries =
            morpher_boundary_entries_nd(config, source.position, current_index, &target_indices);
    }

    for entry_index in 0..boundary_entries.len() {
        let entry = boundary_entries[entry_index];
        let weight = if boundary_entries.len() > 1 {
            let min_other_edge_distance = boundary_entries
                .iter()
                .enumerate()
                .filter(|(other_index, _)| *other_index != entry_index)
                .map(|(_, other)| other.edge_distance)
                .min_by(f64::total_cmp)
                .unwrap_or(f64::INFINITY);
            let denominator = entry.edge_distance + min_other_edge_distance;
            let ratio = if denominator <= VORONOI_TIE_EPSILON {
                1.0
            } else {
                1.0 - entry.edge_distance / denominator
            }
            .clamp(0.0, 1.0);
            morpher_raw_weight_from_path(ratio, entry.edge_distance + entry.neighbour_distance)
        } else {
            let neighbour = &config.targets[entry.neighbour_index];
            let direct_distance = neighbour
                .position
                .distance_to(source.position, config.dimension);
            morpher_raw_weight_from_distance(
                (direct_distance - neighbour.freeze_radius.max(0.0)).max(0.0),
            )
        };
        if weight > 0.0 {
            raw_values.push((config.targets[entry.neighbour_index].item_id, weight));
        }
    }

    normalize_raw_values(raw_values)
}

#[derive(Clone, Copy, Debug)]
struct VoronoiBoundaryEntry {
    neighbour_index: usize,
    edge_distance: f64,
    neighbour_distance: f64,
}

fn enabled_target_indices(config: &SpatializerConfig) -> Vec<usize> {
    config
        .targets
        .iter()
        .enumerate()
        .filter(|(_, target)| target.enabled)
        .map(|(target_index, _)| target_index)
        .collect()
}

fn nearest_target_index(
    config: &SpatializerConfig,
    position: SpatialPoint,
    target_indices: &[usize],
) -> Option<usize> {
    target_indices.iter().copied().min_by(|left, right| {
        let left_distance = config.targets[*left]
            .position
            .distance_to(position, config.dimension);
        let right_distance = config.targets[*right]
            .position
            .distance_to(position, config.dimension);
        left_distance.total_cmp(&right_distance)
    })
}

fn frozen_target_indices(
    config: &SpatializerConfig,
    position: SpatialPoint,
    target_indices: &[usize],
) -> Vec<usize> {
    let frozen_distance = target_indices
        .iter()
        .filter_map(|target_index| {
            let freeze_radius = config.targets[*target_index].freeze_radius.max(0.0);
            let distance = config.targets[*target_index]
                .position
                .distance_to(position, config.dimension);
            (distance <= freeze_radius + VORONOI_TIE_EPSILON).then_some(distance)
        })
        .min_by(f64::total_cmp);
    let Some(frozen_distance) = frozen_distance else {
        return Vec::new();
    };

    target_indices
        .iter()
        .filter_map(|target_index| {
            let distance = config.targets[*target_index]
                .position
                .distance_to(position, config.dimension);
            distances_tie(distance, frozen_distance).then_some(*target_index)
        })
        .collect()
}

fn split_between_targets(config: &SpatializerConfig, target_indices: &[usize]) -> HashMap<NodeId, f64> {
    if target_indices.is_empty() {
        return HashMap::new();
    }

    let value = 1.0 / target_indices.len() as f64;
    target_indices
        .iter()
        .map(|target_index| (config.targets[*target_index].item_id, value))
        .collect()
}

fn morpher_boundary_entries_2d(
    config: &SpatializerConfig,
    position: SpatialPoint,
    current_index: usize,
    target_indices: &[usize],
) -> Vec<VoronoiBoundaryEntry> {
    let current_cell = target_voronoi_cell_polygon(config, current_index, position, target_indices);
    if current_cell.len() < 3 {
        return Vec::new();
    }

    target_indices
        .iter()
        .copied()
        .filter(|target_index| *target_index != current_index)
        .filter_map(|neighbour_index| {
            closest_cell_edge_for_neighbour(config, position, &current_cell, current_index, neighbour_index)
        })
        .collect()
}

fn morpher_boundary_entries_nd(
    config: &SpatializerConfig,
    position: SpatialPoint,
    current_index: usize,
    target_indices: &[usize],
) -> Vec<VoronoiBoundaryEntry> {
    target_indices
        .iter()
        .copied()
        .filter(|target_index| *target_index != current_index)
        .filter_map(|neighbour_index| {
            boundary_entry_for_bisector(config, position, current_index, neighbour_index)
        })
        .collect()
}

fn target_voronoi_cell_polygon(
    config: &SpatializerConfig,
    current_index: usize,
    position: SpatialPoint,
    target_indices: &[usize],
) -> Vec<SpatialPoint> {
    let current = config.targets[current_index].position;
    let mut polygon = voronoi_bounds_polygon(config, position, target_indices);
    for other_index in target_indices {
        if *other_index == current_index {
            continue;
        }
        polygon = clip_to_closer_site(polygon, current, config.targets[*other_index].position);
        if polygon.len() < 3 {
            break;
        }
    }
    polygon
}

fn closest_cell_edge_for_neighbour(
    config: &SpatializerConfig,
    position: SpatialPoint,
    cell: &[SpatialPoint],
    current_index: usize,
    neighbour_index: usize,
) -> Option<VoronoiBoundaryEntry> {
    let current = config.targets[current_index].position;
    let neighbour = config.targets[neighbour_index].position;
    let mut best: Option<(f64, SpatialPoint)> = None;
    for edge_index in 0..cell.len() {
        let start = cell[edge_index];
        let end = cell[(edge_index + 1) % cell.len()];
        let start_value = closer_half_plane_value(start, current, neighbour).abs();
        let end_value = closer_half_plane_value(end, current, neighbour).abs();
        if start_value > 1.0e-6 || end_value > 1.0e-6 {
            continue;
        }
        let closest = closest_point_on_segment_2d(position, start, end);
        let distance = position.distance_to(closest, SpatializerDimension::Two);
        if best
            .as_ref()
            .is_none_or(|(best_distance, _)| distance < *best_distance)
        {
            best = Some((distance, closest));
        }
    }

    let (edge_distance, closest) = best?;
    let neighbour_distance = (closest.distance_to(neighbour, SpatializerDimension::Two)
        - config.targets[neighbour_index].freeze_radius.max(0.0))
    .max(0.0);
    Some(VoronoiBoundaryEntry {
        neighbour_index,
        edge_distance,
        neighbour_distance,
    })
}

fn boundary_entry_for_bisector(
    config: &SpatializerConfig,
    position: SpatialPoint,
    current_index: usize,
    neighbour_index: usize,
) -> Option<VoronoiBoundaryEntry> {
    let current = config.targets[current_index].position;
    let neighbour = config.targets[neighbour_index].position;
    let separation = current.distance_to(neighbour, config.dimension);
    if !separation.is_finite() || separation <= VORONOI_TIE_EPSILON {
        return None;
    }

    let unit = normalized_direction(current, neighbour, config.dimension)?;
    let midpoint = midpoint_for_dimension(current, neighbour, config.dimension);
    let signed_distance = dot_for_dimension(
        SpatialPoint::new(
            position.x - midpoint.x,
            position.y - midpoint.y,
            position.z - midpoint.z,
        ),
        unit,
        config.dimension,
    );
    let closest = SpatialPoint::new(
        position.x - unit.x * signed_distance,
        position.y - unit.y * signed_distance,
        if config.dimension == SpatializerDimension::Three {
            position.z - unit.z * signed_distance
        } else {
            0.0
        },
    );
    let neighbour_distance = (closest.distance_to(neighbour, config.dimension)
        - config.targets[neighbour_index].freeze_radius.max(0.0))
    .max(0.0);
    Some(VoronoiBoundaryEntry {
        neighbour_index,
        edge_distance: signed_distance.abs(),
        neighbour_distance,
    })
}

fn normalized_direction(
    from: SpatialPoint,
    to: SpatialPoint,
    dimension: SpatializerDimension,
) -> Option<SpatialPoint> {
    let distance = from.distance_to(to, dimension);
    if !distance.is_finite() || distance <= VORONOI_TIE_EPSILON {
        return None;
    }
    Some(SpatialPoint::new(
        (to.x - from.x) / distance,
        (to.y - from.y) / distance,
        if dimension == SpatializerDimension::Three {
            (to.z - from.z) / distance
        } else {
            0.0
        },
    ))
}

fn midpoint_for_dimension(
    left: SpatialPoint,
    right: SpatialPoint,
    dimension: SpatializerDimension,
) -> SpatialPoint {
    SpatialPoint::new(
        (left.x + right.x) * 0.5,
        (left.y + right.y) * 0.5,
        if dimension == SpatializerDimension::Three {
            (left.z + right.z) * 0.5
        } else {
            0.0
        },
    )
}

fn dot_for_dimension(
    left: SpatialPoint,
    right: SpatialPoint,
    dimension: SpatializerDimension,
) -> f64 {
    left.x * right.x
        + left.y * right.y
        + if dimension == SpatializerDimension::Three {
            left.z * right.z
        } else {
            0.0
        }
}

fn closest_point_on_segment_2d(
    point: SpatialPoint,
    start: SpatialPoint,
    end: SpatialPoint,
) -> SpatialPoint {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length_squared = dx * dx + dy * dy;
    if length_squared <= VORONOI_TIE_EPSILON {
        return start;
    }
    let t = (((point.x - start.x) * dx + (point.y - start.y) * dy) / length_squared).clamp(0.0, 1.0);
    SpatialPoint::new(start.x + dx * t, start.y + dy * t, 0.0)
}

fn morpher_raw_weight_from_distance(distance: f64) -> f64 {
    if !distance.is_finite() {
        return 0.0;
    }
    if distance <= VORONOI_TIE_EPSILON {
        return 1.0e12;
    }
    1.0 / distance
}

fn morpher_raw_weight_from_path(ratio: f64, distance: f64) -> f64 {
    let ratio = ratio.clamp(0.0, 1.0);
    if ratio <= 0.0 || !distance.is_finite() {
        return 0.0;
    }
    if distance <= VORONOI_TIE_EPSILON {
        return 1.0e12;
    }
    ratio / distance
}

fn voronoi_bounds_polygon(
    config: &SpatializerConfig,
    position: SpatialPoint,
    target_indices: &[usize],
) -> Vec<SpatialPoint> {
    let mut left = position.x;
    let mut right = position.x;
    let mut top = position.y;
    let mut bottom = position.y;
    for target_index in target_indices {
        let position = config.targets[*target_index].position;
        left = left.min(position.x);
        right = right.max(position.x);
        top = top.min(position.y);
        bottom = bottom.max(position.y);
    }

    let width = (right - left).abs();
    let height = (bottom - top).abs();
    let padding = width.max(height).max(1.0) * 4.0;
    left -= padding;
    right += padding;
    top -= padding;
    bottom += padding;

    vec![
        SpatialPoint::new(left, top, 0.0),
        SpatialPoint::new(right, top, 0.0),
        SpatialPoint::new(right, bottom, 0.0),
        SpatialPoint::new(left, bottom, 0.0),
    ]
}

fn clip_to_closer_site(
    polygon: Vec<SpatialPoint>,
    site: SpatialPoint,
    other: SpatialPoint,
) -> Vec<SpatialPoint> {
    if polygon.is_empty() {
        return polygon;
    }

    let mut clipped = Vec::new();
    for index in 0..polygon.len() {
        let current = polygon[index];
        let previous = polygon[(index + polygon.len() - 1) % polygon.len()];
        let current_value = closer_half_plane_value(current, site, other);
        let previous_value = closer_half_plane_value(previous, site, other);
        let current_inside = current_value <= VORONOI_TIE_EPSILON;
        let previous_inside = previous_value <= VORONOI_TIE_EPSILON;

        if current_inside && !previous_inside {
            clipped.push(half_plane_intersection(
                previous,
                current,
                previous_value,
                current_value,
            ));
        }
        if current_inside {
            clipped.push(current);
        } else if previous_inside {
            clipped.push(half_plane_intersection(
                previous,
                current,
                previous_value,
                current_value,
            ));
        }
    }
    clipped
}

fn closer_half_plane_value(point: SpatialPoint, site: SpatialPoint, other: SpatialPoint) -> f64 {
    let dx = other.x - site.x;
    let dy = other.y - site.y;
    2.0 * (dx * point.x + dy * point.y)
        - (other.x * other.x + other.y * other.y - site.x * site.x - site.y * site.y)
}

fn half_plane_intersection(
    start: SpatialPoint,
    end: SpatialPoint,
    start_value: f64,
    end_value: f64,
) -> SpatialPoint {
    let denominator = start_value - end_value;
    if denominator.abs() <= VORONOI_TIE_EPSILON {
        return end;
    }
    let t = (start_value / denominator).clamp(0.0, 1.0);
    SpatialPoint::new(
        start.x + (end.x - start.x) * t,
        start.y + (end.y - start.y) * t,
        0.0,
    )
}

fn normalize_raw_values(raw_values: Vec<(NodeId, f64)>) -> HashMap<NodeId, f64> {
    let total: f64 = raw_values
        .iter()
        .map(|(_, value)| *value)
        .filter(|value| value.is_finite() && *value > 0.0)
        .sum();
    if total <= 0.0 {
        return HashMap::new();
    }

    raw_values
        .into_iter()
        .filter(|(_, value)| value.is_finite() && *value > 0.0)
        .map(|(source_id, value)| (source_id, clamp01(value / total)))
        .collect()
}

fn distances_tie(left: f64, right: f64) -> bool {
    left.is_finite() && right.is_finite() && (left - right).abs() <= VORONOI_TIE_EPSILON
}

fn radius_influence(distance: f64, radius: f64) -> f64 {
    if !distance.is_finite() || !radius.is_finite() {
        return 0.0;
    }
    if radius <= 0.0 {
        return if distance <= 0.0 { 1.0 } else { 0.0 };
    }
    clamp01(1.0 - distance / radius)
}

fn overlap_value(distance: f64, source_radius: f64, target_radius: f64) -> f64 {
    if !distance.is_finite() || !source_radius.is_finite() || !target_radius.is_finite() {
        return 0.0;
    }
    let source_radius = source_radius.max(0.0);
    let target_radius = target_radius.max(0.0);
    if source_radius <= 0.0 || target_radius <= 0.0 {
        return if distance <= 0.0 && source_radius == target_radius {
            1.0
        } else {
            0.0
        };
    }

    let min_radius = source_radius.min(target_radius);
    let max_radius = source_radius.max(target_radius);
    if distance <= max_radius - min_radius {
        return 1.0;
    }
    let separation = source_radius + target_radius;
    if distance >= separation {
        return 0.0;
    }

    clamp01((separation - distance) / (2.0 * min_radius))
}

fn clamp01(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

fn child_ids_by_decl(snapshot: &ProcessTreeSnapshot, root_id: NodeId) -> HashMap<String, NodeId> {
    let mut by_decl = HashMap::new();
    for child_id in snapshot.child_ids(root_id) {
        let Some(child) = snapshot.node(child_id) else {
            continue;
        };
        if child.decl_id.trim().is_empty() {
            continue;
        }
        by_decl.entry(child.decl_id.clone()).or_insert(child_id);
    }
    by_decl
}

fn direct_child_ids_by_decl(
    snapshot: &ProcessTreeSnapshot,
    parent_id: NodeId,
    decl_id: &str,
) -> Vec<NodeId> {
    snapshot
        .child_ids(parent_id)
        .into_iter()
        .filter(|child_id| {
            snapshot
                .node(*child_id)
                .is_some_and(|child| child.decl_id == decl_id)
        })
        .collect()
}

fn direct_child_ids_by_decl_tail(
    snapshot: &ProcessTreeSnapshot,
    parent_id: NodeId,
    decl_tail: &str,
) -> Vec<NodeId> {
    snapshot
        .child_ids(parent_id)
        .into_iter()
        .filter(|child_id| {
            snapshot.node(*child_id).is_some_and(|child| {
                child.decl_id == decl_tail
                    || child.decl_id.rsplit('/').next() == Some(decl_tail)
            })
        })
        .collect()
}

fn node_decl_tail_matches(
    snapshot: &ProcessTreeSnapshot,
    node_id: NodeId,
    decl_tail: &str,
) -> bool {
    snapshot.node(node_id).is_some_and(|node| {
        node.decl_id == decl_tail || node.decl_id.rsplit('/').next() == Some(decl_tail)
    })
}

fn value_decl_id(prefix: &str, item_uuid: NodeUuid) -> String {
    format!("{}_{}", prefix, item_uuid.0.simple())
}

fn target_folder_uuid(target_uuid: NodeUuid) -> NodeUuid {
    const TARGET_FOLDER_UUID_MASK: u128 = 0x7370_6174_6961_6c69_7a65_725f_7467_0000;
    NodeUuid(uuid::Uuid::from_u128(
        target_uuid.0.as_u128() ^ TARGET_FOLDER_UUID_MASK,
    ))
}

fn source_folder_uuid(source_uuid: NodeUuid) -> NodeUuid {
    const SOURCE_FOLDER_UUID_MASK: u128 = 0x7370_6174_6961_6c69_7a65_725f_7366_0000;
    NodeUuid(uuid::Uuid::from_u128(
        source_uuid.0.as_u128() ^ SOURCE_FOLDER_UUID_MASK,
    ))
}

fn source_value_uuid(target_uuid: NodeUuid, source_uuid: NodeUuid) -> NodeUuid {
    const SOURCE_VALUE_UUID_MASK: u128 = 0x7370_6174_6961_6c69_7a65_725f_7372_0000;
    NodeUuid(uuid::Uuid::from_u128(
        target_uuid.0.as_u128().rotate_left(1)
            ^ source_uuid.0.as_u128().rotate_right(1)
            ^ SOURCE_VALUE_UUID_MASK,
    ))
}

fn target_value_uuid(source_uuid: NodeUuid, target_uuid: NodeUuid) -> NodeUuid {
    const TARGET_VALUE_UUID_MASK: u128 = 0x7370_6174_6961_6c69_7a65_725f_7476_0000;
    NodeUuid(uuid::Uuid::from_u128(
        source_uuid.0.as_u128().rotate_left(1)
            ^ target_uuid.0.as_u128().rotate_right(1)
            ^ TARGET_VALUE_UUID_MASK,
    ))
}

fn node_is_descendant_or_self(
    snapshot: &ProcessTreeSnapshot,
    node_id: NodeId,
    ancestor_id: NodeId,
) -> bool {
    let mut current = Some(node_id);
    while let Some(current_id) = current {
        if current_id == ancestor_id {
            return true;
        }
        current = snapshot.node(current_id).and_then(|node| node.parent);
    }
    false
}

fn child_param_value(snapshot: &ProcessTreeSnapshot, node_id: NodeId) -> Option<ParamValue> {
    snapshot.node(node_id).and_then(|node| node.param_value.clone())
}

fn child_value(snapshot: &ProcessTreeSnapshot, parent: NodeId, key: &str) -> Option<ParamValue> {
    snapshot
        .find_child(parent, key)
        .and_then(|node_id| snapshot.node(node_id))
        .and_then(|node| node.param_value.clone())
}

fn child_float(snapshot: &ProcessTreeSnapshot, parent: NodeId, key: &str, default: f64) -> f64 {
    child_value(snapshot, parent, key)
        .and_then(|value| value.as_float())
        .filter(|value| value.is_finite())
        .unwrap_or(default)
}

fn child_position(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    dimension: SpatializerDimension,
    default: SpatialPoint,
) -> SpatialPoint {
    child_value(snapshot, parent, dimension.position_decl_id())
        .and_then(param_value_to_point)
        .unwrap_or(default)
}

fn param_value_to_point(value: ParamValue) -> Option<SpatialPoint> {
    match value {
        ParamValue::Vec2(x, y) if x.is_finite() && y.is_finite() => {
            Some(SpatialPoint::new(x, y, 0.0))
        }
        ParamValue::Vec3(x, y, z) if x.is_finite() && y.is_finite() && z.is_finite() => {
            Some(SpatialPoint::new(x, y, z))
        }
        _ => None,
    }
}

fn position_value_matches_dimension(value: &ParamValue, dimension: SpatializerDimension) -> bool {
    matches!(
        (dimension, value),
        (SpatializerDimension::Two, ParamValue::Vec2(_, _))
            | (SpatializerDimension::Three, ParamValue::Vec3(_, _, _))
    )
}
