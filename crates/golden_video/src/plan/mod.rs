//! Immutable, app-neutral render plans prepared before any backend resources are created.

pub(crate) mod canvas;
mod compile;
mod envelope;
mod error;
mod mapping;
mod overlay;
mod pattern;
mod placement;
mod region;
mod region_fit;
mod weight;

pub use canvas::{
    CanvasPaint, CanvasPattern, CanvasPrimitive, CanvasShape, CanvasText, CanvasTextStyle, MAX_CANVAS_PRIMITIVES,
    MAX_CANVAS_TEXT_BYTES,
};
pub use envelope::{InterpolationCurve, OpacityEnvelope};
pub use error::{GraphLimit, PlanError};
pub use mapping::{MappingError, Point, ProjectiveMapping, Quad};
pub use overlay::{MAX_VIEW_OVERLAYS, OverlayPrimitive, OverlayShape};
pub use pattern::{GridPattern, LayerSource, Pattern};
pub use placement::{MAX_REGION_TILES, RegionGeometry, RegionPlacement, RegionTile};
pub use region::{BlitRegion, UnitRect};
pub use region_fit::RegionFit;

use crate::{LinearColor, Opacity, OutputId, PassId, TextureDescriptor, TextureId};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use ts_rs::TS;

/// Sampling applied when a source extent differs from the destination extent.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
pub enum SampleFilter {
    /// Select the nearest source texel.
    Nearest,
    /// Interpolate adjacent source texels.
    Linear,
}

/// App-neutral compositing modes, applied in authored layer order.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
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

/// An ordered sampled or procedural layer, optionally cropped and mapped into a perspective quad.
#[derive(Clone, Debug)]
pub struct Layer {
    /// Sampled texture or generated procedural content.
    pub source: LayerSource,
    /// Layer opacity applied before blending.
    pub opacity: Opacity,
    /// Optional shared-time factor multiplied by authored opacity on each render submission.
    pub opacity_envelope: Option<OpacityEnvelope>,
    /// Blend mode against the accumulated destination.
    pub blend: BlendMode,
    /// Sampling when the source and destination extents differ.
    pub filter: SampleFilter,
    /// Checked during preparation, never solved on the rendering thread.
    pub mapping: Option<ProjectiveMapping>,
}

impl Layer {
    /// Resolves shared-time opacity and procedural clocks without allocating or advancing time.
    pub fn evaluated(&self, now: golden_time::TimeSnapshot) -> Self {
        let mut result = self.clone();
        if let LayerSource::Pattern(Pattern::Canvas(canvas)) = &mut result.source {
            canvas.sample_time(now);
        }
        if let Some(envelope) = self.opacity_envelope {
            result.opacity =
                Opacity::new(self.opacity.get() * envelope.sample(now).get()).expect("checked opacity product");
            result.opacity_envelope = None;
        }
        result
    }
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
    /// Composite ordered sampled/procedural layers over an initially transparent destination.
    Composite {
        /// Layers in draw order, first layer drawn first.
        layers: Vec<Layer>,
    },
    /// Clear the target, then replace ordered bounded regions. Empty regions clear stale views.
    Regions {
        /// Painter order; each source crop is stretched only into its destination rectangle.
        regions: Vec<BlitRegion>,
        /// Optional per-frame presentation lifetime. An invalidated fence clears this pass
        /// without drawing its regions, even while an older plan remains active. It does not
        /// change source demand, resource admission or any independent output.
        visibility: Option<golden_io::RevisionGuard>,
    },
}

impl RenderOperation {
    pub(crate) fn canvas_images(&self) -> impl Iterator<Item = &std::sync::Arc<crate::image::ImageData>> {
        let layers = match self {
            Self::Composite { layers } => layers.as_slice(),
            _ => &[],
        };
        layers.iter().filter_map(|layer| match &layer.source {
            LayerSource::Pattern(Pattern::Canvas(canvas)) => canvas.image(),
            _ => None,
        })
    }
    fn dependency_count(&self) -> usize {
        match self {
            Self::Clear(_) => 0,
            Self::Sample { .. } => 1,
            Self::Composite { layers } => layers.iter().fold(0_usize, |count, layer| {
                let work = match &layer.source {
                    LayerSource::Pattern(Pattern::Canvas(canvas)) => canvas.primitives().len(),
                    _ => 1,
                };
                count.saturating_add(work)
            }),
            Self::Regions { regions, .. } => regions.len(),
        }
    }

    pub(crate) fn sources(&self) -> impl Iterator<Item = TextureId> + '_ {
        let (sample, layers, regions) = match self {
            Self::Clear(_) => (None, &[][..], &[][..]),
            Self::Sample { source, .. } => (Some(*source), &[][..], &[][..]),
            Self::Composite { layers } => (None, layers.as_slice(), &[][..]),
            Self::Regions { regions, .. } => (None, &[][..], regions.as_slice()),
        };
        sample
            .into_iter()
            .chain(layers.iter().filter_map(|layer| layer.source.texture()))
            .chain(regions.iter().map(|region| region.source))
    }
}

/// A source texture prepared outside this render graph.
#[derive(Clone, Debug)]
pub struct ExternalTexture {
    /// Stable logical identity within this plan.
    pub id: TextureId,
    /// Explicit extent, storage, color and alpha contract.
    pub descriptor: TextureDescriptor,
    /// Optional prepared source, installed atomically with this plan. Null backends simulate it.
    pub source: Option<ExternalSource>,
}

/// A source's immutable CPU admission contract. Native resources remain worker-owned.
#[derive(Clone, Debug)]
pub enum ExternalSource {
    /// Bounded asynchronous GPU receiver subscription.
    Spout(SpoutSource),
    /// Immutable decoder pixels and their preparation/resource lifetime lease.
    Image(golden_assets::PreparedAsset<crate::image::ImageData>),
}

impl ExternalSource {
    /// Returns a GPU receiver subscription, excluding immutable image uploads.
    pub fn spout(&self) -> Option<&SpoutSource> {
        match self {
            Self::Spout(source) => Some(source),
            Self::Image(_) => None,
        }
    }

    pub(crate) fn current(&self) -> bool {
        match self {
            Self::Spout(_) => true,
            Self::Image(image) => image.is_current(),
        }
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        match self {
            Self::Spout(source) => source.sender.capacity(),
            Self::Image(image) => image.retained_bytes(),
        }
    }
}

impl From<SpoutSource> for ExternalSource {
    fn from(source: SpoutSource) -> Self {
        Self::Spout(source)
    }
}

/// Explicit endpoint counter capability; semaphore presence never establishes this contract.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpoutFrameCount {
    /// No versioned counter guarantee. Every actual successful copy can publish, even when
    /// a foreign sender's counter is missing, positive or stale.
    #[default]
    Unversioned,
    /// Caller guarantees this endpoint and every replacement use the version-one instance-local
    /// counting contract. Use only for controlled peers with names at most 239 bytes (room for
    /// the SDK counter suffix); source-handle changes reset observations.
    VersionedV1,
}

impl SpoutFrameCount {
    pub(crate) fn accepts_sender_name(self, name: &str) -> bool {
        // Pinned SDK counter storage is 256 bytes including its 16-byte suffix and terminator.
        let limit = match self {
            Self::Unversioned => 255,
            Self::VersionedV1 => 239,
        };
        !name.is_empty() && name.len() <= limit && !name.contains('\0')
    }
}

/// Immutable Spout subscription. Changing this contract requires a fresh logical texture ID
/// while any prepared/live/retiring generation still retains the old binding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpoutSource {
    /// Exact sender selector; never the SDK's active-sender fallback.
    pub sender: String,
    /// Maximum extent. Storage/transfer/alpha are fixed only in declared metadata mode;
    /// discovery replaces them with admitted native metadata.
    pub descriptor: TextureDescriptor,
    /// Storage discovery or caller-declared transfer/alpha interpretation.
    pub metadata: SpoutMetadata,
    /// Dedicated source-worker polling interval, between one millisecond and one second.
    pub poll_interval: Duration,
    /// Explicit peer capability, supplied by the endpoint owner rather than inferred at runtime.
    pub frame_count: SpoutFrameCount,
}

/// Classic Spout byte textures carry storage format but no transfer/alpha metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpoutMetadata {
    /// Require the caller's fixed storage, transfer and alpha contract.
    Declared,
    /// Discover BGRA/RGBA byte storage and interpret it as sRGB with straight alpha.
    Discover,
}

impl SpoutSource {
    /// Only fixed contracts reserve a maximum before native metadata is available.
    pub(crate) fn reserved_descriptor(&self) -> Option<TextureDescriptor> {
        (self.metadata == SpoutMetadata::Declared).then_some(self.descriptor)
    }

    /// Admits native storage metadata before any receiving pool is allocated.
    pub fn observed_descriptor(
        &self,
        extent: crate::Extent2D,
        format: crate::PixelFormat,
    ) -> Option<TextureDescriptor> {
        let descriptor = match self.metadata {
            SpoutMetadata::Declared => TextureDescriptor {
                extent,
                format,
                ..self.descriptor
            },
            SpoutMetadata::Discover => TextureDescriptor {
                extent,
                format,
                color_encoding: crate::ColorEncoding::Srgb,
                alpha_mode: crate::AlphaMode::Straight,
            },
        };
        self.accepts(descriptor).then_some(descriptor)
    }

    pub(crate) fn accepts(&self, actual: TextureDescriptor) -> bool {
        let maximum = self.descriptor;
        actual.format != crate::PixelFormat::Rgba16Float
            && actual.extent.width() <= maximum.extent.width()
            && actual.extent.height() <= maximum.extent.height()
            && match self.metadata {
                SpoutMetadata::Declared => {
                    actual.format == maximum.format
                        && actual.color_encoding == maximum.color_encoding
                        && actual.alpha_mode == maximum.alpha_mode
                }
                SpoutMetadata::Discover => {
                    actual.color_encoding == crate::ColorEncoding::Srgb
                        && actual.alpha_mode == crate::AlphaMode::Straight
                }
            }
    }
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
        self.compile_with_source_demands(limits, &[])
    }

    /// Keeps explicit external sources demanded even when no presentation consumes them.
    pub fn compile_with_source_demands(
        self,
        limits: &PlanLimits,
        source_demands: &[TextureId],
    ) -> Result<CompiledPlan, PlanError> {
        compile::compile(self, limits, source_demands)
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
    /// Maximum submitted source input/draw units, including repeated samples, procedural layers
    /// and undemanded branches. Attached decorations have separate fixed RegionPlacement capacity.
    pub max_dependencies: usize,
    /// Maximum width or height of any submitted texture.
    pub max_texture_dimension: u32,
    /// Maximum summed byte footprint of distinct demanded external textures and targets.
    pub max_texture_bytes: u64,
}

/// Conservative references for this graph, not physical residency or whole-runtime admission.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TextureFootprint {
    /// Bytes described by distinct demanded sampled inputs, including immutable canvas bitmaps.
    pub external_texture_bytes: u64,
    /// Bytes described by distinct demanded pass targets.
    pub target_texture_bytes: u64,
    /// External and target texture bytes combined.
    pub total_texture_bytes: u64,
}

/// Validated immutable graph containing only demanded resources and passes.
#[derive(Clone, Debug)]
pub struct CompiledPlan {
    pub(crate) guards: std::sync::Arc<[golden_io::RevisionGuard]>,
    external_textures: Box<[ExternalTexture]>,
    passes: Box<[RenderPass]>,
    outputs: Box<[PlanOutput]>,
    footprint: TextureFootprint,
    // Last field: signal final-use release after all immutable plan inputs are dropped too.
    pub(crate) lifetime: Option<crate::runtime::PlanLifetime>,
}

impl CompiledPlan {
    /// Whether captured authoring sources still permit initial installation/reconstruction.
    pub fn admission_is_current(&self) -> bool {
        self.guards.iter().all(golden_io::RevisionGuard::is_current)
            && self
                .external_textures
                .iter()
                .all(|texture| texture.source.as_ref().is_none_or(ExternalSource::current))
    }
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
