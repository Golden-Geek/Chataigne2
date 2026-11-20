use super::*;
use golden_video::plan::{GraphLimit, PlanError, PlanLimits, PlanOutput, RenderPlan};
use golden_video::shader::FragmentFormat;
use golden_video::{Extent2D, OutputId};

#[test]
fn float_pass_uses_an_ordered_budgeted_target_and_one_effect_clock() {
    let program = FragmentEffect::compile(
        FragmentFormat::Isf,
        r#"/*{
        "PASSES":[{"TARGET":"scale","WIDTH":"$WIDTH / 2","HEIGHT":"$HEIGHT / 2","FLOAT":true},{}]
    }*/
    void main() {
        gl_FragColor = PASSINDEX == 0 ? vec4(2.0, 0.0, 0.0, 1.0) : IMG_THIS_NORM_PIXEL(scale);
    }"#,
    )
    .unwrap();
    let output = TextureDescriptor {
        extent: Extent2D::new(8, 4).unwrap(),
        format: PixelFormat::Rgba8Unorm,
        color_encoding: ColorEncoding::Linear,
        alpha_mode: AlphaMode::Premultiplied,
    };
    let passes = Passes::new(&program, output, 8, &Default::default()).unwrap();
    assert_eq!(passes.work(&[None; TEXTURE_INPUTS]), (1, 3));
    let mut entry = Entry {
        target: TextureId::new(1).unwrap(),
        source: None,
        #[cfg(feature = "video-files")]
        subtitles: None,
        sender_external_only: false,
        scratch: None,
        shader_buffers: Vec::new(),
    };
    let mut next = 1;
    let mut plan = RenderPlan::default();
    let operation = passes
        .append(&mut plan.passes, &mut entry, [None; TEXTURE_INPUTS], &mut next)
        .unwrap();
    let final_id = PassId::new(1).unwrap();
    plan.passes.push(RenderPass {
        id: final_id,
        output: entry.target,
        descriptor: output,
        operation,
    });
    plan.outputs.push(PlanOutput {
        id: OutputId::new(1).unwrap(),
        source: entry.target,
    });

    let mut limits = PlanLimits {
        max_passes: 1,
        max_external_textures: 0,
        max_outputs: 1,
        max_dependencies: 3,
        max_texture_dimension: 8,
        max_texture_bytes: 256,
    };
    assert!(matches!(
        plan.clone().compile(&limits),
        Err(PlanError::GraphLimitExceeded {
            kind: GraphLimit::Passes,
            ..
        })
    ));
    limits.max_passes = 2;
    let compiled = plan.compile(&limits).unwrap();
    assert_eq!(compiled.footprint().target_texture_bytes, 256);
    let [temporary, final_pass] = compiled.passes() else {
        panic!("expected two ordered draws")
    };
    assert_eq!(temporary.descriptor.extent, Extent2D::new(4, 2).unwrap());
    let RenderOperation::Shader { clock, index, .. } = &temporary.operation else {
        panic!()
    };
    assert_eq!((*clock, *index), (final_id, 0));
    let RenderOperation::Shader {
        clock, index, inputs, ..
    } = &final_pass.operation
    else {
        panic!()
    };
    assert_eq!((*clock, *index), (final_id, 1));
    assert_eq!(inputs[TEXTURE_INPUTS], Some(ShaderTexture::Current(temporary.output)));
}

#[test]
fn a_single_named_persistent_pass_reads_its_own_final_target() {
    let program = FragmentEffect::compile(
        FragmentFormat::Isf,
        r#"/*{
        "PASSES":[{"TARGET":"trail","PERSISTENT":true}]
    }*/
    void main() { gl_FragColor = IMG_THIS_NORM_PIXEL(trail); }"#,
    )
    .unwrap();
    let output = TextureDescriptor {
        extent: Extent2D::new(8, 4).unwrap(),
        format: PixelFormat::Rgba8Unorm,
        color_encoding: ColorEncoding::Linear,
        alpha_mode: AlphaMode::Premultiplied,
    };
    let passes = Passes::new(&program, output, 8, &Default::default()).unwrap();
    let mut entry = Entry {
        target: TextureId::new(1).unwrap(),
        source: None,
        #[cfg(feature = "video-files")]
        subtitles: None,
        sender_external_only: false,
        scratch: None,
        shader_buffers: Vec::new(),
    };
    let mut intermediates = Vec::new();
    let operation = passes
        .append(&mut intermediates, &mut entry, [None; TEXTURE_INPUTS], &mut 1)
        .unwrap();
    assert!(intermediates.is_empty());
    let RenderOperation::Shader { inputs, .. } = operation else {
        panic!()
    };
    assert_eq!(inputs[TEXTURE_INPUTS], Some(ShaderTexture::PreviousFrame(entry.target)));
}
