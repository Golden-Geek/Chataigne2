//! Fixed qualification scene wiring; no product media or mapping policy enters Golden.

use std::{num::NonZeroUsize, time::Duration};

use golden_io::WorkerNotifier;
use golden_time::RationalRate;
use golden_video::plan::{
    BlitRegion, ExternalSource, ExternalTexture, PlanLimits, PlanOutput, RenderOperation, RenderPass, RenderPlan,
    SampleFilter, SpoutSource, UnitRect,
};
use golden_video::preview::PreviewSnapshot;
use golden_video::runtime::{PlanSubmission, ResourceLimits, RuntimeConfig};
use golden_video::{AlphaMode, ColorEncoding, Extent2D, OutputId, PassId, PixelFormat, TextureDescriptor, TextureId};

pub const MAIN_OUTPUT: u64 = 1;
pub const SPOUT_OUTPUT: u64 = 2;
pub const EDITOR_OUTPUT: u64 = 3;
pub const SOURCE_KEYS: [&str; 2] = ["source-a", "source-b"];
pub const PREVIEW_PIXELS: [[u32; 2]; 2] = [[640, 360], [480, 270]];

pub fn output(id: u64) -> OutputId {
    OutputId::new(id).unwrap()
}

fn texture(id: u64) -> TextureId {
    TextureId::new(id).unwrap()
}

pub fn source_descriptor(extent: Extent2D) -> TextureDescriptor {
    TextureDescriptor {
        extent,
        format: PixelFormat::Bgra8Unorm,
        color_encoding: ColorEncoding::Srgb,
        alpha_mode: AlphaMode::Straight,
    }
}

pub fn source_extents(output: Extent2D) -> [Extent2D; 2] {
    [output, Extent2D::new(1280, 720).unwrap()]
}

pub fn input_bytes(output: Extent2D) -> u64 {
    source_extents(output)
        .into_iter()
        .map(|extent| source_descriptor(extent).byte_len().unwrap() * 5)
        .sum()
}

pub fn config(max_bytes: u64, notifier: WorkerNotifier) -> RuntimeConfig {
    RuntimeConfig {
        plan_limits: PlanLimits {
            max_passes: 4,
            max_external_textures: 2,
            max_outputs: 3,
            max_dependencies: 16,
            max_texture_dimension: 4096,
            max_texture_bytes: max_bytes,
        },
        resource_limits: ResourceLimits {
            max_bytes,
            max_in_flight: NonZeroUsize::new(2).unwrap(),
            max_retiring: 2,
        },
        frame_rate: RationalRate::new(60, 1).unwrap(),
        max_outstanding: NonZeroUsize::new(16).unwrap(),
        max_input_bytes: NonZeroUsize::new(64 * 1024).unwrap(),
        max_retained_input_bytes: NonZeroUsize::new(512 * 1024).unwrap(),
        recovery_delay: Duration::from_millis(100),
        max_recovery_delay: Duration::from_secs(2),
        notifier: Some(notifier),
    }
}

pub fn plan(
    sources: &[String; 2],
    output_extent: Extent2D,
    editor: Extent2D,
    preview: &PreviewSnapshot,
) -> PlanSubmission {
    let internal = |extent| TextureDescriptor {
        extent,
        format: PixelFormat::Rgba8Unorm,
        color_encoding: ColorEncoding::Linear,
        alpha_mode: AlphaMode::Premultiplied,
    };
    let external_textures = source_extents(output_extent)
        .into_iter()
        .enumerate()
        .map(|(index, extent)| {
            let descriptor = source_descriptor(extent);
            ExternalTexture {
                id: texture(index as u64 + 1),
                descriptor,
                source: Some(ExternalSource::Spout(SpoutSource {
                    sender: sources[index].clone(),
                    descriptor,
                    poll_interval: Duration::from_millis(1),
                })),
            }
        })
        .collect();
    let halves = (0..2)
        .map(|index| BlitRegion {
            source: texture(index + 1),
            destination: UnitRect {
                x: index as f32 * 0.5,
                y: 0.,
                width: 0.5,
                height: 1.,
            },
            source_rect: UnitRect::FULL,
            filter: SampleFilter::Linear,
        })
        .collect();
    PlanSubmission::new(RenderPlan {
        external_textures,
        passes: vec![
            RenderPass {
                id: PassId::new(1).unwrap(),
                output: texture(3),
                descriptor: internal(output_extent),
                operation: RenderOperation::Regions {
                    regions: halves,
                    visibility: None,
                },
            },
            RenderPass {
                id: PassId::new(2).unwrap(),
                output: texture(4),
                descriptor: internal(editor),
                operation: RenderOperation::Regions {
                    regions: preview.regions(|key| {
                        SOURCE_KEYS
                            .iter()
                            .position(|source| *source == key)
                            .map(|index| texture(index as u64 + 1))
                    }),
                    visibility: Some(preview.guard.clone()),
                },
            },
        ],
        outputs: vec![
            PlanOutput {
                id: output(MAIN_OUTPUT),
                source: texture(3),
            },
            PlanOutput {
                id: output(SPOUT_OUTPUT),
                source: texture(3),
            },
            PlanOutput {
                id: output(EDITOR_OUTPUT),
                source: texture(4),
            },
        ],
    })
}
