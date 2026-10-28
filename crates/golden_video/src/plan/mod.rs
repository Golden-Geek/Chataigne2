//! Immutable, app-neutral render plans prepared before any backend resources are created.

mod compile;
mod error;
mod weight;

pub use error::{GraphLimit, PlanError};

use crate::{LinearColor, Opacity, OutputId, PassId, TextureDescriptor, TextureId};

/// Sampling applied when a source extent differs from the destination extent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SampleFilter {
    /// Select the nearest source texel.
    Nearest,
    /// Interpolate adjacent source texels.
    Linear,
}

/// App-neutral compositing modes, applied in authored layer order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlendMode {
    /// Normal alpha-over composition.
    Normal,
    /// Porter-Duff Plus: add linear premultiplied RGBA and saturate each component to one.
    Add,
    /// Multiply color blending.
    Multiply,
    /// Screen color blending.
    Screen,
}

/// A full-target sampled layer; geometry belongs to later exercised operations.
#[derive(Clone, Debug)]
pub struct Layer {
    /// Texture sampled by this layer.
    pub source: TextureId,
    /// Layer opacity applied before blending.
    pub opacity: Opacity,
    /// Blend mode against the accumulated destination.
    pub blend: BlendMode,
    /// Sampling when the source and destination extents differ.
    pub filter: SampleFilter,
}

/// A generic pass operation; no app node or native handle crosses this boundary.
#[derive(Clone, Debug)]
pub enum RenderOperation {
    /// Fill the output with a finite linear premultiplied color.
    Clear(LinearColor),
    /// Sample one texture over the entire output, converting its declared color/alpha contract.
    Sample {
        /// Texture sampled by the pass.
        source: TextureId,
        /// Sampling when the source and destination extents differ.
        filter: SampleFilter,
    },
    /// Composite ordered full-target layers over an initially transparent destination.
    Composite {
        /// Layers in draw order, first layer drawn first.
        layers: Vec<Layer>,
    },
}

impl RenderOperation {
    fn dependency_count(&self) -> usize {
        match self {
            Self::Clear(_) => 0,
            Self::Sample { .. } => 1,
            Self::Composite { layers } => layers.len(),
        }
    }

    fn sources(&self) -> impl Iterator<Item = TextureId> + '_ {
        let (sample, layers) = match self {
            Self::Clear(_) => (None, &[][..]),
            Self::Sample { source, .. } => (Some(*source), &[][..]),
            Self::Composite { layers } => (None, layers.as_slice()),
        };
        sample.into_iter().chain(layers.iter().map(|layer| layer.source))
    }
}

/// A source texture prepared outside this render graph.
#[derive(Clone, Debug)]
pub struct ExternalTexture {
    /// Stable logical identity within this plan.
    pub id: TextureId,
    /// Explicit extent, storage, color and alpha contract.
    pub descriptor: TextureDescriptor,
}

/// One writer producing a linear premultiplied target texture.
#[derive(Clone, Debug)]
pub struct RenderPass {
    /// Stable logical pass identity used to break topological-order ties.
    pub id: PassId,
    /// Sole texture produced by this pass.
    pub output: TextureId,
    /// Output allocation contract; must be linear and premultiplied.
    pub descriptor: TextureDescriptor,
    /// Work producing the target texture.
    pub operation: RenderOperation,
}

/// Explicit demand for a texture, without allocating a duplicate presentation texture.
#[derive(Clone, Debug)]
pub struct PlanOutput {
    /// Stable consumer identity within this plan.
    pub id: OutputId,
    /// Texture demanded by this consumer.
    pub source: TextureId,
}

/// Unprepared render graph. Every declared output is an explicit demand root.
#[derive(Clone, Debug, Default)]
pub struct RenderPlan {
    /// External texture definitions, including currently undemanded sources.
    pub external_textures: Vec<ExternalTexture>,
    /// Pass definitions; submission order does not determine execution order.
    pub passes: Vec<RenderPass>,
    /// Demanded textures. An empty list produces an empty executable plan.
    pub outputs: Vec<PlanOutput>,
}

impl RenderPlan {
    /// Validates the entire graph, then consumes it into an immutable demanded execution plan.
    ///
    /// Rejection creates no backend resources and cannot change another compiled/live plan.
    /// Cycles and invalid metadata are rejected even in currently undemanded branches.
    pub fn compile(self, limits: &PlanLimits) -> Result<CompiledPlan, PlanError> {
        compile::compile(self, limits)
    }
}

/// Explicit preparation limits. These do not select arbitrary device/runtime budgets.
#[derive(Clone, Copy, Debug)]
pub struct PlanLimits {
    /// Maximum submitted passes, including undemanded passes.
    pub max_passes: usize,
    /// Maximum submitted external texture definitions, including undemanded sources.
    pub max_external_textures: usize,
    /// Maximum submitted output declarations.
    pub max_outputs: usize,
    /// Maximum submitted input references, including repeated samples and undemanded branches.
    pub max_dependencies: usize,
    /// Maximum width or height of any submitted texture.
    pub max_texture_dimension: u32,
    /// Maximum summed byte footprint of distinct demanded external textures and targets.
    pub max_texture_bytes: u64,
}

/// Conservative references for this graph, not physical residency or whole-runtime admission.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TextureFootprint {
    /// Bytes described by distinct demanded external textures.
    pub external_texture_bytes: u64,
    /// Bytes described by distinct demanded pass targets.
    pub target_texture_bytes: u64,
    /// External and target texture bytes combined.
    pub total_texture_bytes: u64,
}

/// Validated immutable graph containing only demanded resources and passes.
#[derive(Clone, Debug)]
pub struct CompiledPlan {
    external_textures: Box<[ExternalTexture]>,
    passes: Box<[RenderPass]>,
    outputs: Box<[PlanOutput]>,
    footprint: TextureFootprint,
}

impl CompiledPlan {
    /// Demanded external textures, sorted by texture identity.
    pub fn external_textures(&self) -> &[ExternalTexture] {
        &self.external_textures
    }

    /// Demanded passes in dependency order, with ready-pass ties broken by pass identity.
    pub fn passes(&self) -> &[RenderPass] {
        &self.passes
    }

    /// Explicit output demands, sorted by consumer identity.
    pub fn outputs(&self) -> &[PlanOutput] {
        &self.outputs
    }

    /// Conservative graph texture references, counted once per demanded logical texture.
    pub fn footprint(&self) -> TextureFootprint {
        self.footprint
    }
}

#[cfg(test)]
mod tests;
