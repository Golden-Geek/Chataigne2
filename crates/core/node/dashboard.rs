#![allow(missing_docs)]

use crate::{item, node};
use crate::events::Event;
use crate::node::{EventPropagation, Node, NodeData, NodeId, NodeReference, NodeUserPermissions};
use crate::parameter::{CssUnit, CssValue, Enum, Vec2};
use crate::process_ctx::ProcessCtx;

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

fn enable_dashboard_authoring(node_data: &mut NodeData) {
    node_data.meta.user_permissions = NodeUserPermissions::all();
}

fn dashboard_parent_layout_kind(node_id: NodeId, ctx: &ProcessCtx) -> Option<String> {
    let snapshot = ctx.tree_snapshot()?;
    let parent_id = snapshot.node(node_id)?.parent?;
    let parent_type = snapshot.node(parent_id)?.node_type.as_str();
    match parent_type {
        DASHBOARD_PAGE_NODE_TYPE | DASHBOARD_WIDGET_CONTAINER_NODE_TYPE => snapshot
            .find_child(parent_id, "layout_kind")
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
    title_visible: bool = true (label = "Title Visible", description = "Whether the container title bar is visible.");
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
)]
pub struct DashboardWidgetContainerNode {}

impl DashboardWidgetContainerNode {
    fn sync_parent_layout_dependency_handles(&mut self, ctx: &ProcessCtx) {
        let Some(snapshot) = ctx.tree_snapshot() else {
            return;
        };

        if let Some(position) = snapshot.find_child(self.id(), "position") {
            self.position.set_node_id(position);
        } else {
            self.position.clear_node_id();
        }

        if let Some(column_span) = snapshot.find_child(self.id(), "column_span") {
            self.column_span.set_node_id(column_span);
        } else {
            self.column_span.clear_node_id();
        }

        if let Some(row_span) = snapshot.find_child(self.id(), "row_span") {
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

/// Widget that binds directly to one engine node and renders its inspector-style UI.
#[allow(missing_docs)]
#[node("dashboard_node_widget")]
#[children(
    title_visible: bool = true (label = "Title Visible", description = "Whether the widget title bar is visible.");
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
    target_node: NodeReference (
        label = "Target Node",
        description = "Node rendered by this widget using the inspector-style UI.",
        reference_target_kind = crate::parameter::ReferenceTargetKind::AnyNode,
    );
    display_mode: Enum = "auto" (
        label = "Display Mode",
        description = "Rendering strategy used for the generated node UI. Auto chooses the natural default for the target node.",
        enum_options = ["auto", "inspector", "editor"],
    );
    include_children: bool = true (label = "Include Children", description = "Whether the generated UI should render child parameters and subnodes.");
    locked: bool = false (label = "Locked", description = "Whether drag-and-drop rebinding is disabled for this widget.");
)]
pub struct DashboardNodeWidgetNode {}

impl DashboardNodeWidgetNode {
    fn sync_parent_layout_dependency_handles(&mut self, ctx: &ProcessCtx) {
        let Some(snapshot) = ctx.tree_snapshot() else {
            return;
        };

        if let Some(position) = snapshot.find_child(self.id(), "position") {
            self.position.set_node_id(position);
        } else {
            self.position.clear_node_id();
        }

        if let Some(column_span) = snapshot.find_child(self.id(), "column_span") {
            self.column_span.set_node_id(column_span);
        } else {
            self.column_span.clear_node_id();
        }

        if let Some(row_span) = snapshot.find_child(self.id(), "row_span") {
            self.row_span.set_node_id(row_span);
        } else {
            self.row_span.clear_node_id();
        }
    }
}

#[item("dashboard_widget", from_struct)]
impl Node for DashboardNodeWidgetNode {
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

/// Configurable widget for canonical controls such as buttons, sliders, text, and checkboxes.
#[allow(missing_docs)]
#[node("dashboard_generic_widget")]
#[children(
    title_visible: bool = true (label = "Title Visible", description = "Whether the widget title bar is visible.");
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
    target_param: NodeReference (
        label = "Target Parameter",
        description = "Parameter targeted by this widget when it acts as a control or display.",
        reference_target_kind = crate::parameter::ReferenceTargetKind::ParameterOnly,
    );
)]
pub struct DashboardGenericWidgetNode {}

impl DashboardGenericWidgetNode {
    fn sync_parent_layout_dependency_handles(&mut self, ctx: &ProcessCtx) {
        let Some(snapshot) = ctx.tree_snapshot() else {
            return;
        };

        if let Some(position) = snapshot.find_child(self.id(), "position") {
            self.position.set_node_id(position);
        } else {
            self.position.clear_node_id();
        }

        if let Some(column_span) = snapshot.find_child(self.id(), "column_span") {
            self.column_span.set_node_id(column_span);
        } else {
            self.column_span.clear_node_id();
        }

        if let Some(row_span) = snapshot.find_child(self.id(), "row_span") {
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
    use crate::edit::Edit;
    use crate::define_node_enum;
    use crate::engine::Engine;
    use crate::node::{Folder, Node, NodeId};
    use crate::parameter::{ParamValue, ParameterEventBehaviour, ParameterSnapshot, ReferenceTargetKind};

    use super::{
        DASHBOARD_GENERIC_WIDGET_NODE_TYPE, DASHBOARD_NODE_TYPE, DASHBOARD_NODE_WIDGET_NODE_TYPE, DASHBOARD_PAGE_NODE_TYPE, DASHBOARD_WIDGET_CONTAINER_NODE_TYPE, DashboardNode,
    };

    define_node_enum!(enum DashboardTestNode {});

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

        assert_eq!(
            direct_child_decl_ids(&engine, widget),
            vec!["title_visible", "position", "size", "widget_kind", "text", "target_param"],
            "reintroduced free-layout parameters should return to declared order",
        );
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
}