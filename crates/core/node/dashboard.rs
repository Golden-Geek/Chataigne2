#![allow(missing_docs)]

use crate::{item, node};
use crate::events::Event;
use crate::node::{EventPropagation, Node, NodeData, NodeReference, NodeUserPermissions};
use crate::parameter::{Enum, Vec2};
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
        EventPropagation::PassOn
    }
}

/// Top-level dashboard page that arranges widgets for one route/view.
#[allow(missing_docs)]
#[node("dashboard_page")]
#[children(
    route: String = "".to_string() (label = "Route", description = "Stable route or slug used to address this page.");
    layout_kind: Enum = "free" (
        label = "Layout",
        description = "Primary layout strategy used for the page root.",
        enum_options = ["free", "horizontal", "vertical", "grid", "accordion", "tabs"],
    );
    gap: Vec2 = (1.0, 1.0) (label = "Gap", description = "Spacing between child widgets.");
    grid_columns: i32 = 12 [1..64] (label = "Grid Columns", description = "Column count when the page layout uses a grid.");
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

    fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
        EventPropagation::PassOn
    }
}

/// Widget container that arranges child widgets using one layout strategy.
#[allow(missing_docs)]
#[node("dashboard_widget_container")]
#[children(
    title_visible: bool = true (label = "Title Visible", description = "Whether the container title bar is visible.");
    position: Vec2 = (0.0, 0.0) (label = "Position", description = "Anchor position used by free layouts.");
    size: Vec2 = (12.0, 8.0) (label = "Size", description = "Preferred widget size in logical layout units.");
    column_span: i32 = 1 [1..64] (label = "Column Span", description = "Number of grid columns consumed by this widget.");
    row_span: i32 = 1 [1..64] (label = "Row Span", description = "Number of grid rows consumed by this widget.");
    layout_kind: Enum = "free" (
        label = "Layout",
        description = "How this container arranges its child widgets.",
        enum_options = ["free", "horizontal", "vertical", "grid", "accordion", "tabs"],
    );
    gap: Vec2 = (1.0, 1.0) (label = "Gap", description = "Spacing between child widgets.");
    grid_columns: i32 = 12 [1..64] (label = "Grid Columns", description = "Column count when the container layout uses a grid.");
    wrap: bool = true (label = "Wrap", description = "Whether children may wrap onto new rows or columns.");
)]
pub struct DashboardWidgetContainerNode {}

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

    fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
        EventPropagation::PassOn
    }
}

/// Widget that binds directly to one engine node and renders its inspector-style UI.
#[allow(missing_docs)]
#[node("dashboard_node_widget")]
#[children(
    title_visible: bool = true (label = "Title Visible", description = "Whether the widget title bar is visible.");
    position: Vec2 = (0.0, 0.0) (label = "Position", description = "Anchor position used by free layouts.");
    size: Vec2 = (12.0, 4.0) (label = "Size", description = "Preferred widget size in logical layout units.");
    column_span: i32 = 1 [1..64] (label = "Column Span", description = "Number of grid columns consumed by this widget.");
    row_span: i32 = 1 [1..64] (label = "Row Span", description = "Number of grid rows consumed by this widget.");
    target_node: NodeReference (
        label = "Target Node",
        description = "Node rendered by this widget using the inspector-style UI.",
        reference_target_kind = crate::parameter::ReferenceTargetKind::AnyNode,
    );
    display_mode: Enum = "inspector" (
        label = "Display Mode",
        description = "Rendering strategy used for the generated node UI.",
        enum_options = ["inspector", "compact", "minimal"],
    );
    include_children: bool = true (label = "Include Children", description = "Whether the generated UI should render child parameters and subnodes.");
    locked: bool = false (label = "Locked", description = "Whether drag-and-drop rebinding is disabled for this widget.");
)]
pub struct DashboardNodeWidgetNode {}

#[item("dashboard_widget", from_struct)]
impl Node for DashboardNodeWidgetNode {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        enable_dashboard_authoring(self.node_data_mut());
    }

    fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
        EventPropagation::PassOn
    }
}

/// Configurable widget for canonical controls such as buttons, sliders, text, and checkboxes.
#[allow(missing_docs)]
#[node("dashboard_generic_widget")]
#[children(
    title_visible: bool = true (label = "Title Visible", description = "Whether the widget title bar is visible.");
    position: Vec2 = (0.0, 0.0) (label = "Position", description = "Anchor position used by free layouts.");
    size: Vec2 = (10.0, 3.0) (label = "Size", description = "Preferred widget size in logical layout units.");
    column_span: i32 = 1 [1..64] (label = "Column Span", description = "Number of grid columns consumed by this widget.");
    row_span: i32 = 1 [1..64] (label = "Row Span", description = "Number of grid rows consumed by this widget.");
    widget_kind: Enum = "text" (
        label = "Widget Kind",
        description = "Canonical widget family rendered by this node.",
        enum_options = ["text", "button", "slider", "textInput", "checkbox"],
    );
    text: String = "".to_string() (label = "Text", description = "Static text content or button label when applicable.");
    placeholder: String = "".to_string() (label = "Placeholder", description = "Placeholder text used by text input widgets.");
    value_range: Vec2 = (0.0, 1.0) (label = "Value Range", description = "Minimum and maximum range for slider widgets.");
    step: f64 = 0.01 [0.0..1000.0] (label = "Step", description = "Step increment used by slider widgets.");
    multiline: bool = false (label = "Multiline", description = "Whether text inputs accept multiple lines.");
    default_checked: bool = false (label = "Default Checked", description = "Default state used by checkbox widgets.");
    target_param: NodeReference (
        label = "Target Parameter",
        description = "Parameter targeted by this widget when it acts as a control or display.",
        reference_target_kind = crate::parameter::ReferenceTargetKind::ParameterOnly,
    );
)]
pub struct DashboardGenericWidgetNode {}

#[item("dashboard_widget", from_struct, scriptable, contextualizable)]
impl Node for DashboardGenericWidgetNode {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        enable_dashboard_authoring(self.node_data_mut());
    }

    fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
        EventPropagation::PassOn
    }
}

#[cfg(test)]
mod tests {
    use crate::define_node_enum;
    use crate::engine::Engine;
    use crate::node::{Folder, Node, NodeId};
    use crate::parameter::{ParameterSnapshot, ReferenceTargetKind};

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
}