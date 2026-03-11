mod core;
/// Built-in node families, node traits, and node-level constants.
pub mod curve;
mod dashboard;
mod handles;

pub use crate::parameter::ParameterValueType;
pub use core::*;
pub use curve::{
    Curve, CurveBezierFitOptions, CurveCursor, CurveEasing, CurveEasingNode, CurveFitPoint, CurveHandle, CurveKey,
    CurveKeyNode, CurveNode, CurvePhaseMode, CurveRangeConstraint, CurveRangeNode, CurveShape, CurveStepMode,
    curve_from_snapshot,
};
pub use dashboard::{
    DASHBOARD_GENERIC_WIDGET_NODE_TYPE, DASHBOARD_ITEM_KIND, DASHBOARD_NODE_TYPE,
    DASHBOARD_NODE_WIDGET_COLOR_EDITOR_OPTIONS_NODE_TYPE, DASHBOARD_NODE_WIDGET_INSPECTOR_OPTIONS_NODE_TYPE,
    DASHBOARD_NODE_WIDGET_NODE_TYPE, DASHBOARD_NODE_WIDGET_NUMBER_ROTARY_OPTIONS_NODE_TYPE,
    DASHBOARD_NODE_WIDGET_NUMBER_SLIDER_OPTIONS_NODE_TYPE, DASHBOARD_NODE_WIDGET_PARAMETER_EDITOR_OPTIONS_NODE_TYPE,
    DASHBOARD_NODE_WIDGET_VEC2_EDITOR_OPTIONS_NODE_TYPE, DASHBOARD_NODE_WIDGET_VEC2_PAD_OPTIONS_NODE_TYPE,
    DASHBOARD_NODE_WIDGET_VEC3_EDITOR_OPTIONS_NODE_TYPE, DASHBOARD_PAGE_ITEM_KIND, DASHBOARD_PAGE_NODE_TYPE,
    DASHBOARD_WIDGET_CONTAINER_NODE_TYPE, DASHBOARD_WIDGET_ITEM_KIND, DashboardGenericWidgetNode, DashboardNode,
    DashboardNodeWidgetColorEditorOptionsNode, DashboardNodeWidgetInspectorOptionsNode, DashboardNodeWidgetNode,
    DashboardNodeWidgetNumberRotaryOptionsNode, DashboardNodeWidgetNumberSliderOptionsNode,
    DashboardNodeWidgetParameterEditorOptionsNode, DashboardNodeWidgetVec2EditorOptionsNode,
    DashboardNodeWidgetVec2PadOptionsNode, DashboardNodeWidgetVec3EditorOptionsNode, DashboardPageNode,
    DashboardWidgetContainerNode, DashboardWidgetOptionsNodeKind, DashboardWidgetTargetDescriptor,
    DashboardWidgetTypeSpec,
};
pub use handles::{DeclaredNodeHandle, NodeHandle, ParameterHandle, PotentialNodeHandle};

pub(crate) use core::parameter_child_exists;
