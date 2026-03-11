#![allow(missing_docs)]

mod widget_options;

use self::widget_options::{
    dashboard_widget_options_node_type, make_dashboard_widget_options_node, refresh_dashboard_widget_options_node,
};
use crate::events::Event;
use crate::node::{DeclId, EventPropagation, Node, NodeData, NodeId, NodeReference, NodeUserPermissions};
use crate::parameter::{
    CssUnit, CssValue, Enum, ParamValue, Parameter, ParameterChangeCheck, ParameterEnumOption, Vec2,
};
use crate::process_ctx::ProcessCtx;
use crate::{item, node};

pub use self::widget_options::{
    DASHBOARD_NODE_WIDGET_COLOR_EDITOR_OPTIONS_NODE_TYPE, DASHBOARD_NODE_WIDGET_INSPECTOR_OPTIONS_NODE_TYPE,
    DASHBOARD_NODE_WIDGET_NUMBER_ROTARY_OPTIONS_NODE_TYPE, DASHBOARD_NODE_WIDGET_NUMBER_SLIDER_OPTIONS_NODE_TYPE,
    DASHBOARD_NODE_WIDGET_PARAMETER_EDITOR_OPTIONS_NODE_TYPE, DASHBOARD_NODE_WIDGET_VEC2_EDITOR_OPTIONS_NODE_TYPE,
    DASHBOARD_NODE_WIDGET_VEC2_PAD_OPTIONS_NODE_TYPE, DASHBOARD_NODE_WIDGET_VEC3_EDITOR_OPTIONS_NODE_TYPE,
    DashboardNodeWidgetColorEditorOptionsNode, DashboardNodeWidgetInspectorOptionsNode,
    DashboardNodeWidgetNumberRotaryOptionsNode, DashboardNodeWidgetNumberSliderOptionsNode,
    DashboardNodeWidgetParameterEditorOptionsNode, DashboardNodeWidgetVec2EditorOptionsNode,
    DashboardNodeWidgetVec2PadOptionsNode, DashboardNodeWidgetVec3EditorOptionsNode,
};

/// Runtime node type id for dashboard roots.
pub const DASHBOARD_NODE_TYPE: &str = "dashboard";
/// User-item kind for dashboard roots.
pub const DASHBOARD_ITEM_KIND: &str = "dashboard";
/// Runtime node type id for dashboard pages.
pub const DASHBOARD_PAGE_NODE_TYPE: &str = "dashboard_page";
/// User-item kind for dashboard pages.
pub const DASHBOARD_PAGE_ITEM_KIND: &str = "dashboard_page";
/// Runtime node type id for dashboard container widgets.
pub const DASHBOARD_WIDGET_CONTAINER_NODE_TYPE: &str = "dashboard_widget_container";
/// Runtime node type id for dashboard node widgets.
pub const DASHBOARD_NODE_WIDGET_NODE_TYPE: &str = "dashboard_node_widget";
/// Runtime node type id for dashboard generic widgets.
pub const DASHBOARD_GENERIC_WIDGET_NODE_TYPE: &str = "dashboard_generic_widget";
/// Shared user-item kind for all dashboard widgets.
pub const DASHBOARD_WIDGET_ITEM_KIND: &str = "dashboard_widget";

/// One dashboard widget type exposed by a target node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DashboardWidgetTypeSpec {
    /// Stable type id used by widget parameters and UI renderers.
    pub id: String,
    /// User-visible type label.
    pub label: String,
    /// Optional target-specific child node mounted under the widget for this type.
    pub options_node_kind: Option<DashboardWidgetOptionsNodeKind>,
}

impl DashboardWidgetTypeSpec {
    /// Creates one widget-type descriptor.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            options_node_kind: None,
        }
    }

    /// Attaches one widget-type-specific options node kind.
    pub fn with_options_node_kind(mut self, kind: DashboardWidgetOptionsNodeKind) -> Self {
        self.options_node_kind = Some(kind);
        self
    }
}

/// Target-specific child node family exposed to dashboard widgets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DashboardWidgetOptionsNodeKind {
    /// Inspector-style node widget options.
    Inspector,
    /// Default parameter editor options.
    ParameterEditor,
    /// Slider-style scalar parameter options.
    NumberSlider,
    /// Rotary-style scalar parameter options.
    NumberRotary,
    /// 2D pad options for vec2 parameters.
    Vec2Pad,
    /// Default vec2 editor options.
    Vec2Editor,
    /// Default vec3 editor options.
    Vec3Editor,
    /// Default color editor options.
    ColorEditor,
}

/// Dashboard widget authoring capabilities exposed by one target node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DashboardWidgetTargetDescriptor {
    /// Widget types supported by the target node.
    pub widget_types: Vec<DashboardWidgetTypeSpec>,
    /// Natural default widget type for the target node.
    pub default_widget_type_id: String,
}

impl DashboardWidgetTargetDescriptor {
    /// Creates the default inspector-only target descriptor.
    pub fn inspector_only() -> Self {
        Self {
            widget_types: vec![
                DashboardWidgetTypeSpec::new("inspector", "Inspector")
                    .with_options_node_kind(DashboardWidgetOptionsNodeKind::Inspector),
            ],
            default_widget_type_id: "inspector".to_string(),
        }
    }

    fn widget_type(&self, type_id: &str) -> Option<&DashboardWidgetTypeSpec> {
        self.widget_types.iter().find(|widget_type| widget_type.id == type_id)
    }
}

fn enable_dashboard_authoring(node_data: &mut NodeData) {
    node_data.meta.user_permissions = NodeUserPermissions::all();
}

fn snapshot_find_descendant_by_decl(
    snapshot: &crate::process_ctx::ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
) -> Option<NodeId> {
    if decl_id.contains('/') {
        if let Some(node_id) = snapshot.resolve_path_from(parent, decl_id) {
            return Some(node_id);
        }
    }

    let mut stack = vec![parent];
    while let Some(node_id) = stack.pop() {
        let mut child = snapshot.node(node_id)?.first_child;
        while let Some(child_id) = child {
            let child_snapshot = snapshot.node(child_id)?;
            if child_snapshot.decl_id == decl_id || child_snapshot.decl_id.rsplit('/').next() == Some(decl_id) {
                return Some(child_id);
            }
            stack.push(child_id);
            child = child_snapshot.next_sibling;
        }
    }

    None
}

fn snapshot_child_decl_matches(
    snapshot: &crate::process_ctx::ProcessTreeSnapshot,
    node_id: NodeId,
    decl_id: &str,
) -> bool {
    snapshot
        .node(node_id)
        .map(|node| node.decl_id == decl_id || node.decl_id.rsplit('/').next() == Some(decl_id))
        .unwrap_or(false)
}

fn resolve_snapshot_reference_target(
    snapshot: &crate::process_ctx::ProcessTreeSnapshot,
    reference: &NodeReference,
) -> Option<NodeId> {
    reference
        .cached_id()
        .filter(|node_id| snapshot.node(*node_id).is_some())
        .or_else(|| {
            (!reference.uuid().is_nil())
                .then(|| snapshot.node_id_by_uuid(reference.uuid()))
                .flatten()
        })
}

fn dashboard_parent_layout_kind(node_id: NodeId, ctx: &ProcessCtx) -> Option<String> {
    let snapshot = ctx.tree_snapshot()?;
    let parent_id = snapshot.node(node_id)?.parent?;
    let parent_type = snapshot.node(parent_id)?.node_type.as_str();
    match parent_type {
        DASHBOARD_PAGE_NODE_TYPE | DASHBOARD_WIDGET_CONTAINER_NODE_TYPE => {
            snapshot_find_descendant_by_decl(snapshot, parent_id, "layout_kind")
                .and_then(|layout_kind| snapshot.node(layout_kind))
                .and_then(|layout_kind| layout_kind.param_value.as_ref())
                .and_then(|value| value.as_enum().or_else(|| value.as_str()))
                .map(|value| value.to_string())
        }
        _ => None,
    }
}

fn dashboard_parent_layout_is(node_id: NodeId, ctx: &ProcessCtx, expected: &str) -> bool {
    dashboard_parent_layout_kind(node_id, ctx).as_deref() == Some(expected)
}

fn dashboard_parent_layout_matches(node_id: NodeId, ctx: &ProcessCtx, expected: &[&str]) -> bool {
    let Some(layout_kind) = dashboard_parent_layout_kind(node_id, ctx) else {
        return false;
    };
    expected.iter().any(|candidate| *candidate == layout_kind)
}

fn dashboard_widget_target_descriptor_for_reference(
    target_reference: &NodeReference,
    ctx: &ProcessCtx,
) -> DashboardWidgetTargetDescriptor {
    let Some(snapshot) = ctx.tree_snapshot() else {
        return DashboardWidgetTargetDescriptor::inspector_only();
    };
    let Some(target_id) = resolve_snapshot_reference_target(snapshot, target_reference) else {
        return DashboardWidgetTargetDescriptor::inspector_only();
    };
    snapshot
        .node(target_id)
        .map(|target| target.dashboard_widget_target.clone())
        .unwrap_or_else(DashboardWidgetTargetDescriptor::inspector_only)
}

fn dashboard_node_widget_current_widget_type(node_id: NodeId, ctx: &ProcessCtx) -> Option<String> {
    let snapshot = ctx.tree_snapshot()?;
    let widget_type = snapshot_find_descendant_by_decl(snapshot, node_id, "widget_type")?;
    snapshot
        .node(widget_type)?
        .param_value
        .as_ref()
        .and_then(|value| value.as_enum().or_else(|| value.as_str()))
}

fn dashboard_widget_type_enum_option(widget_type: &DashboardWidgetTypeSpec, ordering: i32) -> ParameterEnumOption {
    ParameterEnumOption {
        variant_id: widget_type.id.clone(),
        value: ParamValue::Enum(widget_type.id.clone()),
        label: widget_type.label.clone(),
        tags: Vec::new(),
        ordering: Some(ordering),
    }
}

fn make_dashboard_node_widget_type_parameter(
    value: impl Into<String>,
    descriptor: &DashboardWidgetTargetDescriptor,
) -> Parameter {
    let value = value.into();
    let mut parameter = Parameter::new("Type", ParamValue::Enum(value), ParameterChangeCheck::ValueChange);
    parameter.node_data_mut().meta.decl_id = DeclId("widget_type".to_string());
    parameter.node_data_mut().meta.description =
        Some("Widget rendering type used for the generated node UI.".to_string());
    parameter.constraints.enum_options = descriptor
        .widget_types
        .iter()
        .enumerate()
        .map(|(index, widget_type)| dashboard_widget_type_enum_option(widget_type, index as i32))
        .collect();
    parameter
}

fn refresh_dashboard_widget_dependencies(ctx: &mut ProcessCtx, widget: NodeId) {
    let Some(node_type) = ctx
        .tree_snapshot()
        .and_then(|snapshot| snapshot.node(widget))
        .map(|snapshot| snapshot.node_type.clone())
    else {
        return;
    };

    match node_type.as_str() {
        DASHBOARD_WIDGET_CONTAINER_NODE_TYPE => crate::node::NodeHandle::new(widget)
            .with_mut::<DashboardWidgetContainerNode, _>(ctx, |node, child_ctx| {
                node.sync_parent_layout_dependency_handles(child_ctx);
                node.__golden_node_engine_preprocess_inbox(child_ctx, node.id());
            }),
        DASHBOARD_NODE_WIDGET_NODE_TYPE => {
            crate::node::NodeHandle::new(widget).with_mut::<DashboardNodeWidgetNode, _>(ctx, |node, child_ctx| {
                node.sync_parent_layout_dependency_handles(child_ctx);
                node.__golden_node_engine_preprocess_inbox(child_ctx, node.id());
            })
        }
        DASHBOARD_GENERIC_WIDGET_NODE_TYPE => crate::node::NodeHandle::new(widget)
            .with_mut::<DashboardGenericWidgetNode, _>(ctx, |node, child_ctx| {
                node.sync_parent_layout_dependency_handles(child_ctx);
                node.__golden_node_engine_preprocess_inbox(child_ctx, node.id());
            }),
        _ => {}
    }
}

fn refresh_direct_dashboard_widgets(ctx: &mut ProcessCtx, parent: NodeId) {
    let Some(child_ids) = ctx.tree_snapshot().map(|snapshot| snapshot.child_ids(parent)) else {
        return;
    };

    for child_id in child_ids {
        refresh_dashboard_widget_dependencies(ctx, child_id);
    }
}

/// Root dashboard node that owns a collection of dashboard pages.
#[allow(missing_docs)]
#[node("dashboard", label = "Dashboard")]
pub struct DashboardNode {}

#[node("dashboard", from_struct)]
impl Node for DashboardNode {
    crate::define_user_item_factory_methods! {
        accepts = ["dashboard_page"];
        items = [
            {
                type: DashboardPageNode,
            },
        ];
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        enable_dashboard_authoring(self.node_data_mut());
    }

    fn user_item_kind(&self) -> &str {
        DASHBOARD_ITEM_KIND
    }

    fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
        EventPropagation::Notify
    }
}

/// Top-level dashboard page that arranges widgets for one route/view.
#[allow(missing_docs)]
#[node("dashboard_page", label = "Dashboard Page")]
#[children(
    route: String = "".to_string() (label = "Route", description = "Stable route or slug used to address this page.");
    page_size: Vec2 = (1920.0, 1080.0) [(1.0, 1.0)..] (
        label = "Page Size",
        description = "Rendered page size in pixels. The dashboard viewer treats those values as the page render surface dimensions.",
        enabled = false,
        can_be_disabled = true,
        step = 1.0,
        step_base = 0.0,
    );
    layout_kind: Enum = "free" (
        label = "Layout",
        description = "Primary layout strategy used for the page root.",
        enum_options = ["free", "horizontal", "vertical", "grid", "accordion", "tabs"],
    );
    gap_x: CssValue = CssValue::new(1.0, CssUnit::Rem) (label = "Gap X", description = "Horizontal spacing between child widgets.");
    gap_y: CssValue = CssValue::new(1.0, CssUnit::Rem) (label = "Gap Y", description = "Vertical spacing between child widgets.");
    grid_columns: i32 = 12 [1..64] (
        label = "Grid Columns",
        description = "Column count when the page layout uses a grid.",
        dependency = layout_kind == "grid",
    );
    snap_grid: f64 = 1.0 [0.0..100.0] (
        label = "Snap Grid",
        description = "Grid size in rem used to snap widgets while editing free layouts. Disable to move widgets freely.",
        dependency = layout_kind == "free",
        enabled = false,
        can_be_disabled = true,
    );
    scrollable: bool = true (label = "Scrollable", description = "Whether this page may scroll when content exceeds the viewport.");
)]
pub struct DashboardPageNode {}

#[item("dashboard_page", from_struct)]
impl Node for DashboardPageNode {
    crate::define_user_item_factory_methods! {
        accepts = ["dashboard_widget"];
        items = [
            {
                type: DashboardWidgetContainerNode,
            },
            {
                type: DashboardNodeWidgetNode,
            },
            {
                type: DashboardGenericWidgetNode,
            },
        ];
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        enable_dashboard_authoring(self.node_data_mut());
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: crate::parameter::ParamValue) {
        if param != self.layout_kind.id() {
            return;
        }

        refresh_direct_dashboard_widgets(ctx, self.id());
    }

    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        1
    }

    fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
        EventPropagation::Notify
    }
}

/// Widget container that arranges child widgets using one layout strategy.
#[allow(missing_docs)]
#[node("dashboard_widget_container", label = "Container Widget")]
#[children(
    folder(appearance, label = "Appearance") {
        label_placement: Enum = "top" (
            label = "Label Placement",
            description = "Where the container label is rendered. Disable to hide it.",
            enum_options = ["left", "right", "top", "bottom", "inside"],
            can_be_disabled = true,
        );
    }
    folder(layout, label = "Layout") {
        position: Vec2 = (0.0, 0.0) (
            label = "Position",
            description = "Position offset in pixels relative to the selected free-layout anchor.",
            dependency = |node: &Self, ctx: &ProcessCtx| dashboard_parent_layout_is(node.id(), ctx, "free"),
        );
        anchor: Enum = "top-left" (
            label = "Anchor",
            description = "Anchor used to interpret the widget position within free layouts.",
            dependency = |node: &Self, ctx: &ProcessCtx| dashboard_parent_layout_is(node.id(), ctx, "free"),
            enum_options = ["top-left", "top-center", "top-right", "center-left", "center", "center-right", "bottom-left", "bottom-center", "bottom-right"],
        );
        width: CssValue = CssValue::new(12.0, CssUnit::Rem) (
            label = "Width",
            description = "Preferred widget width when the parent layout allows widgets to size horizontally.",
            dependency = |node: &Self, ctx: &ProcessCtx| dashboard_parent_layout_matches(node.id(), ctx, &["free", "horizontal"]),
        );
        height: CssValue = CssValue::new(8.0, CssUnit::Rem) (
            label = "Height",
            description = "Preferred widget height when the parent layout allows widgets to size vertically.",
            dependency = |node: &Self, ctx: &ProcessCtx| dashboard_parent_layout_matches(node.id(), ctx, &["free", "vertical", "grid"]),
        );
        column_span: i32 = 1 [1..64] (
            label = "Column Span",
            description = "Number of grid columns consumed by this widget.",
            dependency = |node: &Self, ctx: &ProcessCtx| dashboard_parent_layout_is(node.id(), ctx, "grid"),
        );
        row_span: i32 = 1 [1..64] (
            label = "Row Span",
            description = "Number of grid rows consumed by this widget.",
            dependency = |node: &Self, ctx: &ProcessCtx| dashboard_parent_layout_is(node.id(), ctx, "grid"),
        );
    }
    folder(content, label = "Content") {
        layout_kind: Enum = "free" (
            label = "Layout",
            description = "How this container arranges its child widgets.",
            enum_options = ["free", "horizontal", "vertical", "grid", "accordion", "tabs"],
        );
        gap_x: CssValue = CssValue::new(1.0, CssUnit::Rem) (label = "Gap X", description = "Horizontal spacing between child widgets.");
        gap_y: CssValue = CssValue::new(1.0, CssUnit::Rem) (label = "Gap Y", description = "Vertical spacing between child widgets.");
        grid_columns: i32 = 12 [1..64] (
            label = "Grid Columns",
            description = "Column count when the container layout uses a grid.",
            dependency = layout_kind == "grid",
        );
        snap_grid: f64 = 1.0 [0.0..100.0] (
            label = "Snap Grid",
            description = "Grid size in rem used to snap child widgets while editing free layouts. Disable to move widgets freely.",
            dependency = layout_kind == "free",
            enabled = false,
            can_be_disabled = true,
        );
    }
)]
pub struct DashboardWidgetContainerNode {}

impl DashboardWidgetContainerNode {
    fn sync_parent_layout_dependency_handles(&mut self, ctx: &ProcessCtx) {
        let Some(snapshot) = ctx.tree_snapshot() else {
            return;
        };

        if let Some(position) = snapshot_find_descendant_by_decl(snapshot, self.id(), "position") {
            self.position.set_node_id(position);
        } else {
            self.position.clear_node_id();
        }

        if let Some(column_span) = snapshot_find_descendant_by_decl(snapshot, self.id(), "column_span") {
            self.column_span.set_node_id(column_span);
        } else {
            self.column_span.clear_node_id();
        }

        if let Some(row_span) = snapshot_find_descendant_by_decl(snapshot, self.id(), "row_span") {
            self.row_span.set_node_id(row_span);
        } else {
            self.row_span.clear_node_id();
        }
    }
}

#[item("dashboard_widget", from_struct)]
impl Node for DashboardWidgetContainerNode {
    crate::define_user_item_factory_methods! {
        accepts = ["dashboard_widget"];
        items = [
            {
                type: DashboardWidgetContainerNode,
            },
            {
                type: DashboardNodeWidgetNode,
            },
            {
                type: DashboardGenericWidgetNode,
            },
        ];
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        enable_dashboard_authoring(self.node_data_mut());
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: crate::parameter::ParamValue) {
        let Some(snapshot) = ctx.tree_snapshot() else {
            return;
        };
        if !snapshot_child_decl_matches(snapshot, param, "layout_kind") {
            return;
        }

        refresh_direct_dashboard_widgets(ctx, self.id());
    }

    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        1
    }

    fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
        EventPropagation::Notify
    }
}

/// Widget that binds directly to one engine node and renders its inspector-style UI.
#[allow(missing_docs)]
#[node("dashboard_node_widget", label = "Node Widget")]
#[children(
    folder(widget, label = "Widget") {
        target_node: NodeReference (
            label = "Target Node",
            description = "Node rendered by this widget.",
            reference_target_kind = crate::parameter::ReferenceTargetKind::AnyNode,
        );
        widget_type: Enum = "inspector" (
            label = "Type",
            description = "Widget rendering type used for the generated node UI.",
            enum_options = ["inspector"],
        );
    }
    folder(layout, label = "Layout") {
        position: Vec2 = (0.0, 0.0) (
            label = "Position",
            description = "Position offset in pixels relative to the selected free-layout anchor.",
            dependency = |node: &Self, ctx: &ProcessCtx| dashboard_parent_layout_is(node.id(), ctx, "free"),
        );
        anchor: Enum = "top-left" (
            label = "Anchor",
            description = "Anchor used to interpret the widget position within free layouts.",
            dependency = |node: &Self, ctx: &ProcessCtx| dashboard_parent_layout_is(node.id(), ctx, "free"),
            enum_options = ["top-left", "top-center", "top-right", "center-left", "center", "center-right", "bottom-left", "bottom-center", "bottom-right"],
        );
        width: CssValue = CssValue::new(12.0, CssUnit::Rem) (
            label = "Width",
            description = "Preferred widget width when the parent layout allows widgets to size horizontally.",
            dependency = |node: &Self, ctx: &ProcessCtx| dashboard_parent_layout_matches(node.id(), ctx, &["free", "horizontal"]),
        );
        height: CssValue = CssValue::new(4.0, CssUnit::Rem) (
            label = "Height",
            description = "Preferred widget height when the parent layout allows widgets to size vertically.",
            dependency = |node: &Self, ctx: &ProcessCtx| dashboard_parent_layout_matches(node.id(), ctx, &["free", "vertical", "grid"]),
        );
        column_span: i32 = 1 [1..64] (
            label = "Column Span",
            description = "Number of grid columns consumed by this widget.",
            dependency = |node: &Self, ctx: &ProcessCtx| dashboard_parent_layout_is(node.id(), ctx, "grid"),
        );
        row_span: i32 = 1 [1..64] (
            label = "Row Span",
            description = "Number of grid rows consumed by this widget.",
            dependency = |node: &Self, ctx: &ProcessCtx| dashboard_parent_layout_is(node.id(), ctx, "grid"),
        );
        locked: bool = false (
            label = "Locked",
            description = "Whether drag-and-drop rebinding is disabled for this widget.",
        );
    }
)]
pub struct DashboardNodeWidgetNode {}

impl DashboardNodeWidgetNode {
    fn sync_parent_layout_dependency_handles(&mut self, ctx: &ProcessCtx) {
        let Some(snapshot) = ctx.tree_snapshot() else {
            return;
        };

        if let Some(position) = snapshot_find_descendant_by_decl(snapshot, self.id(), "position") {
            self.position.set_node_id(position);
        } else {
            self.position.clear_node_id();
        }

        if let Some(column_span) = snapshot_find_descendant_by_decl(snapshot, self.id(), "column_span") {
            self.column_span.set_node_id(column_span);
        } else {
            self.column_span.clear_node_id();
        }

        if let Some(row_span) = snapshot_find_descendant_by_decl(snapshot, self.id(), "row_span") {
            self.row_span.set_node_id(row_span);
        } else {
            self.row_span.clear_node_id();
        }
    }

    fn sync_widget_type_parameter(
        &mut self,
        ctx: &mut ProcessCtx,
        descriptor: &DashboardWidgetTargetDescriptor,
    ) -> String {
        let Some(snapshot) = ctx.tree_snapshot() else {
            return descriptor.default_widget_type_id.clone();
        };
        let Some(widget_type_node) = snapshot_find_descendant_by_decl(snapshot, self.id(), "widget_type") else {
            return descriptor.default_widget_type_id.clone();
        };
        let Some(widget_type_snapshot) = snapshot.node(widget_type_node) else {
            return descriptor.default_widget_type_id.clone();
        };

        let current_value = widget_type_snapshot
            .param_value
            .as_ref()
            .and_then(|value| value.as_enum().or_else(|| value.as_str()))
            .unwrap_or_default();

        let existing_options = widget_type_snapshot
            .param_constraints
            .as_ref()
            .map(|constraints| constraints.enum_options.as_slice())
            .unwrap_or(&[]);
        let options_match = existing_options.len() == descriptor.widget_types.len()
            && existing_options
                .iter()
                .zip(descriptor.widget_types.iter())
                .all(|(option, widget_type)| {
                    option.variant_id == widget_type.id
                        && option.label == widget_type.label
                        && option.value == ParamValue::Enum(widget_type.id.clone())
                });
        let is_initial_placeholder_mode = existing_options.len() == 1
            && existing_options[0].variant_id == "inspector"
            && current_value == "inspector";
        let resolved_value = if !options_match && is_initial_placeholder_mode {
            descriptor.default_widget_type_id.clone()
        } else if descriptor
            .widget_types
            .iter()
            .any(|widget_type| widget_type.id == current_value)
        {
            current_value.clone()
        } else {
            descriptor.default_widget_type_id.clone()
        };

        if options_match && current_value == resolved_value {
            return resolved_value;
        }

        ctx.replace_node(
            widget_type_node,
            make_dashboard_node_widget_type_parameter(resolved_value.clone(), descriptor),
        );
        resolved_value
    }

    fn sync_widget_options_node(&mut self, ctx: &mut ProcessCtx, kind: Option<&DashboardWidgetOptionsNodeKind>) {
        let Some(snapshot) = ctx.tree_snapshot() else {
            return;
        };
        let options_parent = self.id();
        let existing_node = snapshot.find_child(options_parent, "widget_options");

        match kind {
            Some(kind) => {
                let expected_type = dashboard_widget_options_node_type(kind);

                if let Some(existing_node) = existing_node {
                    let existing_matches = snapshot
                        .node(existing_node)
                        .map(|node| node.node_type == expected_type)
                        .unwrap_or(false);
                    if existing_matches {
                        refresh_dashboard_widget_options_node(ctx, existing_node);
                        return;
                    }

                    let previous_sibling = snapshot.previous_sibling(options_parent, existing_node);
                    self.remove_child(ctx, existing_node);

                    let mut new_node = make_dashboard_widget_options_node(kind);
                    new_node.node_data_mut().meta.decl_id = DeclId("widget_options".to_string());
                    ctx.add_child_boxed(options_parent, new_node, previous_sibling);
                } else {
                    let mut new_node = make_dashboard_widget_options_node(kind);
                    new_node.node_data_mut().meta.decl_id = DeclId("widget_options".to_string());
                    ctx.add_child_boxed(options_parent, new_node, None);
                }
            }
            None => {
                if let Some(existing_node) = existing_node {
                    self.remove_child(ctx, existing_node);
                }
            }
        }
    }

    fn sync_target_descriptor(&mut self, ctx: &mut ProcessCtx) {
        let descriptor = dashboard_widget_target_descriptor_for_reference(self.target_node.get_ref(), ctx);
        let widget_type_id = self.sync_widget_type_parameter(ctx, &descriptor);
        let options_node_kind = descriptor
            .widget_type(widget_type_id.as_str())
            .and_then(|widget_type| widget_type.options_node_kind.as_ref());
        self.sync_widget_options_node(ctx, options_node_kind);
    }
}

#[item("dashboard_widget", from_struct)]
impl Node for DashboardNodeWidgetNode {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        enable_dashboard_authoring(self.node_data_mut());
        self.sync_parent_layout_dependency_handles(ctx);
        self.sync_target_descriptor(ctx);
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: crate::parameter::ParamValue) {
        let Some(snapshot) = ctx.tree_snapshot() else {
            return;
        };
        if snapshot_child_decl_matches(snapshot, param, "target_node") {
            self.sync_target_descriptor(ctx);
            return;
        }
        if snapshot_child_decl_matches(snapshot, param, "widget_type") {
            let descriptor = dashboard_widget_target_descriptor_for_reference(self.target_node.get_ref(), ctx);
            let widget_type_id = dashboard_node_widget_current_widget_type(self.id(), ctx)
                .unwrap_or_else(|| descriptor.default_widget_type_id.clone());
            let options_node_kind = descriptor
                .widget_type(widget_type_id.as_str())
                .and_then(|widget_type| widget_type.options_node_kind.as_ref());
            self.sync_widget_options_node(ctx, options_node_kind);
        }
    }

    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        1
    }

    fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
        EventPropagation::Notify
    }
}

/// Configurable widget for canonical controls such as buttons, sliders, text, and checkboxes.
#[allow(missing_docs)]
#[node("dashboard_generic_widget", label = "Generic Widget")]
#[children(
    folder(binding, label = "Binding") {
        target_param: NodeReference (
            label = "Target Parameter",
            description = "Parameter targeted by this widget when it acts as a control or display.",
            reference_target_kind = crate::parameter::ReferenceTargetKind::ParameterOnly,
        );
    }
    folder(appearance, label = "Appearance") {
        label_placement: Enum = "top" (
            label = "Label Placement",
            description = "Where the widget label is rendered. Disable to hide it.",
            enum_options = ["left", "right", "top", "bottom", "inside"],
            can_be_disabled = true,
        );
    }
    folder(layout, label = "Layout") {
        position: Vec2 = (0.0, 0.0) (
            label = "Position",
            description = "Position offset in pixels relative to the selected free-layout anchor.",
            dependency = |node: &Self, ctx: &ProcessCtx| dashboard_parent_layout_is(node.id(), ctx, "free"),
        );
        anchor: Enum = "top-left" (
            label = "Anchor",
            description = "Anchor used to interpret the widget position within free layouts.",
            dependency = |node: &Self, ctx: &ProcessCtx| dashboard_parent_layout_is(node.id(), ctx, "free"),
            enum_options = ["top-left", "top-center", "top-right", "center-left", "center", "center-right", "bottom-left", "bottom-center", "bottom-right"],
        );
        width: CssValue = CssValue::new(10.0, CssUnit::Rem) (
            label = "Width",
            description = "Preferred widget width when the parent layout allows widgets to size horizontally.",
            dependency = |node: &Self, ctx: &ProcessCtx| dashboard_parent_layout_matches(node.id(), ctx, &["free", "horizontal"]),
        );
        height: CssValue = CssValue::new(3.0, CssUnit::Rem) (
            label = "Height",
            description = "Preferred widget height when the parent layout allows widgets to size vertically.",
            dependency = |node: &Self, ctx: &ProcessCtx| dashboard_parent_layout_matches(node.id(), ctx, &["free", "vertical", "grid"]),
        );
        column_span: i32 = 1 [1..64] (
            label = "Column Span",
            description = "Number of grid columns consumed by this widget.",
            dependency = |node: &Self, ctx: &ProcessCtx| dashboard_parent_layout_is(node.id(), ctx, "grid"),
        );
        row_span: i32 = 1 [1..64] (
            label = "Row Span",
            description = "Number of grid rows consumed by this widget.",
            dependency = |node: &Self, ctx: &ProcessCtx| dashboard_parent_layout_is(node.id(), ctx, "grid"),
        );
    }
    folder(content, label = "Content") {
        widget_kind: Enum = "text" (
            label = "Widget Kind",
            description = "Canonical widget family rendered by this node.",
            enum_options = ["text", "button", "slider", "textInput", "checkbox"],
        );
        text: String = "".to_string() (label = "Text", description = "Static text content or button label when applicable.");
        placeholder: String = "".to_string() (
            label = "Placeholder",
            description = "Placeholder text used by text input widgets.",
            dependency = widget_kind == "textInput",
        );
        value_range: Vec2 = (0.0, 1.0) (
            label = "Value Range",
            description = "Minimum and maximum range for slider widgets.",
            dependency = widget_kind == "slider",
        );
        step: f64 = 0.01 [0.0..1000.0] (
            label = "Step",
            description = "Step increment used by slider widgets.",
            dependency = widget_kind == "slider",
        );
        multiline: bool = false (
            label = "Multiline",
            description = "Whether text inputs accept multiple lines.",
            dependency = widget_kind == "textInput",
        );
        default_checked: bool = false (
            label = "Default Checked",
            description = "Default state used by checkbox widgets.",
            dependency = widget_kind == "checkbox",
        );
    }
)]
pub struct DashboardGenericWidgetNode {}

impl DashboardGenericWidgetNode {
    fn sync_parent_layout_dependency_handles(&mut self, ctx: &ProcessCtx) {
        let Some(snapshot) = ctx.tree_snapshot() else {
            return;
        };

        if let Some(position) = snapshot_find_descendant_by_decl(snapshot, self.id(), "position") {
            self.position.set_node_id(position);
        } else {
            self.position.clear_node_id();
        }

        if let Some(column_span) = snapshot_find_descendant_by_decl(snapshot, self.id(), "column_span") {
            self.column_span.set_node_id(column_span);
        } else {
            self.column_span.clear_node_id();
        }

        if let Some(row_span) = snapshot_find_descendant_by_decl(snapshot, self.id(), "row_span") {
            self.row_span.set_node_id(row_span);
        } else {
            self.row_span.clear_node_id();
        }
    }
}

#[item("dashboard_widget", from_struct, scriptable, contextualizable)]
impl Node for DashboardGenericWidgetNode {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        enable_dashboard_authoring(self.node_data_mut());
    }

    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        1
    }

    fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
        EventPropagation::Notify
    }
}

#[cfg(test)]
mod tests;
