use super::*;
use crate::shader::{FragmentProgram, TEXTURE_BINDINGS};

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
