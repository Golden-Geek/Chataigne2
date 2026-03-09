#![allow(missing_docs)]

use super::dashboard_widget_options::{dashboard_widget_options_node_type, initialize_replaced_dashboard_widget_options_node, make_dashboard_widget_options_node, refresh_dashboard_widget_options_node};
use crate::events::Event;
use crate::node::{DeclId, EventPropagation, Node, NodeData, NodeId, NodeReference, NodeUserPermissions};
use crate::parameter::{CssUnit, CssValue, Enum, ParamValue, Parameter, ParameterChangeCheck, ParameterEnumOption, Vec2};
use crate::process_ctx::ProcessCtx;
use crate::{item, node};

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
            widget_types: vec![DashboardWidgetTypeSpec::new("inspector", "Inspector").with_options_node_kind(DashboardWidgetOptionsNodeKind::Inspector)],
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

fn snapshot_find_descendant_by_decl(snapshot: &crate::process_ctx::ProcessTreeSnapshot, parent: NodeId, decl_id: &str) -> Option<NodeId> {
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

fn snapshot_child_decl_matches(snapshot: &crate::process_ctx::ProcessTreeSnapshot, node_id: NodeId, decl_id: &str) -> bool {
    snapshot.node(node_id).map(|node| node.decl_id == decl_id || node.decl_id.rsplit('/').next() == Some(decl_id)).unwrap_or(false)
}

fn resolve_snapshot_reference_target(snapshot: &crate::process_ctx::ProcessTreeSnapshot, reference: &NodeReference) -> Option<NodeId> {
    reference.cached_id().filter(|node_id| snapshot.node(*node_id).is_some()).or_else(|| (!reference.uuid().is_nil()).then(|| snapshot.node_id_by_uuid(reference.uuid())).flatten())
}

fn dashboard_parent_layout_kind(node_id: NodeId, ctx: &ProcessCtx) -> Option<String> {
    let snapshot = ctx.tree_snapshot()?;
    let parent_id = snapshot.node(node_id)?.parent?;
    let parent_type = snapshot.node(parent_id)?.node_type.as_str();
    match parent_type {
        DASHBOARD_PAGE_NODE_TYPE | DASHBOARD_WIDGET_CONTAINER_NODE_TYPE => snapshot_find_descendant_by_decl(snapshot, parent_id, "layout_kind")
            .and_then(|layout_kind| snapshot.node(layout_kind))
            .and_then(|layout_kind| layout_kind.param_value.as_ref())
            .and_then(|value| value.as_enum().or_else(|| value.as_str()))
            .map(|value| value.to_string()),
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

fn dashboard_widget_target_descriptor_for_reference(target_reference: &NodeReference, ctx: &ProcessCtx) -> DashboardWidgetTargetDescriptor {
    let Some(snapshot) = ctx.tree_snapshot() else {
        return DashboardWidgetTargetDescriptor::inspector_only();
    };
    let Some(target_id) = resolve_snapshot_reference_target(snapshot, target_reference) else {
        return DashboardWidgetTargetDescriptor::inspector_only();
    };
    snapshot.node(target_id).map(|target| target.dashboard_widget_target.clone()).unwrap_or_else(DashboardWidgetTargetDescriptor::inspector_only)
}

fn dashboard_node_widget_current_widget_type(node_id: NodeId, ctx: &ProcessCtx) -> Option<String> {
    let snapshot = ctx.tree_snapshot()?;
    let widget_type = snapshot_find_descendant_by_decl(snapshot, node_id, "widget_type")?;
    snapshot.node(widget_type)?.param_value.as_ref().and_then(|value| value.as_enum().or_else(|| value.as_str()))
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

fn make_dashboard_node_widget_type_parameter(value: impl Into<String>, descriptor: &DashboardWidgetTargetDescriptor) -> Parameter {
    let value = value.into();
    let mut parameter = Parameter::new("Type", ParamValue::Enum(value), ParameterChangeCheck::ValueChange);
    parameter.node_data_mut().meta.decl_id = DeclId("widget_type".to_string());
    parameter.node_data_mut().meta.description = Some("Widget rendering type used for the generated node UI.".to_string());
    parameter.constraints.enum_options = descriptor.widget_types.iter().enumerate().map(|(index, widget_type)| dashboard_widget_type_enum_option(widget_type, index as i32)).collect();
    parameter
}

fn refresh_dashboard_widget_dependencies(ctx: &mut ProcessCtx, widget: NodeId) {
    let Some(node_type) = ctx.tree_snapshot().and_then(|snapshot| snapshot.node(widget)).map(|snapshot| snapshot.node_type.clone()) else {
        return;
    };

    match node_type.as_str() {
        DASHBOARD_WIDGET_CONTAINER_NODE_TYPE => crate::node::NodeHandle::new(widget).with_mut::<DashboardWidgetContainerNode, _>(ctx, |node, child_ctx| {
            node.sync_parent_layout_dependency_handles(child_ctx);
            node.__golden_node_engine_preprocess_inbox(child_ctx, node.id());
        }),
        DASHBOARD_NODE_WIDGET_NODE_TYPE => crate::node::NodeHandle::new(widget).with_mut::<DashboardNodeWidgetNode, _>(ctx, |node, child_ctx| {
            node.sync_parent_layout_dependency_handles(child_ctx);
            node.__golden_node_engine_preprocess_inbox(child_ctx, node.id());
        }),
        DASHBOARD_GENERIC_WIDGET_NODE_TYPE => crate::node::NodeHandle::new(widget).with_mut::<DashboardGenericWidgetNode, _>(ctx, |node, child_ctx| {
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
#[node("dashboard")]
pub struct DashboardNode {}

#[node("dashboard", from_struct)]
impl Node for DashboardNode {
    crate::define_user_item_factory_methods! {
        accepts = ["dashboard_page"];
        items = [
            {
                node_type: "dashboard_page",
                item_kind: "dashboard_page",
                label: "Dashboard Page",
                create: |_: &Self, label: String| DashboardPageNode::new(label),
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
#[node("dashboard_page")]
#[children(
    route: String = "".to_string() (label = "Route", description = "Stable route or slug used to address this page.");
    page_width: CssValue = CssValue::new(1920.0, CssUnit::Px) (
        label = "Page Width",
        description = "Logical page width used by the dashboard panel viewer to preserve aspect ratio. Disable to let the page fill the panel.",
        enabled = false,
        can_be_disabled = true,
    );
    page_height: CssValue = CssValue::new(1080.0, CssUnit::Px) (
        label = "Page Height",
        description = "Logical page height used by the dashboard panel viewer to preserve aspect ratio. Disable to let the page fill the panel.",
        enabled = false,
        can_be_disabled = true,
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
                node_type: "dashboard_widget_container",
                item_kind: "dashboard_widget",
                label: "Container Widget",
                create: |_: &Self, label: String| DashboardWidgetContainerNode::new(label),
            },
            {
                node_type: "dashboard_node_widget",
                item_kind: "dashboard_widget",
                label: "Node Widget",
                create: |_: &Self, label: String| DashboardNodeWidgetNode::new(label),
            },
            {
                node_type: "dashboard_generic_widget",
                item_kind: "dashboard_widget",
                label: "Generic Widget",
                create: |_: &Self, label: String| DashboardGenericWidgetNode::new(label),
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
#[node("dashboard_widget_container")]
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
                node_type: "dashboard_widget_container",
                item_kind: "dashboard_widget",
                label: "Container Widget",
                create: |_: &Self, label: String| DashboardWidgetContainerNode::new(label),
            },
            {
                node_type: "dashboard_node_widget",
                item_kind: "dashboard_widget",
                label: "Node Widget",
                create: |_: &Self, label: String| DashboardNodeWidgetNode::new(label),
            },
            {
                node_type: "dashboard_generic_widget",
                item_kind: "dashboard_widget",
                label: "Generic Widget",
                create: |_: &Self, label: String| DashboardGenericWidgetNode::new(label),
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
#[node("dashboard_node_widget")]
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

    fn sync_widget_type_parameter(&mut self, ctx: &mut ProcessCtx, descriptor: &DashboardWidgetTargetDescriptor) -> String {
        let Some(snapshot) = ctx.tree_snapshot() else {
            return descriptor.default_widget_type_id.clone();
        };
        let Some(widget_type_node) = snapshot_find_descendant_by_decl(snapshot, self.id(), "widget_type") else {
            return descriptor.default_widget_type_id.clone();
        };
        let Some(widget_type_snapshot) = snapshot.node(widget_type_node) else {
            return descriptor.default_widget_type_id.clone();
        };

        let current_value = widget_type_snapshot.param_value.as_ref().and_then(|value| value.as_enum().or_else(|| value.as_str())).unwrap_or_default();

        let existing_options = widget_type_snapshot.param_constraints.as_ref().map(|constraints| constraints.enum_options.as_slice()).unwrap_or(&[]);
        let options_match = existing_options.len() == descriptor.widget_types.len()
            && existing_options
                .iter()
                .zip(descriptor.widget_types.iter())
                .all(|(option, widget_type)| option.variant_id == widget_type.id && option.label == widget_type.label && option.value == ParamValue::Enum(widget_type.id.clone()));
        let is_initial_placeholder_mode = existing_options.len() == 1 && existing_options[0].variant_id == "inspector" && current_value == "inspector";
        let resolved_value = if !options_match && is_initial_placeholder_mode {
            descriptor.default_widget_type_id.clone()
        } else if descriptor.widget_types.iter().any(|widget_type| widget_type.id == current_value) {
            current_value.clone()
        } else {
            descriptor.default_widget_type_id.clone()
        };

        if options_match && current_value == resolved_value {
            return resolved_value;
        }

        ctx.replace_node(widget_type_node, make_dashboard_node_widget_type_parameter(resolved_value.clone(), descriptor));
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
                    let existing_matches = snapshot.node(existing_node).map(|node| node.node_type == expected_type).unwrap_or(false);
                    if existing_matches {
                        refresh_dashboard_widget_options_node(ctx, existing_node);
                        return;
                    }

                    for child_id in snapshot.child_ids(existing_node) {
                        ctx.edits.push(crate::edit::Edit::RemoveNode { node: child_id });
                    }
                    ctx.replace_node_boxed(existing_node, make_dashboard_widget_options_node(kind));
                    initialize_replaced_dashboard_widget_options_node(ctx, existing_node);
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
        let options_node_kind = descriptor.widget_type(widget_type_id.as_str()).and_then(|widget_type| widget_type.options_node_kind.as_ref());
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
            let widget_type_id = dashboard_node_widget_current_widget_type(self.id(), ctx).unwrap_or_else(|| descriptor.default_widget_type_id.clone());
            let options_node_kind = descriptor.widget_type(widget_type_id.as_str()).and_then(|widget_type| widget_type.options_node_kind.as_ref());
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
#[node("dashboard_generic_widget")]
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
mod tests {
    use std::time::Duration;

    use crate::define_node_enum;
    use crate::edit::Edit;
    use crate::engine::Engine;
    use crate::node::{DeclId, Folder, Node, NodeId, NodeReference};
    use crate::parameter::{CssUnit, CssValue, ParamValue, Parameter, ParameterChangeCheck, ParameterEventBehaviour, ParameterSnapshot, RangeConstraint, ReferenceTargetKind};
    use crate::ui_sync::{UiCreateUserItemInitialParam, UiEditIntent};

    use super::{DASHBOARD_GENERIC_WIDGET_NODE_TYPE, DASHBOARD_NODE_TYPE, DASHBOARD_NODE_WIDGET_NODE_TYPE, DASHBOARD_PAGE_NODE_TYPE, DASHBOARD_WIDGET_CONTAINER_NODE_TYPE, DashboardNode};

    define_node_enum!(
        enum DashboardTestNode {}
    );

    fn first_child<T: Node>(engine: &Engine<T>, parent: NodeId) -> NodeId {
        engine.nodes.get(parent).and_then(|node| node.node_data().first_child).expect("parent should have one child")
    }

    fn direct_child_by_type<T: Node>(engine: &Engine<T>, parent: NodeId, node_type: &str) -> Option<NodeId> {
        let mut child = engine.nodes.get(parent).and_then(|node| node.node_data().first_child);
        while let Some(child_id) = child {
            let child_node = engine.nodes.get(child_id)?;
            if child_node.get_type() == node_type {
                return Some(child_id);
            }
            child = child_node.node_data().next_sibling;
        }
        None
    }

    fn find_descendant_by_decl<T: Node>(engine: &Engine<T>, parent: NodeId, decl_id: &str) -> Option<NodeId> {
        let mut stack = vec![parent];
        while let Some(node_id) = stack.pop() {
            let Some(node) = engine.nodes.get(node_id) else {
                continue;
            };

            let mut child = node.node_data().first_child;
            while let Some(child_id) = child {
                let Some(child_node) = engine.nodes.get(child_id) else {
                    break;
                };
                let child_decl_id = child_node.node_data().meta.decl_id.0.as_str();
                if child_decl_id == decl_id || child_decl_id.rsplit('/').next() == Some(decl_id) {
                    return Some(child_id);
                }
                stack.push(child_id);
                child = child_node.node_data().next_sibling;
            }
        }
        None
    }

    fn param_snapshot<T: Node>(engine: &Engine<T>, node_id: NodeId) -> ParameterSnapshot {
        engine.nodes.get(node_id).and_then(Node::engine_param_snapshot).expect("node should expose a parameter snapshot")
    }

    fn direct_child_decl_ids<T: Node>(engine: &Engine<T>, parent: NodeId) -> Vec<String> {
        let mut decl_ids = Vec::new();
        let mut child = engine.nodes.get(parent).and_then(|node| node.node_data().first_child);
        while let Some(child_id) = child {
            let child_node = engine.nodes.get(child_id).expect("child should exist");
            decl_ids.push(child_node.node_data().meta.decl_id.0.clone());
            child = child_node.node_data().next_sibling;
        }
        decl_ids
    }

    #[test]
    fn dashboard_catalog_exposes_pages_and_widgets() {
        let root: DashboardTestNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);

        engine.add_node(DashboardNode::new("Dashboard").into(), None);
        engine.apply_edits().expect("dashboard creation should apply");

        let dashboard = first_child(&engine, engine.root);
        assert_eq!(engine.nodes.get(dashboard).expect("dashboard should exist").get_type(), DASHBOARD_NODE_TYPE);

        let dashboard_creatable = engine.catalog_creatable_items(dashboard);
        assert!(dashboard_creatable.iter().any(|item| item.node_type == DASHBOARD_PAGE_NODE_TYPE), "dashboard should expose page creation");

        engine.queue_catalog_create(dashboard, DASHBOARD_PAGE_NODE_TYPE, Some("Main".to_string()), None).expect("page creation should queue");
        engine.apply_edits().expect("page creation should apply");

        let page = first_child(&engine, dashboard);
        let page_creatable = engine.catalog_creatable_items(page);
        assert!(page_creatable.iter().any(|item| item.node_type == DASHBOARD_WIDGET_CONTAINER_NODE_TYPE), "page should expose container widget creation");
        assert!(page_creatable.iter().any(|item| item.node_type == DASHBOARD_NODE_WIDGET_NODE_TYPE), "page should expose node widget creation");
        assert!(page_creatable.iter().any(|item| item.node_type == DASHBOARD_GENERIC_WIDGET_NODE_TYPE), "page should expose generic widget creation");
    }

    #[test]
    fn dashboard_widgets_materialize_binding_constraints() {
        let root: DashboardTestNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);

        engine.add_node(DashboardNode::new("Dashboard").into(), None);
        engine.apply_edits().expect("dashboard creation should apply");

        let dashboard = first_child(&engine, engine.root);
        engine.queue_catalog_create(dashboard, DASHBOARD_PAGE_NODE_TYPE, Some("Main".to_string()), None).expect("page creation should queue");
        engine.apply_edits().expect("page creation should apply");

        let page = first_child(&engine, dashboard);
        engine.queue_catalog_create(page, DASHBOARD_NODE_WIDGET_NODE_TYPE, Some("Auto UI".to_string()), None).expect("node widget creation should queue");
        engine.queue_catalog_create(page, DASHBOARD_GENERIC_WIDGET_NODE_TYPE, Some("Button".to_string()), None).expect("generic widget creation should queue");
        engine.apply_edits().expect("widget creation should apply");

        let node_widget = direct_child_by_type(&engine, page, DASHBOARD_NODE_WIDGET_NODE_TYPE).expect("page should contain a node widget child");
        let generic_widget = direct_child_by_type(&engine, page, DASHBOARD_GENERIC_WIDGET_NODE_TYPE).expect("page should contain a generic widget child");

        let target_node_param = find_descendant_by_decl(&engine, node_widget, "target_node").expect("node widget target reference should exist");
        let target_param_param = find_descendant_by_decl(&engine, generic_widget, "target_param").expect("generic widget target reference should exist");

        assert_eq!(param_snapshot(&engine, target_node_param).constraints.reference.target_kind, ReferenceTargetKind::AnyNode, "node widgets should accept references to any node");
        assert_eq!(param_snapshot(&engine, target_param_param).constraints.reference.target_kind, ReferenceTargetKind::ParameterOnly, "generic widgets should bind to parameters by default");
    }

    #[test]
    fn dashboard_node_widget_types_follow_target_capabilities() {
        let root: DashboardTestNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);

        let mut speed = Parameter::new("Speed", ParamValue::Float(0.5), ParameterChangeCheck::ValueChange);
        speed.constraints.range = RangeConstraint::uniform(Some(0.0), Some(1.0));
        engine.add_node(speed.into(), None);
        engine.add_node(Parameter::new("Point", ParamValue::Vec2(0.0, 0.0), ParameterChangeCheck::ValueChange).into(), None);
        engine.add_node(DashboardNode::new("Dashboard").into(), None);
        engine.apply_edits().expect("initial tree should apply");

        let float_param = direct_child_by_type(&engine, engine.root, "float").expect("float parameter should exist");
        let vec2_param = direct_child_by_type(&engine, engine.root, "vec2").expect("vec2 parameter should exist");
        let dashboard = direct_child_by_type(&engine, engine.root, DASHBOARD_NODE_TYPE).expect("dashboard should exist");
        engine.queue_catalog_create(dashboard, DASHBOARD_PAGE_NODE_TYPE, Some("Main".to_string()), None).expect("page creation should queue");
        engine.apply_edits().expect("page creation should apply");

        let page = first_child(&engine, dashboard);
        engine.queue_catalog_create(page, DASHBOARD_NODE_WIDGET_NODE_TYPE, Some("Auto UI".to_string()), None).expect("node widget creation should queue");
        for _ in 0..3 {
            engine.apply_edits().expect("widget creation should apply");
            engine.dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");
        }

        let widget = direct_child_by_type(&engine, page, DASHBOARD_NODE_WIDGET_NODE_TYPE).expect("page should contain a node widget child");
        assert_eq!(direct_child_decl_ids(&engine, widget), vec!["widget", "layout", "widget_options"], "node widgets should expose widget, layout, and widget options nodes in direct-child order");
        let widget_type = find_descendant_by_decl(&engine, widget, "widget_type").expect("widget type parameter should exist");
        let target_node = find_descendant_by_decl(&engine, widget, "target_node").expect("target node parameter should exist");
        let max_child_level = find_descendant_by_decl(&engine, widget, "max_child_level").expect("inspector widgets should expose max child level");
        let label_placement = find_descendant_by_decl(&engine, widget, "label_placement").expect("inspector widgets should expose label placement inside widget options");

        let initial_widget_type = param_snapshot(&engine, widget_type);
        let initial_types = initial_widget_type.constraints.enum_options.iter().map(|option| option.variant_id.as_str()).collect::<Vec<_>>();
        assert_eq!(initial_types, vec!["inspector"], "unbound widgets should expose inspector only");
        assert_eq!(initial_widget_type.value, ParamValue::Enum("inspector".to_string()), "unbound widgets should default to inspector");
        assert_eq!(param_snapshot(&engine, max_child_level).value, ParamValue::Int(2), "inspector widgets should default to a child depth of two levels");
        assert_eq!(param_snapshot(&engine, label_placement).value, ParamValue::Enum("inside".to_string()), "node widget label placement should now live under widget options");
        assert!(find_descendant_by_decl(&engine, widget, "include_children").is_none(), "legacy include children should be removed from inspector widget options");

        let float_uuid = engine.nodes.get(float_param).expect("float parameter should exist").node_data().meta.uuid;
        engine.edits.push(Edit::SetParam {
            node: target_node,
            value: ParamValue::Reference(NodeReference::new(float_uuid)),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        for _ in 0..4 {
            engine.apply_edits().expect("binding float target should apply");
            engine.dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");
        }

        let float_widget_type = param_snapshot(&engine, widget_type);
        let float_types = float_widget_type.constraints.enum_options.iter().map(|option| (option.variant_id.as_str(), option.label.as_str())).collect::<Vec<_>>();
        assert_eq!(
            float_types,
            vec![("default", "Default"), ("inspector", "Inspector"), ("slider", "Slider"), ("rotary", "Rotary"),],
            "numeric parameters should expose default, inspector, slider, and rotary widget types",
        );
        assert_eq!(float_widget_type.value, ParamValue::Enum("default".to_string()), "parameter widgets should default to the semantic default widget type when first bound",);
        assert!(find_descendant_by_decl(&engine, widget, "slider_show_value_field").is_some(), "default numeric widgets should materialize slider option nodes",);
        assert!(find_descendant_by_decl(&engine, widget, "custom_range").is_some(), "default numeric widgets should expose the custom range parameter",);
        assert!(find_descendant_by_decl(&engine, widget, "rotary_show_value_field").is_none(), "default numeric widgets should not materialize rotary options",);
        assert!(find_descendant_by_decl(&engine, widget, "widget_options").is_some(), "widget options should remain mounted directly under the node widget",);

        engine.edits.push(Edit::SetParam {
            node: widget_type,
            value: ParamValue::Enum("rotary".to_string()),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        for _ in 0..4 {
            engine.apply_edits().expect("switching float widget to rotary should apply");
            engine.dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");
        }

        assert!(find_descendant_by_decl(&engine, widget, "slider_show_value_field").is_none(), "switching to rotary should replace the slider options node",);
        assert!(find_descendant_by_decl(&engine, widget, "rotary_show_value_field").is_some(), "switching to rotary should materialize rotary-specific options",);
        assert!(find_descendant_by_decl(&engine, widget, "custom_range").is_some(), "rotary widgets should expose the custom range parameter",);

        let vec2_uuid = engine.nodes.get(vec2_param).expect("vec2 parameter should exist").node_data().meta.uuid;
        engine.edits.push(Edit::SetParam {
            node: target_node,
            value: ParamValue::Reference(NodeReference::new(vec2_uuid)),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        for _ in 0..4 {
            engine.apply_edits().expect("binding vec2 target should apply");
            engine.dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");
        }

        let vec2_widget_type = param_snapshot(&engine, widget_type);
        let vec2_types = vec2_widget_type.constraints.enum_options.iter().map(|option| (option.variant_id.as_str(), option.label.as_str())).collect::<Vec<_>>();
        assert_eq!(vec2_types, vec![("default", "Default"), ("inspector", "Inspector"), ("vec2Pad", "2D Pad"),], "vec2 parameters should expose the default and 2D Pad widget types",);
        assert_eq!(vec2_widget_type.value, ParamValue::Enum("default".to_string()), "unsupported widget types should reset to the target default when rebinding",);
        assert!(find_descendant_by_decl(&engine, widget, "vector_layout").is_some(), "rebinding to vec2 should replace rotary options with vector-editor options",);
        assert!(find_descendant_by_decl(&engine, widget, "custom_range").is_some(), "vec2 widgets should expose the custom range folder",);
        assert!(find_descendant_by_decl(&engine, widget, "custom_range/range_min").is_some(), "vec2 widgets should expose vector custom range bounds",);
        assert!(find_descendant_by_decl(&engine, widget, "rotary_show_value_field").is_none(), "rebinding away from numeric rotary mode should remove rotary options",);

        engine.edits.push(Edit::SetParam {
            node: widget_type,
            value: ParamValue::Enum("vec2Pad".to_string()),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        for _ in 0..4 {
            engine.apply_edits().expect("switching vec2 widget to 2D pad should apply");
            engine.dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");
        }

        assert!(find_descendant_by_decl(&engine, widget, "trail_time").is_some(), "vec2 pad widgets should expose trail time controls",);
        assert!(find_descendant_by_decl(&engine, widget, "custom_range").is_some(), "vec2 pad widgets should keep the custom range folder available",);
        assert!(find_descendant_by_decl(&engine, widget, "custom_range/range_min").is_some(), "vec2 pad widgets should keep custom range bounds available",);
        assert!(find_descendant_by_decl(&engine, widget, "vector_layout").is_none(), "vec2 pad widgets should hide vector-editor-only layout options",);
    }

    #[test]
    fn dashboard_widget_layout_dependencies_follow_parent_layout() {
        let root: DashboardTestNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);

        engine.add_node(DashboardNode::new("Dashboard").into(), None);
        engine.apply_edits().expect("dashboard creation should apply");

        let dashboard = first_child(&engine, engine.root);
        engine.queue_catalog_create(dashboard, DASHBOARD_PAGE_NODE_TYPE, Some("Main".to_string()), None).expect("page creation should queue");
        engine.apply_edits().expect("page creation should apply");

        let page = first_child(&engine, dashboard);
        engine.queue_catalog_create(page, DASHBOARD_GENERIC_WIDGET_NODE_TYPE, Some("Widget".to_string()), None).expect("generic widget creation should queue");
        for _ in 0..3 {
            engine.apply_edits().expect("widget creation should apply");
            engine.dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");
        }

        let widget = direct_child_by_type(&engine, page, DASHBOARD_GENERIC_WIDGET_NODE_TYPE).expect("page should contain a generic widget child");
        let page_layout = find_descendant_by_decl(&engine, page, "layout_kind").expect("page layout parameter should exist");

        assert!(find_descendant_by_decl(&engine, widget, "position").is_some(), "free-layout widgets should expose position");
        assert!(find_descendant_by_decl(&engine, widget, "column_span").is_none(), "free-layout widgets should hide grid-only spans");
        assert!(find_descendant_by_decl(&engine, widget, "row_span").is_none(), "free-layout widgets should hide grid-only spans");

        engine.edits.push(Edit::SetParam {
            node: page_layout,
            value: ParamValue::Enum("grid".to_string()),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        for _ in 0..3 {
            engine.apply_edits().expect("switching page layout should apply");
            engine.dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");
        }

        assert!(find_descendant_by_decl(&engine, widget, "position").is_none(), "grid-layout widgets should hide free-layout position");
        assert!(find_descendant_by_decl(&engine, widget, "column_span").is_some(), "grid-layout widgets should expose column span");
        assert!(find_descendant_by_decl(&engine, widget, "row_span").is_some(), "grid-layout widgets should expose row span");

        engine.edits.push(Edit::SetParam {
            node: page_layout,
            value: ParamValue::Enum("free".to_string()),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        for _ in 0..3 {
            engine.apply_edits().expect("switching page layout back should apply");
            engine.dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");
        }

        assert_eq!(direct_child_decl_ids(&engine, widget), vec!["binding", "appearance", "layout", "content"], "reintroduced free-layout parameters should return to declared order",);
    }

    #[test]
    fn dashboard_generic_widget_kind_dependencies_follow_widget_kind() {
        let root: DashboardTestNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);

        engine.add_node(DashboardNode::new("Dashboard").into(), None);
        engine.apply_edits().expect("dashboard creation should apply");

        let dashboard = first_child(&engine, engine.root);
        engine.queue_catalog_create(dashboard, DASHBOARD_PAGE_NODE_TYPE, Some("Main".to_string()), None).expect("page creation should queue");
        engine.apply_edits().expect("page creation should apply");

        let page = first_child(&engine, dashboard);
        engine.queue_catalog_create(page, DASHBOARD_GENERIC_WIDGET_NODE_TYPE, Some("Widget".to_string()), None).expect("generic widget creation should queue");
        for _ in 0..3 {
            engine.apply_edits().expect("widget creation should apply");
            engine.dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");
        }

        let widget = direct_child_by_type(&engine, page, DASHBOARD_GENERIC_WIDGET_NODE_TYPE).expect("page should contain a generic widget child");
        let widget_kind = find_descendant_by_decl(&engine, widget, "widget_kind").expect("widget kind parameter should exist");

        assert!(find_descendant_by_decl(&engine, widget, "placeholder").is_none(), "text widgets should hide text-input placeholder");
        assert!(find_descendant_by_decl(&engine, widget, "multiline").is_none(), "text widgets should hide multiline");
        assert!(find_descendant_by_decl(&engine, widget, "value_range").is_none(), "text widgets should hide slider range");
        assert!(find_descendant_by_decl(&engine, widget, "step").is_none(), "text widgets should hide slider step");
        assert!(find_descendant_by_decl(&engine, widget, "default_checked").is_none(), "text widgets should hide checkbox defaults");

        engine.edits.push(Edit::SetParam {
            node: widget_kind,
            value: ParamValue::Enum("textInput".to_string()),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        for _ in 0..3 {
            engine.apply_edits().expect("switching widget kind to text input should apply");
            engine.dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");
        }

        assert!(find_descendant_by_decl(&engine, widget, "placeholder").is_some(), "text-input widgets should expose placeholder");
        assert!(find_descendant_by_decl(&engine, widget, "multiline").is_some(), "text-input widgets should expose multiline");
        assert!(find_descendant_by_decl(&engine, widget, "value_range").is_none(), "text-input widgets should hide slider range");
        assert!(find_descendant_by_decl(&engine, widget, "step").is_none(), "text-input widgets should hide slider step");
        assert!(find_descendant_by_decl(&engine, widget, "default_checked").is_none(), "text-input widgets should hide checkbox defaults");

        engine.edits.push(Edit::SetParam {
            node: widget_kind,
            value: ParamValue::Enum("slider".to_string()),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        for _ in 0..3 {
            engine.apply_edits().expect("switching widget kind to slider should apply");
            engine.dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");
        }

        assert!(find_descendant_by_decl(&engine, widget, "placeholder").is_none(), "slider widgets should hide text-input placeholder");
        assert!(find_descendant_by_decl(&engine, widget, "multiline").is_none(), "slider widgets should hide multiline");
        assert!(find_descendant_by_decl(&engine, widget, "value_range").is_some(), "slider widgets should expose range");
        assert!(find_descendant_by_decl(&engine, widget, "step").is_some(), "slider widgets should expose step");
        assert!(find_descendant_by_decl(&engine, widget, "default_checked").is_none(), "slider widgets should hide checkbox defaults");

        engine.edits.push(Edit::SetParam {
            node: widget_kind,
            value: ParamValue::Enum("checkbox".to_string()),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        for _ in 0..3 {
            engine.apply_edits().expect("switching widget kind to checkbox should apply");
            engine.dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");
        }

        assert!(find_descendant_by_decl(&engine, widget, "default_checked").is_some(), "checkbox widgets should expose default checked");
        assert!(find_descendant_by_decl(&engine, widget, "placeholder").is_none(), "checkbox widgets should hide text-input placeholder");
        assert!(find_descendant_by_decl(&engine, widget, "value_range").is_none(), "checkbox widgets should hide slider range");
        assert!(find_descendant_by_decl(&engine, widget, "step").is_none(), "checkbox widgets should hide slider step");
    }

    #[test]
    fn dashboard_widgets_expose_disableable_label_placement() {
        let root: DashboardTestNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);

        engine.add_node(DashboardNode::new("Dashboard").into(), None);
        engine.apply_edits().expect("dashboard creation should apply");

        let dashboard = first_child(&engine, engine.root);
        engine.queue_catalog_create(dashboard, DASHBOARD_PAGE_NODE_TYPE, Some("Main".to_string()), None).expect("page creation should queue");
        engine.apply_edits().expect("page creation should apply");

        let page = first_child(&engine, dashboard);
        engine.queue_catalog_create(page, DASHBOARD_GENERIC_WIDGET_NODE_TYPE, Some("Widget".to_string()), None).expect("generic widget creation should queue");
        for _ in 0..3 {
            engine.apply_edits().expect("widget creation should apply");
            engine.dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");
        }

        let widget = direct_child_by_type(&engine, page, DASHBOARD_GENERIC_WIDGET_NODE_TYPE).expect("page should contain a generic widget child");
        let label_placement = find_descendant_by_decl(&engine, widget, "label_placement").expect("label placement parameter should exist");
        let label_placement_node = engine.nodes.get(label_placement).expect("label placement node should exist");

        assert_eq!(param_snapshot(&engine, label_placement).value, ParamValue::Enum("top".to_string()));
        assert!(label_placement_node.node_data().meta.enabled, "label placement should start enabled");
        assert!(label_placement_node.node_data().meta.can_be_disabled, "label placement should be disableable so hidden remains available");
    }

    #[test]
    fn dashboard_node_widget_inspector_options_follow_target_disable_capabilities() {
        let root: DashboardTestNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);

        let mut target = Parameter::new("Toggleable", ParamValue::Str("Value".to_string()), ParameterChangeCheck::ValueChange);
        target.node_data_mut().meta.can_be_disabled = true;
        engine.add_node(target.into(), None);
        engine.add_node(DashboardNode::new("Dashboard").into(), None);
        engine.apply_edits().expect("initial tree should apply");

        let dashboard = direct_child_by_type(&engine, engine.root, DASHBOARD_NODE_TYPE).expect("dashboard should exist");
        let target_node = {
            let mut child = engine.nodes.get(engine.root).and_then(|node| node.node_data().first_child);
            let mut found = None;
            while let Some(child_id) = child {
                let child_node = engine.nodes.get(child_id).expect("child should exist");
                if child_node.get_type() != DASHBOARD_NODE_TYPE {
                    found = Some(child_id);
                    break;
                }
                child = child_node.node_data().next_sibling;
            }
            found.expect("disableable target parameter should exist")
        };
        engine.queue_catalog_create(dashboard, DASHBOARD_PAGE_NODE_TYPE, Some("Main".to_string()), None).expect("page creation should queue");
        engine.apply_edits().expect("page creation should apply");

        let page = first_child(&engine, dashboard);
        engine.queue_catalog_create(page, DASHBOARD_NODE_WIDGET_NODE_TYPE, Some("Inspector".to_string()), None).expect("node widget creation should queue");
        for _ in 0..3 {
            engine.apply_edits().expect("widget creation should apply");
            engine.dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");
        }

        let widget = direct_child_by_type(&engine, page, DASHBOARD_NODE_WIDGET_NODE_TYPE).expect("page should contain a node widget child");
        let widget_target = find_descendant_by_decl(&engine, widget, "target_node").expect("target node parameter should exist");
        let widget_type = find_descendant_by_decl(&engine, widget, "widget_type").expect("widget type parameter should exist");
        let target_uuid = engine.nodes.get(target_node).expect("target node should exist").node_data().meta.uuid;

        engine.edits.push(Edit::SetParam {
            node: widget_target,
            value: ParamValue::Reference(NodeReference::new(target_uuid)),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        for _ in 0..4 {
            engine.apply_edits().expect("binding inspector target should apply");
            engine.dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");
        }

        let show_enable_button = find_descendant_by_decl(&engine, widget, "show_enable_button").expect("disableable parameter widgets should expose the enable-button toggle in their default widget options");
        assert_eq!(param_snapshot(&engine, show_enable_button).value, ParamValue::Bool(true), "disableable parameter widgets should show the enable button by default");

        engine.edits.push(Edit::SetParam {
            node: widget_type,
            value: ParamValue::Enum("inspector".to_string()),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        for _ in 0..4 {
            engine.apply_edits().expect("switching widget to inspector should apply");
            engine.dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");
        }

        let show_enable_button = find_descendant_by_decl(&engine, widget, "show_enable_button").expect("disableable inspector targets should expose the enable-button toggle");
        assert_eq!(param_snapshot(&engine, show_enable_button).value, ParamValue::Bool(true), "disableable inspector targets should show the enable button by default");
    }

    #[test]
    fn ui_create_user_item_initial_params_materialize_dashboard_widget_without_follow_up_edits() {
        let root: DashboardTestNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);

        engine.add_node(Parameter::new("Tempo", ParamValue::Float(120.0), ParameterChangeCheck::ValueChange).into(), None);
        engine.add_node(DashboardNode::new("Dashboard").into(), None);
        engine.apply_edits().expect("initial tree should apply");

        let target_param = direct_child_by_type(&engine, engine.root, "float").expect("target parameter should exist");
        let dashboard = direct_child_by_type(&engine, engine.root, DASHBOARD_NODE_TYPE).expect("dashboard should exist");
        engine.queue_catalog_create(dashboard, DASHBOARD_PAGE_NODE_TYPE, Some("Main".to_string()), None).expect("page creation should queue");
        engine.apply_edits().expect("page creation should apply");

        let page = first_child(&engine, dashboard);
        let target_uuid = engine.nodes.get(target_param).expect("target parameter should exist").node_data().meta.uuid;
        let create_ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
            parent: page,
            node_type: DASHBOARD_GENERIC_WIDGET_NODE_TYPE.to_string(),
            label: Some("Tempo Widget".to_string()),
            initial_params: vec![
                UiCreateUserItemInitialParam {
                    decl_id: DeclId("binding/target_param".to_string()),
                    value: ParamValue::Reference(NodeReference::new(target_uuid)),
                },
                UiCreateUserItemInitialParam {
                    decl_id: DeclId("content/widget_kind".to_string()),
                    value: ParamValue::Enum("textInput".to_string()),
                },
                UiCreateUserItemInitialParam {
                    decl_id: DeclId("content/placeholder".to_string()),
                    value: ParamValue::Str("Tempo".to_string()),
                },
                UiCreateUserItemInitialParam {
                    decl_id: DeclId("content/multiline".to_string()),
                    value: ParamValue::Bool(true),
                },
                UiCreateUserItemInitialParam {
                    decl_id: DeclId("layout/anchor".to_string()),
                    value: ParamValue::Enum("center".to_string()),
                },
                UiCreateUserItemInitialParam {
                    decl_id: DeclId("layout/position".to_string()),
                    value: ParamValue::Vec2(240.0, 160.0),
                },
                UiCreateUserItemInitialParam {
                    decl_id: DeclId("layout/width".to_string()),
                    value: ParamValue::CssValue(CssValue::new(18.0, CssUnit::Rem)),
                },
                UiCreateUserItemInitialParam {
                    decl_id: DeclId("layout/height".to_string()),
                    value: ParamValue::CssValue(CssValue::new(6.0, CssUnit::Rem)),
                },
            ],
        });
        assert!(create_ack.success, "configured widget creation should succeed");

        let widget = direct_child_by_type(&engine, page, DASHBOARD_GENERIC_WIDGET_NODE_TYPE).expect("page should contain the created generic widget");
        let widget_kind = find_descendant_by_decl(&engine, widget, "widget_kind").expect("widget kind parameter should exist");
        let placeholder = find_descendant_by_decl(&engine, widget, "placeholder").expect("placeholder parameter should exist");
        let multiline = find_descendant_by_decl(&engine, widget, "multiline").expect("multiline parameter should exist");
        let anchor = find_descendant_by_decl(&engine, widget, "anchor").expect("anchor parameter should exist");
        let position = find_descendant_by_decl(&engine, widget, "position").expect("position parameter should exist");
        let width = find_descendant_by_decl(&engine, widget, "width").expect("width parameter should exist");
        let height = find_descendant_by_decl(&engine, widget, "height").expect("height parameter should exist");

        assert_eq!(param_snapshot(&engine, widget_kind).value, ParamValue::Enum("textInput".to_string()));
        assert_eq!(param_snapshot(&engine, placeholder).value, ParamValue::Str("Tempo".to_string()));
        assert_eq!(param_snapshot(&engine, multiline).value, ParamValue::Bool(true));
        assert_eq!(param_snapshot(&engine, anchor).value, ParamValue::Enum("center".to_string()));
        assert_eq!(param_snapshot(&engine, position).value, ParamValue::Vec2(240.0, 160.0));
        assert_eq!(param_snapshot(&engine, width).value, ParamValue::CssValue(CssValue::new(18.0, CssUnit::Rem)));
        assert_eq!(param_snapshot(&engine, height).value, ParamValue::CssValue(CssValue::new(6.0, CssUnit::Rem)));
    }

    #[test]
    fn dashboard_node_widget_initial_target_reference_without_cached_id_stabilizes() {
        let root: DashboardTestNode = Folder::new("Root").into();
        let mut engine = Engine::new(root);

        let mut speed = Parameter::new("Speed", ParamValue::Float(0.5), ParameterChangeCheck::ValueChange);
        speed.constraints.range = RangeConstraint::uniform(Some(0.0), Some(1.0));
        engine.add_node(speed.into(), None);
        engine.add_node(DashboardNode::new("Dashboard").into(), None);
        engine.apply_edits().expect("initial tree should apply");

        let target_param = direct_child_by_type(&engine, engine.root, "float").expect("target parameter should exist");
        let dashboard = direct_child_by_type(&engine, engine.root, DASHBOARD_NODE_TYPE).expect("dashboard should exist");
        engine.queue_catalog_create(dashboard, DASHBOARD_PAGE_NODE_TYPE, Some("Main".to_string()), None).expect("page creation should queue");
        engine.apply_edits().expect("page creation should apply");

        let page = first_child(&engine, dashboard);
        let target_uuid = engine.nodes.get(target_param).expect("target parameter should exist").node_data().meta.uuid;
        engine.queue_catalog_create(page, DASHBOARD_NODE_WIDGET_NODE_TYPE, Some("Speed Widget".to_string()), None).expect("node widget creation should queue");
        engine.apply_edits().expect("node widget creation should apply");

        let widget = direct_child_by_type(&engine, page, DASHBOARD_NODE_WIDGET_NODE_TYPE).expect("page should contain the created node widget");
        let target_node = find_descendant_by_decl(&engine, widget, "target_node").expect("target node parameter should exist");
        engine.edits.push(Edit::SetParam {
            node: target_node,
            value: ParamValue::Reference(NodeReference::new(target_uuid)),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        engine.apply_edits().expect("target binding should apply");
        engine.resolve().expect("resolve should succeed");
        engine.run_tick(Duration::from_millis(1)).expect("runtime tick should stabilize");

        let widget_type = find_descendant_by_decl(&engine, widget, "widget_type").expect("widget type parameter should exist");
        let widget_type_snapshot = param_snapshot(&engine, widget_type);
        let widget_types = widget_type_snapshot.constraints.enum_options.iter().map(|option| option.variant_id.as_str()).collect::<Vec<_>>();
        assert_eq!(
            widget_types,
            vec!["default", "inspector", "slider", "rotary"],
            "bound numeric widgets should converge to their widget types even when the target reference cached id is initially empty"
        );
        assert_eq!(widget_type_snapshot.value, ParamValue::Enum("default".to_string()), "bound numeric widgets should resolve to the default widget type");
    }
}
