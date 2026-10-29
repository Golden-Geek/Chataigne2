//! Worker-local D3D11 execution with explicit adapter and immutable native output bindings.
//!
//! Only configuration and bounded observations leave this module's worker. The host owns HWND
//! lifetime; neither COM objects nor textures enter engine/UI mailboxes. Dynamic preview geometry
//! remains separate integration work. Bounded source workers own their SDK and device work;
//! renderer imports retain allocation leases through their own matching completion.

mod backend;
mod completion;
mod config;
mod device;
mod inputs;
mod observation;
mod outputs;
mod pipeline;
mod texture;

pub use config::{AdapterId, D3d11Config, InputLimits, NativeConfigError, OutputBinding, WindowTarget};
pub use device::{AdapterInfo, adapters};
pub use inputs::{SourceSnapshot, SourceState};
pub use observation::{D3d11Control, D3d11Snapshot, NativeFailure};

pub(crate) use backend::D3d11Backend;

#[cfg(test)]
pub(crate) mod tests;
