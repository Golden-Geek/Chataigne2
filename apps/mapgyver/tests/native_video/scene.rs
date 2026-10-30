use std::num::NonZeroUsize;
use std::time::Duration;

use golden_time::RationalRate;
use golden_video::plan::{
    BlendMode, Layer, PlanLimits, PlanOutput, RenderOperation, RenderPass, RenderPlan, SampleFilter,
};
use golden_video::runtime::{ResourceLimits, RuntimeConfig};
use golden_video::{
    AlphaMode, ColorEncoding, Extent2D, LinearColor, Opacity, OutputId, PassId, PixelFormat, TextureDescriptor,
    TextureId,
};

pub fn output(value: u64) -> OutputId {
    OutputId::new(value).unwrap()
}

pub fn descriptor(width: u32, height: u32, format: PixelFormat) -> TextureDescriptor {
    TextureDescriptor {
        extent: Extent2D::new(width, height).unwrap(),
        format,
        color_encoding: ColorEncoding::Linear,
        alpha_mode: AlphaMode::Premultiplied,
    }
}

pub fn config() -> RuntimeConfig {
    RuntimeConfig {
        plan_limits: PlanLimits {
            max_passes: 16,
            max_external_textures: 0,
            max_outputs: 2,
            max_dependencies: 32,
            max_texture_dimension: 2048,
            max_texture_bytes: 16 * 1024 * 1024,
        },
        resource_limits: ResourceLimits {
            max_bytes: 64 * 1024 * 1024,
            max_in_flight: NonZeroUsize::new(2).unwrap(),
            max_retiring: 2,
        },
        frame_rate: RationalRate::new(60, 1).unwrap(),
        max_outstanding: NonZeroUsize::new(4).unwrap(),
        max_input_bytes: NonZeroUsize::new(256 * 1024).unwrap(),
        max_retained_input_bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
        recovery_delay: Duration::from_millis(100),
        max_recovery_delay: Duration::from_secs(1),
        notifier: None,
    }
}

/// Both outputs demand the same final producer; presentation conversion remains backend-owned.
pub fn plan(outputs: &[u64]) -> RenderPlan {
    let texture = |value| TextureId::new(value).unwrap();
    let pass = |value| PassId::new(value).unwrap();
    let background = RenderPass {
        id: pass(1),
        output: texture(1),
        descriptor: descriptor(320, 180, PixelFormat::Bgra8Unorm),
        operation: RenderOperation::Clear(LinearColor::from_straight(0.08, 0.24, 0.48, 1.0).unwrap()),
    };
    let accent = RenderPass {
        id: pass(2),
        output: texture(2),
        descriptor: descriptor(128, 72, PixelFormat::Rgba8Unorm),
        operation: RenderOperation::Clear(LinearColor::from_straight(0.65, 0.12, 0.03, 0.6).unwrap()),
    };
    let composite = RenderPass {
        id: pass(3),
        output: texture(3),
        descriptor: descriptor(320, 180, PixelFormat::Rgba8Unorm),
        operation: RenderOperation::Composite {
            layers: vec![
                Layer {
                    source: texture(1).into(),
                    opacity: Opacity::OPAQUE,
                    blend: BlendMode::Normal,
                    filter: SampleFilter::Nearest,
                    mapping: None,
                },
                Layer {
                    source: texture(2).into(),
                    opacity: Opacity::new(0.2).unwrap(),
                    blend: BlendMode::Add,
                    filter: SampleFilter::Linear,
                    mapping: None,
                },
                Layer {
                    source: texture(2).into(),
                    opacity: Opacity::new(0.3).unwrap(),
                    blend: BlendMode::Multiply,
                    filter: SampleFilter::Nearest,
                    mapping: None,
                },
                Layer {
                    source: texture(2).into(),
                    opacity: Opacity::new(0.4).unwrap(),
                    blend: BlendMode::Screen,
                    filter: SampleFilter::Linear,
                    mapping: None,
                },
            ],
        },
    };
    let sampled = RenderPass {
        id: pass(4),
        output: texture(4),
        descriptor: descriptor(320, 180, PixelFormat::Bgra8Unorm),
        operation: RenderOperation::Sample {
            source: texture(3),
            filter: SampleFilter::Linear,
        },
    };
    RenderPlan {
        external_textures: Vec::new(),
        // Deliberately unordered: preparation owns dependency ordering.
        passes: vec![sampled, composite, accent, background],
        outputs: outputs
            .iter()
            .map(|&id| PlanOutput {
                id: output(id),
                source: texture(4),
            })
            .collect(),
    }
}
