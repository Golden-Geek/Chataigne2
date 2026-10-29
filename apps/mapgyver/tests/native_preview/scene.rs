use golden_video::plan::{PlanLimits, PlanOutput, RenderOperation, RenderPass, RenderPlan};
use golden_video::preview::PreviewSnapshot;
use golden_video::runtime::{PlanSubmission, ResourceLimits, RuntimeConfig};
use golden_video::{
    AlphaMode, ColorEncoding, Extent2D, LinearColor, OutputId, PassId, PixelFormat, TextureDescriptor, TextureId,
};
use std::{num::NonZeroUsize, sync::Arc, time::Duration};

pub fn config(notifier: golden_io::WorkerNotifier) -> RuntimeConfig {
    RuntimeConfig {
        plan_limits: PlanLimits {
            max_passes: 8,
            max_external_textures: 8,
            max_outputs: 2,
            max_dependencies: 256,
            max_texture_dimension: 1024,
            max_texture_bytes: 8 * 1024 * 1024,
        },
        resource_limits: ResourceLimits {
            max_bytes: 16 * 1024 * 1024,
            max_in_flight: NonZeroUsize::new(2).unwrap(),
            max_retiring: 2,
        },
        frame_rate: golden_time::RationalRate::new(60, 1).unwrap(),
        max_outstanding: NonZeroUsize::new(16).unwrap(),
        max_input_bytes: NonZeroUsize::new(64 * 1024).unwrap(),
        max_retained_input_bytes: NonZeroUsize::new(512 * 1024).unwrap(),
        recovery_delay: Duration::from_millis(50),
        max_recovery_delay: Duration::from_secs(1),
        notifier: Some(notifier),
    }
}
pub fn descriptor(width: u32, height: u32) -> TextureDescriptor {
    TextureDescriptor {
        extent: Extent2D::new(width, height).unwrap(),
        format: PixelFormat::Rgba8Unorm,
        color_encoding: ColorEncoding::Linear,
        alpha_mode: AlphaMode::Premultiplied,
    }
}
pub fn plan(snapshot: &PreviewSnapshot) -> PlanSubmission {
    let source = descriptor(8, 8);
    let canvas = descriptor(512, 320);
    let passes = vec![
        RenderPass {
            id: PassId::new(1).unwrap(),
            output: TextureId::new(1).unwrap(),
            descriptor: source,
            operation: RenderOperation::Clear(LinearColor::from_straight(1., 0., 0., 1.).unwrap()),
        },
        RenderPass {
            id: PassId::new(2).unwrap(),
            output: TextureId::new(2).unwrap(),
            descriptor: source,
            operation: RenderOperation::Clear(LinearColor::from_straight(0., 1., 0., 1.).unwrap()),
        },
        RenderPass {
            id: PassId::new(3).unwrap(),
            output: TextureId::new(3).unwrap(),
            descriptor: canvas,
            operation: RenderOperation::Regions {
                visibility: Some(snapshot.guard.clone()),
                regions: snapshot.regions(|source| match source {
                    "red" => TextureId::new(1),
                    "green" => TextureId::new(2),
                    _ => None,
                }),
            },
        },
    ];
    let mut submission = PlanSubmission::new(RenderPlan {
        external_textures: Vec::new(),
        passes,
        outputs: vec![
            PlanOutput {
                id: OutputId::new(1).unwrap(),
                source: TextureId::new(3).unwrap(),
            },
            PlanOutput {
                id: OutputId::new(2).unwrap(),
                source: TextureId::new(2).unwrap(),
            },
        ],
    });
    submission.guards = Arc::from([snapshot.guard.clone()]);
    submission
}
