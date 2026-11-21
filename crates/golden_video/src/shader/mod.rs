//! Bounded fragment program preparation for Golden's render graph.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Maximum size of one authored fragment program, before translation.
pub const MAX_SOURCE_BYTES: usize = 128 * 1024;

/// Maximum number of sampled 2D input textures in a fragment pass.
pub const TEXTURE_INPUTS: usize = 4;

/// Maximum number of named effect buffers visible to one fragment pass.
pub const TEMPORARY_BUFFERS: usize = 4;

/// Texture registers shared by Media inputs and effect-owned buffers.
pub const TEXTURE_BINDINGS: usize = TEXTURE_INPUTS + TEMPORARY_BUFFERS;

/// Source dialect for a GPU fragment program.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Deserialize, Serialize, TS)]
pub enum FragmentFormat {
    /// GLSL with a fragment `main` function.
    #[default]
    #[serde(rename = "GLSL")]
    Glsl,
    /// ShaderToy with a `mainImage` function.
    ShaderToy,
    /// Interactive Shader Format with JSON metadata and a fragment `main`.
    #[serde(rename = "ISF")]
    Isf,
    /// FragCoord GLSL with its clock and lower-left coordinate system.
    FragCoord,
}

impl FragmentFormat {
    /// All supported source dialects in presentation order.
    pub const ALL: [Self; 4] = [Self::Glsl, Self::ShaderToy, Self::Isf, Self::FragCoord];

    /// Exact name used by authored format selectors.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Glsl => "GLSL",
            Self::ShaderToy => "ShaderToy",
            Self::Isf => "ISF",
            Self::FragCoord => "FragCoord",
        }
    }

    /// Parse an authored format selector.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|format| format.name() == name)
    }
}

mod adapter;
mod stages;
mod uniforms;
pub use uniforms::{FragmentUniform, UniformValue};

/// Shader source diagnostics suitable for presentation beside the source.
#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    /// An empty program or a program exceeding the source or binary limit.
    #[error("Shader source is empty or exceeds the shader size limit")]
    Size,
    /// The source failed parsing, validation or native shader compilation.
    #[error("{0}")]
    Invalid(String),
}

/// Validate source input declarations without a render device or native compiler.
pub fn inspect_uniforms(format: FragmentFormat, source: &str) -> Result<Vec<FragmentUniform>, CompileError> {
    if source.trim().is_empty() || source.len() > MAX_SOURCE_BYTES {
        return Err(CompileError::Size);
    }
    adapter::inspect_uniforms(format, source)
}

#[cfg(all(windows, feature = "d3d11"))]
mod compiler;
#[cfg(all(windows, feature = "d3d11"))]
mod effect;
#[cfg(all(windows, feature = "d3d11"))]
pub use compiler::FragmentProgram;
#[cfg(all(windows, feature = "d3d11"))]
pub use effect::FragmentEffect;
#[cfg(all(windows, feature = "d3d11"))]
pub use stages::{FragmentInput, FragmentStage};

#[cfg(all(test, windows, feature = "d3d11"))]
mod tests;
