//! Reusable script authoring and QuickJS runtime APIs for Golden.

#![warn(missing_docs)]

mod quickjs;
mod types;

pub use quickjs::{QuickJsRuntime, ScriptCancellationHandle};
pub use types::*;
