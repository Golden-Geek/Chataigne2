use crate::node::{DashboardWidgetOptionsNodeKind, Node, NodeData, NodeId, NodeUserPermissions};
use crate::process_ctx::ProcessCtx;

mod color_editor;
mod inspector;
mod number_rotary;
mod number_slider;
mod parameter_editor;
mod vec2_pad;
mod vector_editor;

pub use color_editor::{
    DASHBOARD_NODE_WIDGET_COLOR_EDITOR_OPTIONS_NODE_TYPE, DashboardNodeWidgetColorEditorOptionsNode,
};
pub use inspector::{DASHBOARD_NODE_WIDGET_INSPECTOR_OPTIONS_NODE_TYPE, DashboardNodeWidgetInspectorOptionsNode};
pub use number_rotary::{
    DASHBOARD_NODE_WIDGET_NUMBER_ROTARY_OPTIONS_NODE_TYPE, DashboardNodeWidgetNumberRotaryOptionsNode,
};
pub use number_slider::{
    DASHBOARD_NODE_WIDGET_NUMBER_SLIDER_OPTIONS_NODE_TYPE, DashboardNodeWidgetNumberSliderOptionsNode,
};
pub use parameter_editor::{
    DASHBOARD_NODE_WIDGET_PARAMETER_EDITOR_OPTIONS_NODE_TYPE, DashboardNodeWidgetParameterEditorOptionsNode,
};
pub use vec2_pad::{DASHBOARD_NODE_WIDGET_VEC2_PAD_OPTIONS_NODE_TYPE, DashboardNodeWidgetVec2PadOptionsNode};
pub use vector_editor::{
    DASHBOARD_NODE_WIDGET_VEC2_EDITOR_OPTIONS_NODE_TYPE, DASHBOARD_NODE_WIDGET_VEC3_EDITOR_OPTIONS_NODE_TYPE,
    DashboardNodeWidgetVec2EditorOptionsNode, DashboardNodeWidgetVec3EditorOptionsNode,
};

pub(crate) fn dashboard_widget_options_node_type(kind: &DashboardWidgetOptionsNodeKind) -> &'static str {
    match kind {
        DashboardWidgetOptionsNodeKind::Inspector => DASHBOARD_NODE_WIDGET_INSPECTOR_OPTIONS_NODE_TYPE,
        DashboardWidgetOptionsNodeKind::ParameterEditor => DASHBOARD_NODE_WIDGET_PARAMETER_EDITOR_OPTIONS_NODE_TYPE,
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
        DashboardWidgetOptionsNodeKind::Inspector => Box::new(DashboardNodeWidgetInspectorOptionsNode::new()),
        DashboardWidgetOptionsNodeKind::ParameterEditor => {
            Box::new(DashboardNodeWidgetParameterEditorOptionsNode::new())
        }
        DashboardWidgetOptionsNodeKind::NumberSlider => Box::new(DashboardNodeWidgetNumberSliderOptionsNode::new()),
        DashboardWidgetOptionsNodeKind::NumberRotary => Box::new(DashboardNodeWidgetNumberRotaryOptionsNode::new()),
        DashboardWidgetOptionsNodeKind::Vec2Pad => Box::new(DashboardNodeWidgetVec2PadOptionsNode::new()),
        DashboardWidgetOptionsNodeKind::Vec2Editor => Box::new(DashboardNodeWidgetVec2EditorOptionsNode::new()),
        DashboardWidgetOptionsNodeKind::Vec3Editor => Box::new(DashboardNodeWidgetVec3EditorOptionsNode::new()),
        DashboardWidgetOptionsNodeKind::ColorEditor => Box::new(DashboardNodeWidgetColorEditorOptionsNode::new()),
    }
}

pub(crate) fn refresh_dashboard_widget_options_node(ctx: &mut ProcessCtx, node_id: NodeId) {
    let Some(node_type) = ctx
        .tree_snapshot()
        .and_then(|snapshot| snapshot.node(node_id))
        .map(|snapshot| snapshot.node_type.clone())
    else {
        return;
    };

    match node_type.as_str() {
        DASHBOARD_NODE_WIDGET_INSPECTOR_OPTIONS_NODE_TYPE => crate::node::NodeHandle::new(node_id)
            .with_mut::<DashboardNodeWidgetInspectorOptionsNode, _>(
            ctx,
            |node, child_ctx| node.__golden_node_engine_preprocess_inbox(child_ctx, node.id()),
        ),
        DASHBOARD_NODE_WIDGET_PARAMETER_EDITOR_OPTIONS_NODE_TYPE => crate::node::NodeHandle::new(node_id)
            .with_mut::<DashboardNodeWidgetParameterEditorOptionsNode, _>(
            ctx,
            |node, child_ctx| node.__golden_node_engine_preprocess_inbox(child_ctx, node.id()),
        ),
        DASHBOARD_NODE_WIDGET_NUMBER_SLIDER_OPTIONS_NODE_TYPE => crate::node::NodeHandle::new(node_id)
            .with_mut::<DashboardNodeWidgetNumberSliderOptionsNode, _>(
            ctx,
            |node, child_ctx| node.__golden_node_engine_preprocess_inbox(child_ctx, node.id()),
        ),
        DASHBOARD_NODE_WIDGET_NUMBER_ROTARY_OPTIONS_NODE_TYPE => crate::node::NodeHandle::new(node_id)
            .with_mut::<DashboardNodeWidgetNumberRotaryOptionsNode, _>(
            ctx,
            |node, child_ctx| node.__golden_node_engine_preprocess_inbox(child_ctx, node.id()),
        ),
        DASHBOARD_NODE_WIDGET_VEC2_PAD_OPTIONS_NODE_TYPE => crate::node::NodeHandle::new(node_id)
            .with_mut::<DashboardNodeWidgetVec2PadOptionsNode, _>(
            ctx,
            |node, child_ctx| node.__golden_node_engine_preprocess_inbox(child_ctx, node.id()),
        ),
        DASHBOARD_NODE_WIDGET_VEC2_EDITOR_OPTIONS_NODE_TYPE => crate::node::NodeHandle::new(node_id)
            .with_mut::<DashboardNodeWidgetVec2EditorOptionsNode, _>(
            ctx,
            |node, child_ctx| node.__golden_node_engine_preprocess_inbox(child_ctx, node.id()),
        ),
        DASHBOARD_NODE_WIDGET_VEC3_EDITOR_OPTIONS_NODE_TYPE => crate::node::NodeHandle::new(node_id)
            .with_mut::<DashboardNodeWidgetVec3EditorOptionsNode, _>(
            ctx,
            |node, child_ctx| node.__golden_node_engine_preprocess_inbox(child_ctx, node.id()),
        ),
        DASHBOARD_NODE_WIDGET_COLOR_EDITOR_OPTIONS_NODE_TYPE => crate::node::NodeHandle::new(node_id)
            .with_mut::<DashboardNodeWidgetColorEditorOptionsNode, _>(
            ctx,
            |node, child_ctx| node.__golden_node_engine_preprocess_inbox(child_ctx, node.id()),
        ),
        _ => {}
    }
}

pub(crate) fn configure_dashboard_widget_options_node(node_data: &mut NodeData) {
    node_data.meta.user_permissions = NodeUserPermissions::none();
}
