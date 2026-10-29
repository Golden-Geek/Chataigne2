use super::*;

#[test]
fn source_only_demand_is_explicit_bounded_and_does_not_require_a_presentation() {
    let plan = RenderPlan {
        external_textures: vec![external(1)],
        ..Default::default()
    };
    assert!(plan.clone().compile(&limits()).unwrap().external_textures().is_empty());
    let warmed = plan
        .clone()
        .compile_with_source_demands(&limits(), &[texture(1)])
        .unwrap();
    assert_eq!(warmed.external_textures().len(), 1);
    assert!(warmed.outputs().is_empty());
    assert!(matches!(
        plan.clone().compile_with_source_demands(&limits(), &[texture(2)]),
        Err(PlanError::MissingSourceDemand(_))
    ));
    assert!(
        plan.compile_with_source_demands(
            &PlanLimits {
                max_external_textures: 1,
                ..limits()
            },
            &[texture(1), texture(1)]
        )
        .is_err()
    );
}

#[test]
fn source_contracts_are_validated_before_pruning_and_charge_retained_selector_capacity() {
    use crate::plan::SpoutSource;
    use std::time::Duration;
    let valid = SpoutSource {
        sender: "Source".into(),
        descriptor: descriptor(4, 4),
        poll_interval: Duration::from_millis(16),
    };
    let make = |source: SpoutSource| RenderPlan {
        external_textures: vec![ExternalTexture {
            source: Some(source.into()),
            ..external(1)
        }],
        ..Default::default()
    };
    assert!(make(valid.clone()).compile(&limits()).is_ok());
    let mut invalid = Vec::new();
    for sender in [String::new(), "x".repeat(256), "bad\0source".into()] {
        invalid.push(SpoutSource {
            sender,
            ..valid.clone()
        });
    }
    invalid.push(SpoutSource {
        poll_interval: Duration::ZERO,
        ..valid.clone()
    });
    invalid.push(SpoutSource {
        poll_interval: Duration::from_secs(2),
        ..valid.clone()
    });
    invalid.push(SpoutSource {
        descriptor: TextureDescriptor {
            alpha_mode: AlphaMode::Straight,
            ..valid.descriptor
        },
        ..valid.clone()
    });
    invalid.push(SpoutSource {
        descriptor: descriptor(1, 1),
        ..valid.clone()
    });
    for source in invalid {
        assert!(matches!(
            make(source).compile(&limits()),
            Err(PlanError::InvalidSourceBinding(_))
        ));
    }
    let compact = make(valid.clone()).retained_bytes().unwrap();
    let mut retained = String::with_capacity(65536);
    retained.push_str("Source");
    assert!(
        make(SpoutSource {
            sender: retained,
            ..valid
        })
        .retained_bytes()
        .unwrap()
            >= compact + 65000
    );
}

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
            source: None,
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
                    source: None,
                },
                ExternalTexture {
                    id: texture(2),
                    descriptor: half_range,
                    source: None,
                },
            ],
            outputs: vec![output(1, 1), output(2, 2)],
            ..Default::default()
        },
        RenderPlan {
            external_textures: vec![ExternalTexture {
                id: texture(1),
                descriptor: half_range,
                source: None,
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
