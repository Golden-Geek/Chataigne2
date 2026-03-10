mod animation_curve_nodes;
mod control_animation;
mod core;
mod dashboard;
mod dashboard_widget_options;
mod handles;

pub use animation_curve_nodes::{
    curve_from_snapshot, AnimationCurveEasingNode, AnimationCurveKeyNode, AnimationCurveNode,
    AnimationCurveRangeConstraint, AnimationCurveRangeNode,
};
pub use control_animation::ParameterAnimationControlNode;
pub use core::*;
pub use dashboard::{
    DashboardGenericWidgetNode, DashboardNode, DashboardNodeWidgetNode, DashboardPageNode,
    DashboardWidgetContainerNode, DashboardWidgetOptionsNodeKind, DashboardWidgetTargetDescriptor,
    DashboardWidgetTypeSpec, DASHBOARD_GENERIC_WIDGET_NODE_TYPE, DASHBOARD_ITEM_KIND, DASHBOARD_NODE_TYPE,
    DASHBOARD_NODE_WIDGET_NODE_TYPE, DASHBOARD_PAGE_ITEM_KIND, DASHBOARD_PAGE_NODE_TYPE,
    DASHBOARD_WIDGET_CONTAINER_NODE_TYPE, DASHBOARD_WIDGET_ITEM_KIND,
};
pub use dashboard_widget_options::{
    DashboardNodeWidgetColorEditorOptionsNode, DashboardNodeWidgetInspectorOptionsNode,
    DashboardNodeWidgetNumberRotaryOptionsNode, DashboardNodeWidgetNumberSliderOptionsNode,
    DashboardNodeWidgetParameterEditorOptionsNode, DashboardNodeWidgetVec2EditorOptionsNode,
    DashboardNodeWidgetVec2PadOptionsNode, DashboardNodeWidgetVec3EditorOptionsNode,
    DASHBOARD_NODE_WIDGET_COLOR_EDITOR_OPTIONS_NODE_TYPE, DASHBOARD_NODE_WIDGET_INSPECTOR_OPTIONS_NODE_TYPE,
    DASHBOARD_NODE_WIDGET_NUMBER_ROTARY_OPTIONS_NODE_TYPE, DASHBOARD_NODE_WIDGET_NUMBER_SLIDER_OPTIONS_NODE_TYPE,
    DASHBOARD_NODE_WIDGET_PARAMETER_EDITOR_OPTIONS_NODE_TYPE, DASHBOARD_NODE_WIDGET_VEC2_EDITOR_OPTIONS_NODE_TYPE,
    DASHBOARD_NODE_WIDGET_VEC2_PAD_OPTIONS_NODE_TYPE, DASHBOARD_NODE_WIDGET_VEC3_EDITOR_OPTIONS_NODE_TYPE,
};
pub use crate::parameter::ParameterValueType;
pub use handles::{DeclaredNodeHandle, NodeHandle, ParameterHandle, PotentialNodeHandle};

pub(crate) use core::parameter_child_exists;