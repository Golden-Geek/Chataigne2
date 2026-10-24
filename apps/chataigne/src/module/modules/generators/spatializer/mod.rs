mod builders;
mod endpoints;
mod helpers;
mod math;
mod publication;
mod service;
#[cfg(test)]
mod tests;
mod worker;

use builders::*;
use delaunator::{triangulate, Point};
use endpoints::*;
use golden_core::{
    edit::NodeTree,
    engine::NodeExecutionRule,
    events::{CustomEvent, Event, EventFrame, EventKind},
    node,
    node::{DeclId, Folder, Node, NodeHandle, NodeId, NodeMetaPatch, NodeUuid, UserContainerRules, UserCreatableItem},
    parameter::{Enum, ParamValue, Parameter, ParameterChangeCheck, ParameterEnumOption, RangeConstraint},
    process_ctx::{ProcessCtx, ProcessTreeNodeSnapshot, ProcessTreeSnapshot},
};
use helpers::*;
#[cfg(test)]
use math::{spatializer_value, spatializer_values_by_target, VoronoiTopology};
use std::collections::{HashMap, HashSet};

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

const SPATIALIZER_SOURCE_ENABLEMENT_CHANGED_EVENT: &str = "spatializer.sources.itemEnablementChanged";
const SPATIALIZER_TARGET_ENABLEMENT_CHANGED_EVENT: &str = "spatializer.targets.itemEnablementChanged";

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
    service: service::Service,
}

impl SpatializerModule {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleBase::new(), service::Service::new())
    }

    fn request_service(&self) {
        self.service.request();
    }

    fn parameter_event(&self, snapshot: &ProcessTreeSnapshot, node: NodeId) -> bool {
        // Authored Spatializer inputs have a fixed three-edge path. Never walk
        // arbitrary user trees or maintain an entry for every generated pair.
        let Some(parameters) = self.base.parameters_id() else {
            return false;
        };
        let mut current = Some(node);
        for _ in 0..4 {
            if current == Some(parameters) {
                return true;
            }
            current = current.and_then(|id| snapshot.node(id)).and_then(|n| n.parent);
        }
        false
    }
}

#[golden_core::item("module", node = "spatializer_module", via = base, from_struct, menu_path = ["Generators"])]
impl Node for SpatializerModule {
    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        u32::MAX
    }

    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);
        self.base
            .set_data_capabilities(ctx, crate::app::module::ModuleDataCapabilities::new(true, false));
        self.base.set_connected(ctx, true);
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: golden_core::node::NodeCreationContext) {
        if let Err(error) = self.service.register(ctx) {
            NodeHandle::new(self.id()).set_warning_with(ctx, Some("spatializer_service"), error, None);
        }
        self.request_service();
    }

    fn on_worker_wake(&mut self, ctx: &mut ProcessCtx) {
        let module = self.id();
        if let (Some(parameters), Some(values)) = (self.base.parameters_id(), self.base.values_id()) {
            self.service.turn(ctx, module, parameters, values);
        }
    }

    fn worker_wake_requires_tree_snapshot(&self) -> bool {
        true
    }
    fn worker_wake_when_disabled(&self) -> bool {
        true
    }
    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::passive().with_compiled_kernel("chataigne.runtime.spatializer")
    }
    fn update_requires_tree_snapshot(&self) -> bool {
        false
    }
    fn inbox_requires_tree_snapshot(&self, events: &EventFrame) -> bool {
        events
            .iter()
            .any(|event| matches!(event.kind, EventKind::ParamChanged { .. }))
    }
    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, old_value: ParamValue) {
        if let Some(snapshot) = ctx.tree_snapshot_arc() {
            if self.parameter_event(&snapshot, param) {
                self.request_service();
            }
            let matrix_output = snapshot
                .node(param)
                .and_then(|node| node.parent)
                .and_then(|parent| snapshot.node(parent))
                .is_some_and(|parent| {
                    parent.parent == self.base.values_id()
                        && (parent.decl_id.starts_with(VALUE_SOURCE_DECL_PREFIX)
                            || parent.decl_id.starts_with(VALUE_TARGET_DECL_PREFIX))
                });
            if !matrix_output {
                self.base.emit_script_param_callback(ctx, &snapshot, param, &old_value);
            }
        }
    }
    fn on_structure_changed(&mut self, _ctx: &mut ProcessCtx) {
        self.service.structure_changed();
    }
    fn on_meta_changed(&mut self, _ctx: &mut ProcessCtx, _node: NodeId, patch: NodeMetaPatch) {
        if patch.label.is_some() || patch.short_name.is_some() || patch.enabled.is_some() {
            self.service.reconcile = true;
            self.request_service();
        }
    }
    fn on_effective_enabled_changed(&mut self, _ctx: &mut ProcessCtx, _enabled: bool) {
        self.service.cancel();
        self.request_service();
    }
    fn on_custom_event(&mut self, _ctx: &mut ProcessCtx, event: CustomEvent) {
        if event.topic == SPATIALIZER_SOURCE_ENABLEMENT_CHANGED_EVENT
            || event.topic == SPATIALIZER_TARGET_ENABLEMENT_CHANGED_EVENT
        {
            self.request_service();
        }
    }
    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.service.cancel();
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
        vec![UserCreatableItem::new(
            SpatializerSourceItem::NODE_TYPE,
            SPATIALIZER_SOURCE_ITEM_KIND,
            SPATIALIZER_SOURCE_ITEM_LABEL,
        )
        .with_select_when_created(false)]
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
        vec![UserCreatableItem::new(
            SpatializerTargetItem::NODE_TYPE,
            SPATIALIZER_TARGET_ITEM_KIND,
            SPATIALIZER_TARGET_ITEM_LABEL,
        )
        .with_select_when_created(false)]
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
        enum_option(SPATIALIZER_VALUE_LAYOUT_SOURCE_CENTRIC, "Source Centric", 0),
        enum_option(SPATIALIZER_VALUE_LAYOUT_TARGET_CENTRIC, "Target Centric", 1),
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
