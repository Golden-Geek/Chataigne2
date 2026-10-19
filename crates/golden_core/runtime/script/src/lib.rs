//! Reusable script authoring and QuickJS runtime APIs for Golden.

#![warn(missing_docs)]

mod custom_event;
mod input;
mod method_call;
mod property_write;
mod quickjs;
mod types;

pub use custom_event::{ScriptCustomEvent, ScriptEventReservation};
pub use input::ScriptInputError;
pub use method_call::{ScriptArguments, ScriptCallPreparationError, ScriptCallReservation, ScriptMethodCall};
pub use property_write::{ScriptPropertyReservation, ScriptPropertyWrite};
pub use quickjs::{QuickJsRuntime, ScriptCancellationHandle};
pub use types::*;
