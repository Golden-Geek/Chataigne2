//! Reusable script authoring and QuickJS runtime APIs for Golden.

#![warn(missing_docs)]

mod app_operation;
mod custom_event;
mod input;
mod method_call;
mod property_write;
mod quickjs;
mod types;

pub use app_operation::{
    SCRIPT_APP_OPERATION_BYTES, SCRIPT_APP_OPERATION_DEPTH, SCRIPT_APP_OPERATION_VALUES, ScriptAppOperationRequest,
    ScriptAppOperationReservation, ScriptAppOperationResult,
};
pub use custom_event::{ScriptCustomEvent, ScriptEventReservation};
pub use input::ScriptInputError;
pub use method_call::{ScriptArguments, ScriptCallPreparationError, ScriptCallReservation, ScriptMethodCall};
pub use property_write::{ScriptPropertyReservation, ScriptPropertyWrite};
pub use quickjs::{QuickJsRuntime, ScriptCancellationHandle};
pub use types::*;
