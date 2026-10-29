use std::error::Error;
use std::fmt;

use crate::{OutputId, PassId, TextureError, TextureId};

/// Submitted graph work counted before allocating compiler indexes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphLimit {
    /// All submitted render passes.
    Passes,
    /// All submitted external textures.
    ExternalTextures,
    /// All submitted output declarations.
    Outputs,
    /// All submitted input references, including repeated references.
    Dependencies,
}

impl fmt::Display for GraphLimit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Passes => "passes",
            Self::ExternalTextures => "external textures",
            Self::Outputs => "outputs",
            Self::Dependencies => "dependencies",
        })
    }
}

/// Render-plan rejection before backend allocation or live installation.
#[derive(Debug)]
pub enum PlanError {
    /// A submitted graph count exceeds its explicit limit.
    GraphLimitExceeded {
        /// Kind of submitted work.
        kind: GraphLimit,
        /// Submitted count, including inactive definitions.
        actual: usize,
        /// Configured maximum.
        limit: usize,
    },
    /// Summing submitted input-reference counts overflowed the platform count type.
    DependencyCountOverflow,
    /// More than one pass has the same logical identity.
    DuplicatePass(PassId),
    /// More than one external definition or pass writes the same texture identity.
    DuplicateTextureWriter(TextureId),
    /// More than one consumer has the same output identity.
    DuplicateOutput(OutputId),
    /// Source selector, polling interval or actual/maximum image contract is invalid.
    InvalidSourceBinding(TextureId),
    /// A pass references a texture that has no definition or writer.
    MissingPassInput {
        /// Pass containing the missing reference.
        pass: PassId,
        /// Undefined input texture.
        source: TextureId,
    },
    /// An output references a texture that has no definition or writer.
    MissingOutputSource {
        /// Consumer containing the missing reference.
        output: OutputId,
        /// Undefined demanded texture.
        source: TextureId,
    },
    /// A submitted texture dimension exceeds the configured maximum.
    TextureDimensionExceeded {
        /// Texture with the invalid extent.
        texture: TextureId,
        /// Submitted width.
        width: u32,
        /// Submitted height.
        height: u32,
        /// Maximum width and height.
        limit: u32,
    },
    /// A submitted descriptor has an invalid or overflowing byte representation.
    InvalidTexture {
        /// Texture with the invalid descriptor.
        texture: TextureId,
        /// Descriptor validation failure.
        source: TextureError,
    },
    /// Internal pass targets must use the linear premultiplied compositing contract.
    InvalidTargetContract {
        /// Pass declaring an incompatible target.
        pass: PassId,
        /// Target texture identity.
        output: TextureId,
    },
    /// A cycle prevents ordering part of the submitted graph.
    Cycle {
        /// Sorted passes left blocked, including downstream dependents, not exact cycle members.
        blocked_passes: Vec<PassId>,
    },
    /// Summed demanded texture bytes cannot be represented by `u64`.
    TextureFootprintOverflow,
    /// Demanded texture references exceed the explicit preparation budget.
    TextureBudgetExceeded {
        /// Required bytes, counting each demanded logical texture once.
        actual: u64,
        /// Configured maximum bytes.
        limit: u64,
    },
}

impl fmt::Display for PlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GraphLimitExceeded { kind, actual, limit } => {
                write!(formatter, "submitted {kind} count {actual} exceeds limit {limit}")
            }
            Self::DependencyCountOverflow => formatter.write_str("submitted dependency count overflow"),
            Self::DuplicatePass(id) => write!(formatter, "duplicate render pass {}", id.get()),
            Self::DuplicateTextureWriter(id) => write!(formatter, "multiple writers for texture {}", id.get()),
            Self::DuplicateOutput(id) => write!(formatter, "duplicate plan output {}", id.get()),
            Self::MissingPassInput { pass, source } => {
                write!(
                    formatter,
                    "pass {} references missing texture {}",
                    pass.get(),
                    source.get()
                )
            }
            Self::MissingOutputSource { output, source } => {
                write!(
                    formatter,
                    "output {} references missing texture {}",
                    output.get(),
                    source.get()
                )
            }
            Self::TextureDimensionExceeded {
                texture,
                width,
                height,
                limit,
            } => write!(
                formatter,
                "texture {} extent {width}x{height} exceeds dimension limit {limit}",
                texture.get()
            ),
            Self::InvalidTexture { texture, source } => write!(formatter, "texture {}: {source}", texture.get()),
            Self::InvalidSourceBinding(texture) => {
                write!(formatter, "texture {} has an invalid source binding", texture.get())
            }
            Self::InvalidTargetContract { pass, output } => write!(
                formatter,
                "pass {} target {} must be linear and premultiplied",
                pass.get(),
                output.get()
            ),
            Self::Cycle { blocked_passes } => {
                formatter.write_str("render graph contains a cycle; blocked passes [")?;
                for (index, id) in blocked_passes.iter().enumerate() {
                    if index != 0 {
                        formatter.write_str(", ")?;
                    }
                    write!(formatter, "{}", id.get())?;
                }
                formatter.write_str("]")
            }
            Self::TextureFootprintOverflow => formatter.write_str("demanded texture footprint overflow"),
            Self::TextureBudgetExceeded { actual, limit } => {
                write!(
                    formatter,
                    "demanded texture footprint {actual} exceeds byte limit {limit}"
                )
            }
        }
    }
}

impl Error for PlanError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidTexture { source, .. } => Some(source),
            _ => None,
        }
    }
}
