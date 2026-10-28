use super::*;

#[test]
fn pass_writer_and_output_identities_are_unique() {
    let mut duplicate_pass = diamond();
    duplicate_pass.passes.push(clear(20, 20));
    assert!(matches!(duplicate_pass.compile(&limits()), Err(PlanError::DuplicatePass(id)) if id == pass_id(20)));

    for candidate in [
        RenderPlan {
            external_textures: vec![external(1), external(1)],
            ..Default::default()
        },
        RenderPlan {
            passes: vec![clear(1, 1), clear(2, 1)],
            ..Default::default()
        },
        RenderPlan {
            external_textures: vec![external(1)],
            passes: vec![clear(1, 1)],
            ..Default::default()
        },
    ] {
        assert!(matches!(candidate.compile(&limits()), Err(PlanError::DuplicateTextureWriter(id)) if id == texture(1)));
    }

    let mut duplicate_output = diamond();
    duplicate_output.outputs.push(output(1, 5));
    assert!(
        matches!(duplicate_output.compile(&limits()), Err(PlanError::DuplicateOutput(id)) if id == OutputId::new(1).unwrap())
    );
}

#[test]
fn missing_pass_inputs_and_output_sources_reject_instead_of_disappearing_during_pruning() {
    let candidate = RenderPlan {
        passes: vec![clear(1, 1), sample(2, 2, 999)],
        outputs: vec![output(1, 1)],
        ..Default::default()
    };
    assert!(
        matches!(candidate.compile(&limits()), Err(PlanError::MissingPassInput { pass, source })
        if pass == pass_id(2) && source == texture(999))
    );

    let candidate = RenderPlan {
        outputs: vec![output(1, 999)],
        ..Default::default()
    };
    assert!(
        matches!(candidate.compile(&limits()), Err(PlanError::MissingOutputSource { output, source })
        if output == OutputId::new(1).unwrap() && source == texture(999))
    );
}

#[test]
fn raw_counts_are_bounded_before_compiler_indexes_or_pruning() {
    let cases = [
        (
            PlanLimits {
                max_passes: 3,
                ..limits()
            },
            GraphLimit::Passes,
            4,
            3,
        ),
        (
            PlanLimits {
                max_external_textures: 0,
                ..limits()
            },
            GraphLimit::ExternalTextures,
            1,
            0,
        ),
        (
            PlanLimits {
                max_outputs: 1,
                ..limits()
            },
            GraphLimit::Outputs,
            2,
            1,
        ),
        (
            PlanLimits {
                max_dependencies: 6,
                ..limits()
            },
            GraphLimit::Dependencies,
            7,
            6,
        ),
    ];
    for (limits, expected_kind, expected_actual, expected_limit) in cases {
        let error = diamond().compile(&limits).unwrap_err();
        assert!(matches!(error, PlanError::GraphLimitExceeded { kind, actual, limit }
            if kind == expected_kind && actual == expected_actual && limit == expected_limit));
    }

    let candidate = RenderPlan {
        passes: vec![RenderPass {
            operation: RenderOperation::Composite {
                layers: vec![layer(999), layer(999)],
            },
            ..clear(1, 1)
        }],
        ..Default::default()
    };
    assert!(matches!(
        candidate.compile(&PlanLimits {
            max_dependencies: 1,
            ..limits()
        }),
        Err(PlanError::GraphLimitExceeded {
            kind: GraphLimit::Dependencies,
            actual: 2,
            limit: 1
        })
    ));
}

#[test]
fn inactive_dimensions_and_internal_color_alpha_contracts_are_validated() {
    let candidate = RenderPlan {
        external_textures: vec![ExternalTexture {
            descriptor: descriptor(65, 1),
            ..external(1)
        }],
        ..Default::default()
    };
    assert!(
        matches!(candidate.compile(&PlanLimits { max_texture_dimension: 64, ..limits() }),
        Err(PlanError::TextureDimensionExceeded { texture: id, width: 65, height: 1, limit: 64 }) if id == texture(1))
    );

    for (encoding, alpha) in [
        (ColorEncoding::Srgb, AlphaMode::Premultiplied),
        (ColorEncoding::Linear, AlphaMode::Straight),
        (ColorEncoding::Linear, AlphaMode::Opaque),
    ] {
        let mut pass = clear(1, 1);
        pass.descriptor.color_encoding = encoding;
        pass.descriptor.alpha_mode = alpha;
        assert!(
            matches!(RenderPlan { passes: vec![pass], ..Default::default() }.compile(&limits()),
            Err(PlanError::InvalidTargetContract { pass, output }) if pass == pass_id(1) && output == texture(1))
        );
    }
}

#[test]
fn descriptor_and_summed_footprint_overflow_never_wrap_into_admission() {
    let generous = PlanLimits {
        max_texture_dimension: u32::MAX,
        max_texture_bytes: u64::MAX,
        ..limits()
    };
    let invalid = RenderPlan {
        external_textures: vec![ExternalTexture {
            id: texture(1),
            descriptor: descriptor(u32::MAX, u32::MAX),
        }],
        ..Default::default()
    };
    assert!(
        matches!(invalid.compile(&generous), Err(PlanError::InvalidTexture { texture: id, .. }) if id == texture(1))
    );

    let half_range = descriptor(1 << 31, 1 << 30);
    assert_eq!(half_range.byte_len().unwrap(), 1 << 63);
    for candidate in [
        RenderPlan {
            external_textures: vec![
                ExternalTexture {
                    id: texture(1),
                    descriptor: half_range,
                },
                ExternalTexture {
                    id: texture(2),
                    descriptor: half_range,
                },
            ],
            outputs: vec![output(1, 1), output(2, 2)],
            ..Default::default()
        },
        RenderPlan {
            external_textures: vec![ExternalTexture {
                id: texture(1),
                descriptor: half_range,
            }],
            passes: vec![RenderPass {
                descriptor: half_range,
                ..sample(1, 2, 1)
            }],
            outputs: vec![output(1, 2)],
        },
    ] {
        assert!(matches!(
            candidate.compile(&generous),
            Err(PlanError::TextureFootprintOverflow)
        ));
    }
}

#[test]
fn exact_demanded_byte_budget_is_inclusive_and_does_not_double_charge_shared_inputs() {
    assert!(
        diamond()
            .compile(&PlanLimits {
                max_texture_bytes: 80,
                ..limits()
            })
            .is_ok()
    );
    assert!(matches!(
        diamond().compile(&PlanLimits {
            max_texture_bytes: 79,
            ..limits()
        }),
        Err(PlanError::TextureBudgetExceeded { actual: 80, limit: 79 })
    ));
}
