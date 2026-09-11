use golden_context::{UiUserContextsDto, UserContextCandidate};
use golden_model::{
    CurveBezierFitOptions, CurveFitPoint, CustomEventRetention, DeclId, EngineTime, LogRecord, NodeId,
    NodeUserPermissions, NodeUuid, PresentationHint, ProjectFileSpec, UserNodeRole,
};
use golden_parameters::{
    CssValue, ParamValue, ParamValueProjection, ParameterConstraints, ParameterControlMode, ParameterControlSpec,
    ParameterControlState, ParameterEventBehaviour, ParameterSnapshot, ParameterUiHints,
};
use golden_script_contract::ScriptUiConfig;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

mod edit;
mod event;
mod intent;
mod message;
mod snapshot;

pub use edit::*;
pub use event::*;
pub use intent::*;
pub use message::*;
pub use snapshot::*;

fn is_default_presentation_hint(value: &PresentationHint) -> bool {
    *value == PresentationHint::default()
}

fn is_default_user_permissions(value: &NodeUserPermissions) -> bool {
    *value == NodeUserPermissions::default()
}

fn is_default_event_behaviour(value: &ParameterEventBehaviour) -> bool {
    *value == ParameterEventBehaviour::default()
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn is_empty_create_user_item_initial_params(value: &[UiCreateUserItemInitialParam]) -> bool {
    value.is_empty()
}

fn is_empty_duplicate_node_specs(value: &[UiDuplicateNodeSpec]) -> bool {
    value.is_empty()
}

fn is_empty_duplicate_create_user_item_specs(value: &[UiDuplicateCreateUserItemSpec]) -> bool {
    value.is_empty()
}

fn is_empty_duplicate_dependent_user_items(value: &[UiDuplicateDependentUserItem]) -> bool {
    value.is_empty()
}

fn is_empty_duplicate_dependent_initial_params(value: &[UiDuplicateDependentUserItemInitialParam]) -> bool {
    value.is_empty()
}

fn is_default_parameter_constraints(value: &ParameterConstraints) -> bool {
    *value == ParameterConstraints::default()
}

fn is_default_parameter_ui_hints(value: &ParameterUiHints) -> bool {
    *value == ParameterUiHints::default()
}
