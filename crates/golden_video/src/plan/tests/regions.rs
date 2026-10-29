use super::*;

fn region(source: u64) -> BlitRegion {
    BlitRegion {
        source: texture(source),
        destination: UnitRect::FULL,
        source_rect: UnitRect::FULL,
        filter: SampleFilter::Nearest,
    }
}

fn scene(regions: Vec<BlitRegion>) -> RenderPlan {
    RenderPlan {
        external_textures: vec![external(1), external(2)],
        passes: vec![RenderPass {
            operation: RenderOperation::Regions {
                regions,
                visibility: None,
            },
            ..clear(1, 3)
        }],
        outputs: vec![output(1, 3)],
    }
}

#[test]
fn region_demands_share_sources_preserve_painter_order_and_compact_retained_capacity() {
    let mut regions = Vec::with_capacity(1024);
    let cropped = BlitRegion {
        source_rect: UnitRect {
            x: 0.5,
            y: 0.25,
            width: 0.5,
            height: 0.5,
        },
        destination: UnitRect {
            x: 0.25,
            y: 0.25,
            width: 0.5,
            height: 0.5,
        },
        ..region(2)
    };
    regions.extend([cropped.clone(), region(1), region(2)]);
    let input = scene(regions);
    assert!(input.retained_bytes().unwrap() >= 1024 * size_of::<BlitRegion>());
    let compiled = input.compile(&limits()).unwrap();
    assert_eq!(compiled.footprint().external_texture_bytes, 32);
    assert_eq!(compiled.footprint().target_texture_bytes, 16);
    let RenderOperation::Regions { regions, .. } = &compiled.passes()[0].operation else {
        panic!()
    };
    assert_eq!(
        regions.iter().map(|region| region.source).collect::<Vec<_>>(),
        [texture(2), texture(1), texture(2)]
    );
    assert_eq!(regions[0].source_rect, cropped.source_rect);
    assert_eq!(regions[0].destination, cropped.destination);
    assert_eq!(regions.capacity(), regions.len());
}

#[test]
fn invalid_region_rectangles_reject_both_source_and_destination_in_inactive_passes() {
    let invalid = [
        UnitRect {
            x: f32::NAN,
            ..UnitRect::FULL
        },
        UnitRect {
            y: f32::INFINITY,
            ..UnitRect::FULL
        },
        UnitRect {
            x: -0.01,
            ..UnitRect::FULL
        },
        UnitRect {
            y: -0.01,
            ..UnitRect::FULL
        },
        UnitRect {
            width: 0.0,
            ..UnitRect::FULL
        },
        UnitRect {
            height: -0.1,
            ..UnitRect::FULL
        },
        UnitRect {
            width: f32::INFINITY,
            ..UnitRect::FULL
        },
        UnitRect {
            x: 0.5,
            width: 0.6,
            ..UnitRect::FULL
        },
        UnitRect {
            y: 0.5,
            height: 0.6,
            ..UnitRect::FULL
        },
    ];
    assert!(UnitRect::default().is_valid());
    for rectangle in invalid {
        for source in [false, true] {
            let mut entry = region(1);
            if source {
                entry.source_rect = rectangle;
            } else {
                entry.destination = rectangle;
            }
            let mut plan = scene(vec![entry]);
            plan.outputs.clear();
            assert!(
                matches!(plan.compile(&limits()), Err(PlanError::InvalidRegion { pass, region: 0 }) if pass == pass_id(1))
            );
        }
    }
}

#[test]
fn region_raw_dependencies_are_bounded_before_validation_and_missing_inputs_are_explicit() {
    let mut bounded = limits();
    bounded.max_dependencies = 2;
    let mut entry = region(1);
    entry.destination.width = f32::NAN;
    assert!(matches!(
        scene(vec![entry; 3]).compile(&bounded),
        Err(PlanError::GraphLimitExceeded {
            kind: GraphLimit::Dependencies,
            actual: 3,
            limit: 2
        })
    ));
    assert!(matches!(scene(vec![region(99)]).compile(&limits()),
        Err(PlanError::MissingPassInput {pass,source}) if pass==pass_id(1) && source==texture(99)));
}

#[test]
fn region_dependencies_join_cycle_detection_and_stable_topological_order() {
    let mut plan = scene(vec![region(4)]);
    plan.passes.insert(0, sample(2, 4, 1));
    let compiled = plan.clone().compile(&limits()).unwrap();
    assert_eq!(pass_order(&compiled), [2, 1]);
    plan.passes[0].operation = RenderOperation::Regions {
        regions: vec![region(3)],
        visibility: None,
    };
    assert!(
        matches!(plan.compile(&limits()),Err(PlanError::Cycle {blocked_passes}) if blocked_passes==[pass_id(1),pass_id(2)])
    );
}

#[test]
fn an_empty_region_pass_demands_no_sources_but_retains_its_cleared_target() {
    let compiled = scene(Vec::new()).compile(&limits()).unwrap();
    assert!(compiled.external_textures().is_empty());
    assert_eq!(compiled.passes().len(), 1);
    assert_eq!(compiled.footprint().total_texture_bytes, 16);
    let RenderOperation::Regions { regions, .. } = &compiled.passes()[0].operation else {
        panic!()
    };
    assert!(regions.is_empty());
}

#[test]
fn presentation_visibility_fences_retain_their_clock_but_do_not_change_graph_demand_or_admission() {
    let source = golden_io::RevisionSource::default();
    let guard = source.reader().guard_at(0);
    let mut input = scene(vec![region(1)]);
    let without = input.retained_bytes().unwrap();
    let RenderOperation::Regions { visibility, .. } = &mut input.passes[0].operation else {
        panic!()
    };
    *visibility = Some(guard.clone());
    assert_eq!(input.retained_bytes().unwrap() - without, guard.retained_heap_bytes());
    source.advance();
    let compiled = input.compile(&limits()).unwrap();
    assert!(
        compiled.admission_is_current(),
        "visibility is independent of source/plan admission"
    );
    assert_eq!(compiled.external_textures().len(), 1);
    assert_eq!(compiled.footprint().total_texture_bytes, 32);
    let RenderOperation::Regions {
        visibility: Some(visibility),
        ..
    } = &compiled.passes()[0].operation
    else {
        panic!()
    };
    assert!(!visibility.is_current());
}
