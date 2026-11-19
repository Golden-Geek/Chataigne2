//! Bounded fragment program preparation for Golden's render graph.

/// Maximum size of one authored fragment program, before translation.
pub const MAX_SOURCE_BYTES: usize = 128 * 1024;

#[cfg(all(windows, feature = "d3d11"))]
mod compiler;
#[cfg(all(windows, feature = "d3d11"))]
pub use compiler::{CompileError, FragmentProgram};

#[cfg(all(test, windows, feature = "d3d11"))]
mod tests;
