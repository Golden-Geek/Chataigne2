use super::*;

#[test]
fn dependency_order_and_ready_ties_are_stable_under_submission_permutations() {
    let original = diamond();
    for rotation in 0..original.passes.len() {
        for reverse in [false, true] {
            let mut candidate = original.clone();
            candidate.passes.rotate_left(rotation);
            if reverse {
                candidate.passes.reverse();
                candidate.outputs.reverse();
            }
            let compiled = candidate.compile(&limits()).unwrap();
            assert_eq!(pass_order(&compiled), [20, 5, 90, 1]);
            assert_eq!(
                compiled
                    .outputs()
                    .iter()
                    .map(|output| output.id.get())
                    .collect::<Vec<_>>(),
                [1, 2]
            );
            assert_eq!(compiled.external_textures().len(), 1);
            assert_eq!(
                compiled.footprint(),
                TextureFootprint {
                    external_texture_bytes: 16,
                    target_texture_bytes: 64,
                    total_texture_bytes: 80,
                }
            );
        }
    }
}

#[test]
fn undemanded_metadata_is_pruned_and_shared_output_demand_does_not_duplicate_resources() {
    let mut candidate = diamond();
    candidate.external_textures.push(ExternalTexture {
        descriptor: descriptor(100, 100),
        ..external(99)
    });
    candidate.passes.push(RenderPass {
        descriptor: descriptor(100, 100),
        ..clear(99, 100)
    });
    candidate.outputs.push(output(3, 5));
    let limits = PlanLimits {
        max_texture_bytes: 80,
        ..limits()
    };
    let compiled = candidate.compile(&limits).unwrap();
    assert_eq!(pass_order(&compiled), [20, 5, 90, 1]);
    assert_eq!(
        compiled
            .external_textures()
            .iter()
            .map(|texture| texture.id.get())
            .collect::<Vec<_>>(),
        [1]
    );
    assert_eq!(compiled.outputs().len(), 3);
    assert_eq!(compiled.footprint().total_texture_bytes, 80);
}

#[test]
fn an_empty_demand_set_retains_no_executable_resources() {
    let mut candidate = diamond();
    candidate.outputs.clear();
    let compiled = candidate
        .compile(&PlanLimits {
            max_texture_bytes: 0,
            ..limits()
        })
        .unwrap();
    assert!(compiled.passes().is_empty());
    assert!(compiled.external_textures().is_empty());
    assert!(compiled.outputs().is_empty());
    assert_eq!(compiled.footprint(), TextureFootprint::default());
}

#[test]
fn direct_external_outputs_preserve_the_declared_source_contract() {
    let source = TextureDescriptor {
        format: PixelFormat::Rgba8Unorm,
        color_encoding: ColorEncoding::Srgb,
        alpha_mode: AlphaMode::Straight,
        ..descriptor(2, 2)
    };
    let compiled = RenderPlan {
        external_textures: vec![ExternalTexture {
            id: texture(1),
            descriptor: source,
            source: None,
        }],
        outputs: vec![output(1, 1), output(2, 1)],
        ..Default::default()
    }
    .compile(&limits())
    .unwrap();
    assert_eq!(compiled.external_textures()[0].descriptor, source);
    assert!(compiled.passes().is_empty());
    assert_eq!(compiled.footprint().external_texture_bytes, 16);
    assert_eq!(compiled.footprint().target_texture_bytes, 0);
}

#[test]
fn direct_and_indirect_cycles_reject_even_without_demand() {
    for passes in [vec![sample(1, 1, 1)], vec![sample(1, 1, 2), sample(2, 2, 1)]] {
        let error = RenderPlan {
            passes,
            ..Default::default()
        }
        .compile(&limits())
        .unwrap_err();
        assert!(matches!(error, PlanError::Cycle { .. }));
    }
}

#[test]
fn cycle_diagnostics_name_blocked_dependents_without_claiming_exact_cycle_membership() {
    let error = RenderPlan {
        passes: vec![sample(8, 8, 2), sample(2, 2, 1), clear(9, 9), sample(1, 1, 2)],
        outputs: vec![output(1, 9)],
        ..Default::default()
    }
    .compile(&limits())
    .unwrap_err();
    let PlanError::Cycle { blocked_passes } = error else {
        panic!("expected cycle rejection")
    };
    assert_eq!(blocked_passes, [pass_id(1), pass_id(2), pass_id(8)]);
}

#[test]
fn resize_changes_footprint_without_replacing_logical_identities() {
    let original = diamond();
    let mut resized = original.clone();
    for source in &mut resized.external_textures {
        source.descriptor.extent = Extent2D::new(4, 4).unwrap();
    }
    for pass in &mut resized.passes {
        pass.descriptor.extent = Extent2D::new(4, 4).unwrap();
    }
    let first = original.compile(&limits()).unwrap();
    let second = resized.compile(&limits()).unwrap();
    assert_eq!(pass_order(&first), pass_order(&second));
    assert_eq!(
        first
            .outputs()
            .iter()
            .map(|output| (output.id, output.source))
            .collect::<Vec<_>>(),
        second
            .outputs()
            .iter()
            .map(|output| (output.id, output.source))
            .collect::<Vec<_>>()
    );
    assert_eq!(first.footprint().total_texture_bytes, 80);
    assert_eq!(second.footprint().total_texture_bytes, 320);
}

#[test]
fn rejected_preparation_cannot_mutate_an_existing_immutable_plan() {
    let live = diamond().compile(&limits()).unwrap();
    let prior_order = pass_order(&live);
    let prior_footprint = live.footprint();
    let mut rejected = diamond();
    rejected.passes[0].operation = RenderOperation::Sample {
        source: texture(999),
        filter: SampleFilter::Nearest,
    };
    assert!(rejected.compile(&limits()).is_err());
    assert_eq!(pass_order(&live), prior_order);
    assert_eq!(live.footprint(), prior_footprint);
}

#[test]
fn a_ten_thousand_pass_chain_compiles_iteratively() {
    const COUNT: u64 = 10_000;
    let one_pixel = descriptor(1, 1);
    let mut passes = (0..COUNT)
        .map(|index| RenderPass {
            descriptor: one_pixel,
            ..sample(COUNT - index, index + 2, index + 1)
        })
        .collect::<Vec<_>>();
    passes.reverse();
    let compiled = RenderPlan {
        external_textures: vec![ExternalTexture {
            id: texture(1),
            descriptor: one_pixel,
            source: None,
        }],
        passes,
        outputs: vec![output(1, COUNT + 1)],
    }
    .compile(&PlanLimits {
        max_passes: COUNT as usize,
        max_dependencies: COUNT as usize,
        max_texture_bytes: (COUNT + 1) * 4,
        ..limits()
    })
    .unwrap();
    assert_eq!(compiled.passes().len(), COUNT as usize);
    assert_eq!(compiled.passes().first().unwrap().id.get(), COUNT);
    assert_eq!(compiled.passes().last().unwrap().id.get(), 1);
    assert_eq!(compiled.footprint().total_texture_bytes, (COUNT + 1) * 4);
}
