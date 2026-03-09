use crate::node::{DashboardWidgetOptionsNodeKind, Node, NodeData, NodeId, NodeUserPermissions};
use crate::process_ctx::ProcessCtx;

mod color_editor;
mod inspector;
mod number_rotary;
mod number_slider;
mod vec2_pad;
mod vector_editor;

pub use color_editor::{DASHBOARD_NODE_WIDGET_COLOR_EDITOR_OPTIONS_NODE_TYPE, DashboardNodeWidgetColorEditorOptionsNode};
pub use inspector::{DASHBOARD_NODE_WIDGET_INSPECTOR_OPTIONS_NODE_TYPE, DashboardNodeWidgetInspectorOptionsNode};
pub use number_rotary::{DASHBOARD_NODE_WIDGET_NUMBER_ROTARY_OPTIONS_NODE_TYPE, DashboardNodeWidgetNumberRotaryOptionsNode};
pub use number_slider::{DASHBOARD_NODE_WIDGET_NUMBER_SLIDER_OPTIONS_NODE_TYPE, DashboardNodeWidgetNumberSliderOptionsNode};
pub use vec2_pad::{DASHBOARD_NODE_WIDGET_VEC2_PAD_OPTIONS_NODE_TYPE, DashboardNodeWidgetVec2PadOptionsNode};
pub use vector_editor::{DASHBOARD_NODE_WIDGET_VEC2_EDITOR_OPTIONS_NODE_TYPE, DASHBOARD_NODE_WIDGET_VEC3_EDITOR_OPTIONS_NODE_TYPE, DashboardNodeWidgetVec2EditorOptionsNode, DashboardNodeWidgetVec3EditorOptionsNode};

pub(crate) fn dashboard_widget_options_node_type(kind: &DashboardWidgetOptionsNodeKind) -> &'static str {
    match kind {
        DashboardWidgetOptionsNodeKind::Inspector => DASHBOARD_NODE_WIDGET_INSPECTOR_OPTIONS_NODE_TYPE,
        DashboardWidgetOptionsNodeKind::NumberSlider => DASHBOARD_NODE_WIDGET_NUMBER_SLIDER_OPTIONS_NODE_TYPE,
        DashboardWidgetOptionsNodeKind::NumberRotary => DASHBOARD_NODE_WIDGET_NUMBER_ROTARY_OPTIONS_NODE_TYPE,
        DashboardWidgetOptionsNodeKind::Vec2Pad => DASHBOARD_NODE_WIDGET_VEC2_PAD_OPTIONS_NODE_TYPE,
        DashboardWidgetOptionsNodeKind::Vec2Editor => DASHBOARD_NODE_WIDGET_VEC2_EDITOR_OPTIONS_NODE_TYPE,
        DashboardWidgetOptionsNodeKind::Vec3Editor => DASHBOARD_NODE_WIDGET_VEC3_EDITOR_OPTIONS_NODE_TYPE,
        DashboardWidgetOptionsNodeKind::ColorEditor => DASHBOARD_NODE_WIDGET_COLOR_EDITOR_OPTIONS_NODE_TYPE,
    }
}

pub(crate) fn make_dashboard_widget_options_node(kind: &DashboardWidgetOptionsNodeKind) -> Box<dyn Node> {
    match kind {
        DashboardWidgetOptionsNodeKind::Inspector => Box::new(DashboardNodeWidgetInspectorOptionsNode::new("Widget Options")),
        DashboardWidgetOptionsNodeKind::NumberSlider => Box::new(DashboardNodeWidgetNumberSliderOptionsNode::new("Widget Options")),
        DashboardWidgetOptionsNodeKind::NumberRotary => Box::new(DashboardNodeWidgetNumberRotaryOptionsNode::new("Widget Options")),
        DashboardWidgetOptionsNodeKind::Vec2Pad => Box::new(DashboardNodeWidgetVec2PadOptionsNode::new("Widget Options")),
        DashboardWidgetOptionsNodeKind::Vec2Editor => Box::new(DashboardNodeWidgetVec2EditorOptionsNode::new("Widget Options")),
        DashboardWidgetOptionsNodeKind::Vec3Editor => Box::new(DashboardNodeWidgetVec3EditorOptionsNode::new("Widget Options")),
        DashboardWidgetOptionsNodeKind::ColorEditor => Box::new(DashboardNodeWidgetColorEditorOptionsNode::new("Widget Options")),
    }
}

pub(crate) fn initialize_replaced_dashboard_widget_options_node(ctx: &mut ProcessCtx, node_id: NodeId) {
    ctx.call_node_mutation(node_id, |node, child_ctx| {
        node.engine_on_attached(child_ctx);
        node.init(child_ctx);
        Ok(())
    });
}

pub(crate) fn configure_dashboard_widget_options_node(node_data: &mut NodeData) {
    node_data.meta.user_permissions = NodeUserPermissions::none();
}
