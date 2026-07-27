mod easing;
mod helpers;
mod key;
mod model;
mod node;
mod range;
mod snapshot;

mod prelude {
    pub(super) use super::model::{
        Curve, CurveBezierFitOptions, CurveEasing, CurveFitPoint, CurveHandle, CurveKey, CurvePhaseMode, CurveShape,
        CurveStepMode, bezier_easing_from_endpoint_slopes, fit_points_to_bezier_keys,
    };
    pub(super) use crate::edit::Edit;
    pub(super) use crate::events::{Event, EventKind};
    pub(super) use crate::node::NodeId;
    pub(super) use crate::parameter::{
        ParamValue, Parameter, ParameterChangeCheck, ParameterEventBehaviour, RangeConstraint,
    };
    pub(super) use crate::process_ctx::{ProcessCtx, ProcessTreeSnapshot};

    pub(super) use super::super::{
        DeclId, EventPropagation, Node, NodeData, PARAMETER_ANIMATION_CURVE_DECL_ID,
        PARAMETER_ANIMATION_CURVE_ITEM_KIND, PARAMETER_ANIMATION_CURVE_NODE_TYPE,
        PARAMETER_ANIMATION_EASING_AMPLITUDE_DECL_ID, PARAMETER_ANIMATION_EASING_DECL_ID,
        PARAMETER_ANIMATION_EASING_FADE_IN_DECL_ID, PARAMETER_ANIMATION_EASING_FADE_OUT_DECL_ID,
        PARAMETER_ANIMATION_EASING_FREQUENCY_DECL_ID, PARAMETER_ANIMATION_EASING_IN_POSITION_DECL_ID,
        PARAMETER_ANIMATION_EASING_IN_VALUE_DECL_ID, PARAMETER_ANIMATION_EASING_KIND_DECL_ID,
        PARAMETER_ANIMATION_EASING_NUM_PHASES_DECL_ID, PARAMETER_ANIMATION_EASING_NUM_STEPS_DECL_ID,
        PARAMETER_ANIMATION_EASING_OCTAVES_DECL_ID, PARAMETER_ANIMATION_EASING_OUT_POSITION_DECL_ID,
        PARAMETER_ANIMATION_EASING_OUT_VALUE_DECL_ID, PARAMETER_ANIMATION_EASING_PHASE_DECL_ID,
        PARAMETER_ANIMATION_EASING_PHASE_MODE_DECL_ID, PARAMETER_ANIMATION_EASING_SCRIPT_SOURCE_DECL_ID,
        PARAMETER_ANIMATION_EASING_SEED_DECL_ID, PARAMETER_ANIMATION_EASING_SHAPE_DECL_ID,
        PARAMETER_ANIMATION_EASING_STEP_MODE_DECL_ID, PARAMETER_ANIMATION_EASING_STEP_SIZE_DECL_ID,
        PARAMETER_ANIMATION_KEY_ITEM_KIND, PARAMETER_ANIMATION_KEY_NODE_TYPE, PARAMETER_ANIMATION_KEY_POSITION_DECL_ID,
        PARAMETER_ANIMATION_KEY_VALUE_DECL_ID, PARAMETER_ANIMATION_RANGE_DECL_ID, PARAMETER_ANIMATION_RANGE_NODE_TYPE,
        PARAMETER_ANIMATION_RANGE_X_DECL_ID, PARAMETER_ANIMATION_RANGE_Y_DECL_ID, UserContainerRules,
        UserCreatableItem, parameter_child_exists,
    };
}

pub use easing::CurveEasingNode;
pub use key::CurveKeyNode;
pub use model::{
    Curve, CurveBezierFitOptions, CurveCursor, CurveEasing, CurveFitPoint, CurveHandle, CurveKey, CurvePhaseMode,
    CurveShape, CurveStepMode,
};
pub use node::CurveNode;
pub use range::{CurveRangeConstraint, CurveRangeNode};
pub use snapshot::curve_from_snapshot;

#[cfg(test)]
mod tests;
