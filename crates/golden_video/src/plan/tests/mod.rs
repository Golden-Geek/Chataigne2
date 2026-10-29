use super::*;
use crate::{AlphaMode, ColorEncoding, Extent2D, PixelFormat};

mod topology;
mod validation;

fn texture(id: u64) -> TextureId {
    TextureId::new(id).unwrap()
}

fn pass_id(id: u64) -> PassId {
    PassId::new(id).unwrap()
}

fn output(id: u64, source: u64) -> PlanOutput {
    PlanOutput {
        id: OutputId::new(id).unwrap(),
        source: texture(source),
    }
}

fn descriptor(width: u32, height: u32) -> TextureDescriptor {
    TextureDescriptor {
        extent: Extent2D::new(width, height).unwrap(),
        format: PixelFormat::Bgra8Unorm,
        color_encoding: ColorEncoding::Linear,
        alpha_mode: AlphaMode::Premultiplied,
    }
}

fn external(id: u64) -> ExternalTexture {
    ExternalTexture {
        id: texture(id),
        descriptor: descriptor(2, 2),
        source: None,
    }
}

fn clear(id: u64, target: u64) -> RenderPass {
    RenderPass {
        id: pass_id(id),
        output: texture(target),
        descriptor: descriptor(2, 2),
        operation: RenderOperation::Clear(LinearColor::TRANSPARENT),
    }
}

fn sample(id: u64, target: u64, source: u64) -> RenderPass {
    RenderPass {
        operation: RenderOperation::Sample {
            source: texture(source),
            filter: SampleFilter::Nearest,
        },
        ..clear(id, target)
    }
}

fn layer(source: u64) -> Layer {
    Layer {
        source: texture(source),
        opacity: Opacity::OPAQUE,
        blend: BlendMode::Normal,
        filter: SampleFilter::Linear,
    }
}

fn limits() -> PlanLimits {
    PlanLimits {
        max_passes: 64,
        max_external_textures: 64,
        max_outputs: 64,
        max_dependencies: 256,
        max_texture_dimension: 1024,
        max_texture_bytes: 1024 * 1024,
    }
}

fn diamond() -> RenderPlan {
    RenderPlan {
        external_textures: vec![external(1)],
        passes: vec![
            RenderPass {
                operation: RenderOperation::Composite {
                    layers: vec![layer(3), layer(4), layer(2), layer(2)],
                },
                ..clear(1, 5)
            },
            sample(5, 4, 2),
            sample(90, 3, 1),
            sample(20, 2, 1),
        ],
        outputs: vec![output(2, 5), output(1, 4)],
    }
}

fn pass_order(plan: &CompiledPlan) -> Vec<u64> {
    plan.passes().iter().map(|pass| pass.id.get()).collect()
}
