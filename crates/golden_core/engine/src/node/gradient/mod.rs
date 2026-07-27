mod helpers;
mod model;
mod node;
mod snapshot;
mod stop;

mod prelude {
    pub(super) use super::model::{Gradient, GradientInterpolation, GradientStop};
    pub(super) use crate::color::Color;
    pub(super) use crate::edit::Edit;
    pub(super) use crate::events::{Event, EventKind};
    pub(super) use crate::node::NodeId;
    pub(super) use crate::parameter::{
        ParamValue, Parameter, ParameterChangeCheck, ParameterEnumOption, RangeConstraint,
    };
    pub(super) use crate::process_ctx::{ProcessCtx, ProcessTreeSnapshot};

    pub(super) use super::super::{
        DeclId, EventPropagation, GRADIENT_DECL_ID, GRADIENT_ITEM_KIND, GRADIENT_NODE_TYPE,
        GRADIENT_STOP_COLOR_DECL_ID, GRADIENT_STOP_INTERPOLATION_DECL_ID, GRADIENT_STOP_ITEM_KIND,
        GRADIENT_STOP_NODE_TYPE, GRADIENT_STOP_POSITION_DECL_ID, Node, NodeData, UserContainerRules, UserCreatableItem,
        parameter_child_exists,
    };
}

pub use model::{Gradient, GradientInterpolation, GradientStop};
pub use node::GradientNode;
pub use snapshot::gradient_from_snapshot;
pub use stop::GradientStopNode;

#[cfg(test)]
mod tests;
