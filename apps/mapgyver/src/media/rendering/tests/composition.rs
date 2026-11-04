use super::*;
use crate::media::composition::protocol::*;
use crate::media::resolution::ChannelAvailability;
use golden_video::plan::{Point, RenderOperation};
use std::collections::HashSet;

fn composition(engine: &mut AppEngine, width: u32, height: u32) -> ChannelReference {
    let reference = create(engine, MediaKind::Composition);
    edit(
        engine,
        MediaEdit::ConfigureMediaResolution {
            media: reference.media,
            size: SizePolicy::Fixed {
                extent: Extent2D::new(width, height).unwrap(),
            },
        },
    );
    reference
}
fn add(engine: &mut AppEngine, reference: ChannelReference, sources: Vec<ChannelReference>) -> Vec<InstanceReference> {
    let MediaEditResult::InstancesCreated { instances } = edit(
        engine,
        MediaEdit::Composition {
            reference,
            edit: CompositionEdit::AddInstances { sources },
        },
    ) else {
        panic!()
    };
    instances
}
fn inspect(engine: &mut AppEngine, reference: ChannelReference) -> CompositionSnapshot {
    let MediaEditResult::Composition { snapshot } = edit(
        engine,
        MediaEdit::Composition {
            reference,
            edit: CompositionEdit::Inspect,
        },
    ) else {
        panic!()
    };
    snapshot
}
fn prepare(engine: &AppEngine, reference: ChannelReference) -> super::super::plan::Prepared {
    let owner = renderer();
    let prepared = super::super::plan::build(
        &capture_channels(engine, &[reference], 16).unwrap(),
        super::super::plan::Targets {
            routes: &routes(reference),
            preview: None,
            screens: &[],
        },
        super::super::plan::Previous {
            channels: &std::collections::HashMap::new(),
            screens: &std::collections::HashMap::new(),
        },
        &mut 3,
        config().plan_limits,
        &owner.source_observations(),
        None,
    )
    .unwrap();
    close(owner);
    prepared
}

#[test]
fn demanded_nested_channels_render_once_for_shared_instances_and_outputs() {
    let mut engine = engine();
    let leaf = create(&mut engine, MediaKind::SolidColor);
    resize(&mut engine, leaf, 4);
    let inner = composition(&mut engine, 8, 8);
    add(&mut engine, inner, vec![leaf, leaf]);
    let outer = composition(&mut engine, 8, 8);
    add(&mut engine, outer, vec![inner, leaf]);
    let capture = capture_channels(&engine, &[outer], 16).unwrap();
    assert_eq!(
        capture.channels.len(),
        7,
        "three render owners and four passthrough References"
    );
    let prepared = prepare(&engine, outer);
    assert_eq!(prepared.submission.plan.passes.len(), 3);
    assert_eq!(prepared.submission.plan.outputs.len(), 2);
    assert_eq!(
        prepared.submission.plan.outputs[0].source,
        prepared.submission.plan.outputs[1].source
    );
    let leaf_target = prepared.entries[&leaf].target;
    let inner_target = prepared.entries[&inner].target;
    let outer_pass = prepared
        .submission
        .plan
        .passes
        .iter()
        .find(|pass| pass.output == prepared.entries[&outer].target)
        .unwrap();
    let RenderOperation::Composite { layers } = &outer_pass.operation else {
        panic!()
    };
    assert_eq!(
        layers.iter().map(|layer| layer.source.texture()).collect::<Vec<_>>(),
        vec![Some(inner_target), Some(leaf_target)]
    );
    assert!(
        prepared
            .channels
            .iter()
            .all(|channel| channel.resolution.availability == ChannelAvailability::Ready)
    );
}

#[test]
fn native_scene_five_producers_share_six_aliases_and_recover_without_duplicate_passes() {
    let mut engine = engine();
    let leaves = std::array::from_fn::<_, 3, _>(|_| {
        let source = create(&mut engine, MediaKind::SolidColor);
        edit(
            &mut engine,
            MediaEdit::ConfigureMediaResolution {
                media: source.media,
                size: SizePolicy::Fixed {
                    extent: Extent2D::new(1, 1).unwrap(),
                },
            },
        );
        source
    });
    let inner = composition(&mut engine, 8, 4);
    add(&mut engine, inner, vec![leaves[0], leaves[1]]);
    let root = composition(&mut engine, 32, 24);
    add(&mut engine, root, vec![leaves[2], inner, inner, inner]);
    let capture = capture_channels(&engine, &[root], 16).unwrap();
    assert_eq!(capture.channels.len(), 11, "five producers and six virtual References");
    let assert_shared = |engine: &AppEngine| {
        let compiled = prepare(engine, root);
        assert_eq!(compiled.channels.len(), 11);
        assert_eq!(compiled.submission.plan.passes.len(), 5);
        let targets = compiled
            .channels
            .iter()
            .map(|channel| channel.target)
            .collect::<HashSet<_>>();
        assert_eq!(targets.len(), 5, "References must reuse existing producer textures");
        assert_eq!(compiled.submission.plan.outputs.len(), 2);
        assert_eq!(
            compiled.submission.plan.outputs[0].source,
            compiled.submission.plan.outputs[1].source
        );
        assert!(compiled.submission.plan.external_textures.is_empty());
        let pass_targets = compiled
            .submission
            .plan
            .passes
            .iter()
            .map(|pass| pass.output)
            .collect::<HashSet<_>>();
        assert_eq!(targets, pass_targets, "each target has exactly one producing pass");
        compiled
    };
    assert_shared(&engine);
    edit(&mut engine, MediaEdit::RemoveMedia { media: leaves[0].media });
    assert!(!capture.is_current());
    let missing = prepare(&engine, root);
    assert!(
        missing.submission.plan.passes.len() <= 5,
        "source loss cannot duplicate production"
    );
    let pass = missing
        .submission
        .plan
        .passes
        .iter()
        .find(|pass| pass.output == missing.entries[&inner].target)
        .unwrap();
    let RenderOperation::Composite { layers } = &pass.operation else {
        panic!()
    };
    assert_eq!(
        layers.len(),
        1,
        "the healthy peer producer remains in the inner Composition"
    );
    assert!(engine.undo().unwrap());
    assert_shared(&engine);
    let json = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
    let mut restored = golden_core::app::from_sparse_project_json::<crate::app::AppNode>(&json).unwrap();
    restored.apply_edits().unwrap();
    assert_shared(&restored);
}

#[test]
fn affine_reference_points_preserve_position_mirror_rotation_pivot_opacity_and_blend() {
    let mut engine = engine();
    let leaf = create(&mut engine, MediaKind::SolidColor);
    resize(&mut engine, leaf, 4); // source is 4 x 2 texels
    let composition = composition(&mut engine, 8, 8);
    let instances = add(&mut engine, composition, vec![leaf]);
    let mut definition = inspect(&mut engine, composition).instances[0].definition.clone();
    definition.transform = Transform2D {
        position: [4.0, 4.0],
        scale: [-1.0, 2.0],
        rotation_degrees: 90.0,
        pivot: [0.25, 0.5],
    };
    definition.opacity = 0.4;
    for blend in [
        BlendMode::Normal,
        BlendMode::Add,
        BlendMode::Multiply,
        BlendMode::Screen,
    ] {
        definition.blend = blend;
        edit(
            &mut engine,
            MediaEdit::Composition {
                reference: composition,
                edit: CompositionEdit::ConfigureInstance {
                    instance: instances[0].instance,
                    definition: definition.clone(),
                },
            },
        );
        let prepared = prepare(&engine, composition);
        let RenderOperation::Composite { layers } = &prepared.submission.plan.passes.last().unwrap().operation else {
            panic!()
        };
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].blend, blend);
        assert_eq!(layers[0].opacity, golden_video::Opacity::new(0.4).unwrap());
        let mapping = layers[0].mapping.unwrap();
        // Independently calculated interior points avoid f32 boundary classification at exact edges.
        for (target, expected) in [
            (Point { x: 0.5, y: 0.5 }, [0.25, 0.5]),
            (Point { x: 0.7, y: 0.575 }, [0.1, 0.1]),
            (Point { x: 0.7, y: 0.175 }, [0.9, 0.1]),
            (Point { x: 0.3, y: 0.175 }, [0.9, 0.9]),
        ] {
            let actual = mapping.source_coordinate(target).unwrap();
            assert!(
                (actual[0] - expected[0]).abs() < 1e-5 && (actual[1] - expected[1]).abs() < 1e-5,
                "{actual:?} != {expected:?}"
            );
        }
    }
}

#[test]
fn content_membership_enable_and_source_undo_invalidate_nested_captures() {
    let mut engine = engine();
    let leaf = create(&mut engine, MediaKind::SolidColor);
    let composition = composition(&mut engine, 8, 8);
    let instances = add(&mut engine, composition, vec![leaf]);
    let captured = capture_channels(&engine, &[composition], 16).unwrap();
    let mut definition = inspect(&mut engine, composition).instances[0].definition.clone();
    definition.enabled = false;
    edit(
        &mut engine,
        MediaEdit::Composition {
            reference: composition,
            edit: CompositionEdit::ConfigureInstance {
                instance: instances[0].instance,
                definition,
            },
        },
    );
    assert!(!captured.is_current());
    let disabled = capture_channels(&engine, &[composition], 16).unwrap();
    assert_eq!(
        disabled.channels.len(),
        1,
        "disabled instances must not demand their source"
    );
    assert!(engine.undo().unwrap());
    assert!(!disabled.is_current());
    let captured = capture_channels(&engine, &[composition], 16).unwrap();
    edit(&mut engine, MediaEdit::RemoveMedia { media: leaf.media });
    assert!(!captured.is_current());
    let missing = prepare(&engine, composition);
    let RenderOperation::Composite { layers } = &missing.submission.plan.passes.last().unwrap().operation else {
        panic!()
    };
    assert!(layers.is_empty());
    let captured = capture_channels(&engine, &[composition], 16).unwrap();
    assert!(engine.undo().unwrap());
    assert!(!captured.is_current());
    let restored = prepare(&engine, composition);
    let RenderOperation::Composite { layers } = &restored.submission.plan.passes.last().unwrap().operation else {
        panic!()
    };
    assert_eq!(layers.len(), 1);
}

#[test]
fn media_resolution_updates_canvas_mapping_preserves_authored_instances_and_keeps_capture_bounded() {
    let mut engine = engine();
    let leaf = create(&mut engine, MediaKind::SolidColor);
    resize(&mut engine, leaf, 4);
    let composition = composition(&mut engine, 8, 8);
    add(&mut engine, composition, vec![leaf]);
    let authored = inspect(&mut engine, composition).instances;
    let prepared = prepare(&engine, composition);
    let RenderOperation::Composite { layers: before } = &prepared.submission.plan.passes.last().unwrap().operation
    else {
        panic!()
    };
    let mapping = before[0].mapping;
    resize(&mut engine, composition, 16);
    let prepared = prepare(&engine, composition);
    let pass = prepared.submission.plan.passes.last().unwrap();
    assert_eq!(pass.descriptor.extent, Extent2D::new(16, 2).unwrap());
    let RenderOperation::Composite { layers } = &pass.operation else {
        panic!()
    };
    assert_ne!(layers[0].mapping, mapping);
    assert_eq!(
        layers[0].mapping.unwrap().inverse_rows(),
        [[4.0, 0.0, -0.5, 0.0], [0.0, 1.0, -1.5, 0.0], [0.0, 0.0, 1.0, 0.0]]
    );
    let document = inspect(&mut engine, composition);
    assert_eq!(document.canvas, Extent2D::new(16, 2).unwrap());
    assert_eq!(document.instances, authored);
    let bounded = capture_channels(&engine, &[composition], 1).unwrap();
    assert_eq!(bounded.channels.len(), 1);
    assert!(bounded.channels[0].instances.is_none());
}

#[test]
fn pending_spout_dependencies_warm_without_drawing_and_disabled_instances_withdraw_demand() {
    let mut engine = engine();
    let source = create(&mut engine, MediaKind::SharedTexture);
    let mut definition = channel(&engine, source).unwrap().definition;
    let ChannelSource::Spout { sender } = &mut definition.source else {
        panic!()
    };
    *sender = Some("CompositionTestInput".into());
    edit(
        &mut engine,
        MediaEdit::ConfigureChannel {
            reference: source,
            source: definition.source,
        },
    );
    let composition = composition(&mut engine, 8, 8);
    let instances = add(&mut engine, composition, vec![source]);
    let prepared = prepare(&engine, composition);
    assert_eq!(prepared.submission.source_demands.len(), 1);
    assert_eq!(prepared.submission.plan.external_textures.len(), 1);
    let RenderOperation::Composite { layers } = &prepared.submission.plan.passes.last().unwrap().operation else {
        panic!()
    };
    assert!(layers.is_empty());
    let mut definition = inspect(&mut engine, composition).instances[0].definition.clone();
    definition.enabled = false;
    edit(
        &mut engine,
        MediaEdit::Composition {
            reference: composition,
            edit: CompositionEdit::ConfigureInstance {
                instance: instances[0].instance,
                definition,
            },
        },
    );
    let prepared = prepare(&engine, composition);
    assert!(prepared.submission.source_demands.is_empty());
    assert!(prepared.submission.plan.external_textures.is_empty());
    assert_eq!(prepared.submission.plan.passes.len(), 1);
}
