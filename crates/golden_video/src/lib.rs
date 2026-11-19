//! App-neutral video execution contracts.
//!
//! Plans describe logical textures and generic operations, without native interfaces, engine
//! nodes or product policy. Preparation validates and compiles them before a renderer admits
//! actual resources. Project/channel authoring and endpoint selection remain in the application.

#![warn(missing_docs)]

mod color;
#[cfg(feature = "ffmpeg")]
pub mod decode;
mod identity;
pub mod image;
#[cfg(all(windows, target_arch = "x86_64", feature = "ndi"))]
pub mod ndi;
mod ndi_name;
pub mod plan;
pub mod preview;
pub mod qualification;
mod resource;
pub mod runtime;
pub mod scheduling;

#[cfg(all(windows, feature = "d3d11"))]
pub mod native;
pub mod shader;

pub use color::{ColorError, LinearColor, Opacity};
pub use identity::{OutputId, PassId, TextureId};
pub use ndi_name::{normalize_ndi_sender_name, valid_ndi_sender_name};
pub use resource::{AlphaMode, ColorEncoding, Extent2D, PixelFormat, TextureDescriptor, TextureError};
