use super::*;
use crate::shader::{FragmentEffect, FragmentFormat, FragmentProgram, TEXTURE_BINDINGS};

#[test]
fn delayed_cycle_demands_both_writers_and_charges_one_snapshot_per_target() {
    let program = FragmentProgram::compile_glsl("void main() { fragColor = texture(iChannel0, vec2(0.5)); }").unwrap();
    let shader = |id, source| {
        let mut inputs = [None; TEXTURE_BINDINGS];
        inputs[0] = Some(source);
        RenderPass {
            operation: RenderOperation::Shader {
                program: program.clone(),
                inputs,
                clock: pass_id(1),
                index: id as u8 - 1,
            },
            ..clear(id, id)
        }
    };
    let mut plan = RenderPlan {
        passes: vec![
            shader(1, ShaderTexture::PreviousFrame(texture(2))),
            shader(2, ShaderTexture::Current(texture(1))),
        ],
        outputs: vec![output(1, 1)],
        ..Default::default()
    };
    let mut limits = limits();
    limits.max_texture_bytes = 47;
    assert!(matches!(
        plan.clone().compile(&limits),
        Err(PlanError::TextureBudgetExceeded { actual: 48, .. })
    ));
    limits.max_texture_bytes = 48;
    let compiled = plan.clone().compile(&limits).unwrap();
    assert_eq!(compiled.passes().len(), 2);
    assert_eq!(compiled.footprint().target_texture_bytes, 48);
    assert_eq!(compiled.history_targets, std::collections::BTreeSet::from([texture(2)]));

    let RenderOperation::Shader { clock, .. } = &mut plan.passes[1].operation else {
        panic!()
    };
    *clock = pass_id(2);
    assert!(
        matches!(plan.compile(&limits), Err(PlanError::InvalidShaderHistory { pass, source })
        if pass == pass_id(1) && source == texture(2))
    );
}

#[test]
fn raw_shader_data_stays_inside_the_compiled_effect() {
    let effect = FragmentEffect::compile(
        FragmentFormat::ShaderToy,
        r#"{"renderpass":[
        {"type":"buffer","name":"Buffer A","outputs":[{"id":257}],
            "code":"void mainImage(out vec4 c,in vec2 p) { c=vec4(2.0,0.0,0.0,0.0); }"},
        {"type":"image","inputs":[{"ctype":"buffer","channel":0,"id":257}],
            "code":"void mainImage(out vec4 c,in vec2 p) { c=vec4(texture(iChannel0,vec2(0.5)).rgb,1.0); }"}
    ]}"#,
    )
    .unwrap();
    let draws = effect.stage_layout(descriptor(2, 2)).unwrap();
    let mut inputs = [None; TEXTURE_BINDINGS];
    inputs[0] = Some(ShaderTexture::Current(texture(1)));
    let mut plan = RenderPlan {
        passes: vec![
            RenderPass {
                descriptor: draws[0].target,
                operation: RenderOperation::Shader {
                    program: draws[0].program.clone(),
                    clock: pass_id(2),
                    index: 0,
                    inputs: [None; TEXTURE_BINDINGS],
                },
                ..clear(1, 1)
            },
            RenderPass {
                operation: RenderOperation::Shader {
                    program: draws[1].program.clone(),
                    clock: pass_id(2),
                    index: 1,
                    inputs,
                },
                ..clear(2, 2)
            },
        ],
        outputs: vec![output(1, 2)],
        ..Default::default()
    };
    assert_eq!(plan.clone().compile(&limits()).unwrap().passes().len(), 2);
    plan.outputs[0].source = texture(1);
    assert!(matches!(
        plan.clone().compile(&limits()),
        Err(PlanError::RawShaderOutput { .. })
    ));
    plan.outputs[0].source = texture(2);
    plan.passes[1] = sample(2, 2, 1);
    assert!(matches!(
        plan.compile(&limits()),
        Err(PlanError::InvalidShaderInput { .. })
    ));
}
