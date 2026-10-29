//! App-neutral video execution contracts.
//!
//! Plans describe logical textures and generic operations, without native interfaces, engine
//! nodes or product policy. Preparation validates and compiles them before a renderer admits
//! actual resources. Project/channel authoring and endpoint selection remain in the application.

#![warn(missing_docs)]

mod color;
mod identity;
pub mod image;
pub mod plan;
pub mod preview;
pub mod qualification;
mod resource;
pub mod runtime;

#[cfg(all(windows, feature = "d3d11"))]
pub mod native;

pub use color::{ColorError, LinearColor, Opacity};
pub use identity::{OutputId, PassId, TextureId};
pub use resource::{AlphaMode, ColorEncoding, Extent2D, PixelFormat, TextureDescriptor, TextureError};
