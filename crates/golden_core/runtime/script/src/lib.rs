//! Reusable script authoring and QuickJS runtime APIs for Golden.

#![warn(missing_docs)]

mod method_call;
mod quickjs;
mod types;

pub use method_call::{ScriptArguments, ScriptCallPreparationError, ScriptCallReservation, ScriptMethodCall};
pub use quickjs::{QuickJsRuntime, ScriptCancellationHandle};
pub use types::*;
