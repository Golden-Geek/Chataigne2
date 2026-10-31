use super::contract::*;

fn stable() -> StableCounters {
    StableCounters {
        handles: 200,
        private_bytes: 100 * 1024 * 1024,
        working_set_bytes: 80 * 1024 * 1024,
        lifetime_peak_working_set_bytes: 90 * 1024 * 1024,
        reserved_texture_bytes: 1000,
        active_plan_bytes: 100,
        retiring_generations: 0,
        prepared_generations: 0,
        input_bindings: 2,
    }
}

#[test]
fn lifecycle_growth_rejects_peak_growth_even_when_final_private_bytes_return() {
    let baseline = stable();
    let verdict = evaluate(
        baseline,
        baseline,
        baseline.private_bytes + 32 * 1024 * 1024 + 1,
        100,
        2000,
        1000,
        1,
    );
    assert!(verdict.eligible);
    assert!(!verdict.observed_within_target);
}

#[test]
fn short_or_unexercised_runs_never_qualify() {
    for (cycles, spacing, edits, superseded) in [
        (12, 2000, 120, 1),
        (100, 1999, 1000, 1),
        (100, 2000, 999, 1),
        (100, 2000, 1000, 0),
    ] {
        let state = stable();
        let verdict = evaluate(state, state, state.private_bytes, cycles, spacing, edits, superseded);
        assert!(verdict.observed_within_target);
        assert!(!verdict.eligible);
    }
}

#[test]
fn retained_resources_and_handle_leaks_fail_exact_boundary_checks() {
    let base = stable();
    let mut changed = base;
    changed.handles += 11;
    assert!(!evaluate(base, changed, base.private_bytes, 100, 2000, 1000, 1).observed_within_target);
    changed = base;
    changed.retiring_generations = 1;
    assert!(!evaluate(base, changed, base.private_bytes, 100, 2000, 1000, 1).logical_live_set_restored);
    changed = base;
    changed.reserved_texture_bytes += 1;
    assert!(!evaluate(base, changed, base.private_bytes, 100, 2000, 1000, 1).logical_live_set_restored);
    assert!(evaluate(base, base, base.private_bytes, 100, 2000, 1000, 1).observed_within_target);
}

#[test]
fn lifecycle_scene_keeps_three_outputs_and_resizes_actual_source_contract() {
    let names = ["a".into(), "b".into()];
    let extent = golden_video::Extent2D::new(1280, 720).unwrap();
    let plan = super::owner::plan(&names, extent, 1).unwrap();
    let unchanged = super::owner::plan(&names, extent, 0).unwrap();
    let width = |pass: &golden_video::plan::RenderPass| match &pass.operation {
        golden_video::plan::RenderOperation::Regions { regions, .. } => regions[0].destination.width,
        _ => panic!("lifecycle viewport must use regions"),
    };
    assert_eq!(width(&plan.plan.passes[0]), width(&unchanged.plan.passes[0]));
    assert_ne!(width(&plan.plan.passes[1]), width(&unchanged.plan.passes[1]));
    assert_eq!(plan.plan.external_textures[0].descriptor.extent, extent);
    assert_eq!(plan.plan.outputs.len(), 3);
    assert!(plan.plan.passes.iter().all(|pass| matches!(
        &pass.operation,
        golden_video::plan::RenderOperation::Regions { visibility: None, .. }
    )));
    assert!(
        plan.plan
            .compile(&super::super::scene::config(512 * 1024 * 1024, golden_io::WorkerNotifier::new(|| {})).plan_limits)
            .is_ok()
    );
}
