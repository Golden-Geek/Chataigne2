use golden_video::plan::{
    BlendMode, ExternalTexture, Layer, PlanLimits, PlanOutput, RenderOperation, RenderPass, RenderPlan, SampleFilter,
};
use golden_video::{
    AlphaMode, ColorEncoding, Extent2D, LinearColor, Opacity, OutputId, PassId, PixelFormat, TextureDescriptor,
    TextureId,
};

#[test]
fn public_plan_contract_shares_a_composition_across_outputs_and_preserves_source_metadata() {
    let source = TextureId::new(1).unwrap();
    let sampled = TextureId::new(2).unwrap();
    let composition = TextureId::new(3).unwrap();
    let unused = TextureId::new(4).unwrap();
    let source_descriptor = TextureDescriptor {
        extent: Extent2D::new(1920, 1080).unwrap(),
        format: PixelFormat::Bgra8Unorm,
        color_encoding: ColorEncoding::Srgb,
        alpha_mode: AlphaMode::Straight,
    };
    let target_descriptor = TextureDescriptor {
        color_encoding: ColorEncoding::Linear,
        alpha_mode: AlphaMode::Premultiplied,
        ..source_descriptor
    };
    let plan = RenderPlan {
        external_textures: vec![ExternalTexture {
            id: source,
            descriptor: source_descriptor,
            source: None,
        }],
        passes: vec![
            RenderPass {
                id: PassId::new(1).unwrap(),
                output: composition,
                descriptor: target_descriptor,
                operation: RenderOperation::Composite {
                    layers: vec![Layer {
                        source: sampled.into(),
                        opacity: Opacity::new(0.5).unwrap(),
                        opacity_envelope: None,
                        blend: BlendMode::Normal,
                        filter: SampleFilter::Linear,
                        mapping: None,
                    }],
                },
            },
            RenderPass {
                id: PassId::new(2).unwrap(),
                output: sampled,
                descriptor: target_descriptor,
                operation: RenderOperation::Sample {
                    source,
                    filter: SampleFilter::Linear,
                },
            },
            RenderPass {
                id: PassId::new(3).unwrap(),
                output: unused,
                descriptor: target_descriptor,
                operation: RenderOperation::Clear(LinearColor::TRANSPARENT),
            },
        ],
        outputs: vec![
            PlanOutput {
                id: OutputId::new(1).unwrap(),
                source: composition,
            },
            PlanOutput {
                id: OutputId::new(2).unwrap(),
                source: composition,
            },
        ],
    };
    let compiled = plan
        .compile(&PlanLimits {
            max_passes: 8,
            max_external_textures: 8,
            max_outputs: 8,
            max_dependencies: 32,
            max_texture_dimension: 16_384,
            max_texture_bytes: 64 * 1024 * 1024,
        })
        .unwrap();

    assert_eq!(compiled.outputs().len(), 2);
    assert_eq!(compiled.external_textures()[0].descriptor, source_descriptor);
    assert_eq!(
        compiled.passes().iter().map(|pass| pass.id.get()).collect::<Vec<_>>(),
        [2, 1]
    );
    let bytes = source_descriptor.byte_len().unwrap();
    assert_eq!(compiled.footprint().external_texture_bytes, bytes);
    assert_eq!(compiled.footprint().target_texture_bytes, bytes * 2);
    assert_eq!(compiled.footprint().total_texture_bytes, bytes * 3);
}

#[test]
fn logical_ids_do_not_use_zero_for_a_live_resource() {
    assert!(TextureId::new(0).is_none());
    assert!(PassId::new(0).is_none());
    assert!(OutputId::new(0).is_none());
    assert_eq!(TextureId::new(u64::MAX).unwrap().get(), u64::MAX);
}
