use super::*;
use crate::media::spatial::protocol::SpatialResizeHandle;
use golden_core::{application::apply_project_intent, node::Node, ui_sync::UiEditIntent};

fn instance_fixture() -> (AppEngine, ChannelReference, InstanceSnapshot) {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let composition = create(&mut engine, MediaKind::Composition);
    add(&mut engine, composition, vec![source]);
    let mut instance = inspect(&mut engine, composition).instances.remove(0);
    instance.definition.transform = Transform2D {
        position: [100.0, 100.0],
        scale: [-2.0, 3.0],
        rotation_degrees: 0.0,
        pivot: [0.2, 0.8],
    };
    instance.definition.opacity = 0.4;
    instance.definition.blend = BlendMode::Multiply;
    edit(
        &mut engine,
        composition,
        CompositionEdit::ConfigureInstance {
            instance: instance.reference.instance,
            definition: instance.definition.clone(),
        },
    );
    (engine, composition, instance)
}
fn request(
    instance: &InstanceSnapshot,
    mode: InstanceGeometryMode,
    start: [f32; 2],
    point: [f32; 2],
) -> CompositionEdit {
    CompositionEdit::EditInstanceGeometry {
        instance: instance.reference.instance,
        edit: InstanceGeometryEdit {
            mode,
            origin: instance.definition.transform,
            start: Point {
                x: start[0],
                y: start[1],
            },
            point: Point {
                x: point[0],
                y: point[1],
            },
        },
    }
}
fn group(engine: &mut AppEngine, begin: bool) {
    let intent = if begin {
        UiEditIntent::BeginEdit {
            client_edit_id: "instance-drag".into(),
            label: Some("Move instance".into()),
        }
    } else {
        UiEditIntent::EndEdit {
            client_edit_id: "instance-drag".into(),
        }
    };
    let ack = apply_project_intent(engine, intent);
    assert!(ack.success, "{ack:?}");
}

#[test]
fn geometry_publishes_parameters_without_catalog_invalidation() {
    use golden_core::events::EventKind;
    let (mut engine, composition, instance) = instance_fixture();
    engine.clear_ui_event_log();
    edit(
        &mut engine,
        composition,
        request(&instance, InstanceGeometryMode::Translate, [0.0, 0.0], [20.0, 30.0]),
    );
    let events = engine.ui_events_since(None);
    assert!(
        events
            .iter()
            .any(|event| matches!(event.kind, EventKind::ParamChanged { .. }))
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(&event.kind, EventKind::Custom(custom) if custom.topic == MEDIA_NAMESPACE))
    );
    engine.clear_ui_event_log();
    crate::media::tests::apply(
        &mut engine,
        MediaEdit::RenameMedia {
            media: composition.media,
            label: "Renamed Composition".into(),
        },
    );
    assert!(
        engine
            .ui_events_since(None)
            .iter()
            .any(|event| matches!(&event.kind, EventKind::Custom(custom) if custom.topic == MEDIA_NAMESPACE))
    );
}

#[test]
fn translation_preserves_unrelated_authored_f64_values_exactly() {
    use golden_core::parameter::{ParamValue, ParameterEventBehaviour};
    let (mut engine, composition, instance) = instance_fixture();
    let media = engine.node_id_by_uuid(instance.reference.instance.0).unwrap();
    let values = [
        ("opacity", ParamValue::Float(0.123456789012345)),
        ("rotation_degrees", ParamValue::Float(13.123456789012345)),
        ("scale", ParamValue::Vec2(-2.123456789012345, 3.123456789012345)),
        ("pivot", ParamValue::Vec2(0.123456789012345, 0.823456789012345)),
    ]
    .map(|(key, value)| (super::parameters::instance_parameter(&engine, media, key), value));
    for (node, value) in &values {
        let ack = apply_project_intent(
            &mut engine,
            UiEditIntent::SetParam {
                node: *node,
                value: value.clone(),
                behaviour: ParameterEventBehaviour::Append,
            },
        );
        assert!(ack.success, "{ack:?}");
    }
    let instance = inspect(&mut engine, composition).instances.remove(0);
    let before = engine.undo_len();
    group(&mut engine, true);
    for step in 1..=30 {
        edit(
            &mut engine,
            composition,
            request(
                &instance,
                InstanceGeometryMode::Translate,
                [0.0, 0.0],
                [step as f32, 0.0],
            ),
        );
        for (node, value) in &values {
            assert_eq!(
                engine.nodes.get(*node).unwrap().engine_param_snapshot().unwrap().value,
                *value
            );
        }
    }
    group(&mut engine, false);
    assert_eq!(engine.undo_len(), before + 1);
    assert!(engine.undo().unwrap());
    for (node, value) in &values {
        assert_eq!(
            engine.nodes.get(*node).unwrap().engine_param_snapshot().unwrap().value,
            *value
        );
    }
}

#[test]
fn absolute_translation_samples_preserve_other_fields_and_group_history() {
    let (mut engine, composition, instance) = instance_fixture();
    let before = engine.undo_len();
    group(&mut engine, true);
    for step in 1..=100 {
        let MediaEditResult::InstanceChanged { snapshot } = edit(
            &mut engine,
            composition,
            request(
                &instance,
                InstanceGeometryMode::Translate,
                [90.0, 80.0],
                [90.0 + step as f32, 60.0],
            ),
        ) else {
            panic!()
        };
        let mut expected = instance.clone();
        expected.definition.transform.position = [100.0 + step as f32, 80.0];
        assert_eq!(snapshot, expected);
    }
    group(&mut engine, false);
    assert_eq!(engine.undo_len(), before + 1);
    let final_state = inspect(&mut engine, composition);
    assert!(engine.undo().unwrap());
    assert_eq!(inspect(&mut engine, composition).instances[0], instance);
    assert!(engine.redo().unwrap());
    assert_eq!(inspect(&mut engine, composition), final_state);
}

#[test]
fn rotation_uses_canvas_pivot_and_preserves_other_fields() {
    let (mut engine, composition, instance) = instance_fixture();
    edit(
        &mut engine,
        composition,
        request(&instance, InstanceGeometryMode::Rotate, [110.0, 100.0], [100.0, 110.0]),
    );
    let rotated = inspect(&mut engine, composition).instances.remove(0);
    let mut expected = instance.definition.clone();
    expected.transform.rotation_degrees = 90.0;
    assert_eq!(rotated.definition, expected);
}

fn resize(handle: SpatialResizeHandle) -> InstanceGeometryMode {
    InstanceGeometryMode::Resize {
        handle,
        extent: golden_video::Extent2D::new(1920, 1080).unwrap(),
    }
}

fn projected(transform: Transform2D, fraction: [f64; 2]) -> [f64; 2] {
    let local = [
        (fraction[0] - f64::from(transform.pivot[0])) * 1920. * f64::from(transform.scale[0]),
        (fraction[1] - f64::from(transform.pivot[1])) * 1080. * f64::from(transform.scale[1]),
    ];
    let (sin, cos) = f64::from(transform.rotation_degrees).to_radians().sin_cos();
    [
        f64::from(transform.position[0]) + cos * local[0] - sin * local[1],
        f64::from(transform.position[1]) + sin * local[0] + cos * local[1],
    ]
}

#[test]
fn all_resize_handles_fix_opposite_anchor_with_rotated_mirrored_nonuniform_geometry() {
    for (handle, opposite, axes) in [
        (SpatialResizeHandle::TopLeft, [1., 1.], [true, true]),
        (SpatialResizeHandle::Top, [0.5, 1.], [false, true]),
        (SpatialResizeHandle::TopRight, [0., 1.], [true, true]),
        (SpatialResizeHandle::Right, [0., 0.5], [true, false]),
        (SpatialResizeHandle::BottomRight, [0., 0.], [true, true]),
        (SpatialResizeHandle::Bottom, [0.5, 0.], [false, true]),
        (SpatialResizeHandle::BottomLeft, [1., 0.], [true, true]),
        (SpatialResizeHandle::Left, [1., 0.5], [true, false]),
    ] {
        for angle in [0., 37., 90.] {
            let (mut engine, composition, mut instance) = instance_fixture();
            instance.definition.transform.rotation_degrees = angle;
            instance.definition.transform.pivot = [-0.3, 1.2];
            edit(
                &mut engine,
                composition,
                CompositionEdit::ConfigureInstance {
                    instance: instance.reference.instance,
                    definition: instance.definition.clone(),
                },
            );
            let anchor = projected(instance.definition.transform, opposite);
            let history = engine.undo_len();
            group(&mut engine, true);
            // Deliberately start away from the exact handle center. Deltas, not hit offsets,
            // determine size; each admitted sample is relative to the immutable origin.
            for step in 0..=12 {
                edit(
                    &mut engine,
                    composition,
                    request(
                        &instance,
                        resize(handle),
                        [143., 81.],
                        [143. + step as f32 * 2., 81. + step as f32],
                    ),
                );
            }
            group(&mut engine, false);
            let after = inspect(&mut engine, composition).instances.remove(0);
            let actual = projected(after.definition.transform, opposite);
            for axis in 0..2 {
                assert!(
                    (anchor[axis] - actual[axis]).abs() < 0.001,
                    "{handle:?} {angle}: {anchor:?} {actual:?}"
                );
                if !axes[axis] {
                    assert_eq!(
                        after.definition.transform.scale[axis],
                        instance.definition.transform.scale[axis]
                    );
                }
            }
            assert_eq!(after.definition.opacity, instance.definition.opacity);
            assert_eq!(after.definition.blend, instance.definition.blend);
            assert_eq!(engine.undo_len(), history + 1);
            assert!(engine.undo().unwrap());
            assert_eq!(inspect(&mut engine, composition).instances[0], instance);
            assert!(engine.redo().unwrap());
            assert_eq!(inspect(&mut engine, composition).instances[0], after);
        }
    }
}

#[test]
fn rejected_degenerate_nonfinite_and_wrong_owner_geometry_is_atomic() {
    let (mut engine, composition, instance) = instance_fixture();
    let foreign = create(&mut engine, MediaKind::Composition);
    let before = inspect(&mut engine, composition);
    let history = engine.undo_len();
    for (reference, operation) in [
        (
            composition,
            request(&instance, InstanceGeometryMode::Rotate, [100.0; 2], [110.0; 2]),
        ),
        (
            composition,
            request(
                &instance,
                InstanceGeometryMode::Resize {
                    handle: SpatialResizeHandle::Right,
                    extent: golden_video::Extent2D::new(20, 16).unwrap(),
                },
                [0.; 2],
                [10.; 2],
            ),
        ),
        (
            composition,
            request(&instance, resize(SpatialResizeHandle::Right), [0.; 2], [3840., 0.]),
        ),
        (
            composition,
            request(
                &instance,
                InstanceGeometryMode::Translate,
                [0.0; 2],
                [f32::INFINITY, 0.0],
            ),
        ),
        (
            foreign,
            request(&instance, InstanceGeometryMode::Translate, [0.0; 2], [10.0; 2]),
        ),
    ] {
        assert!(
            !invoke(
                &mut engine,
                MediaEdit::Composition {
                    reference,
                    edit: operation
                }
            )
            .success
        );
        assert_eq!(inspect(&mut engine, composition), before);
        assert_eq!(engine.undo_len(), history);
    }
}

#[test]
fn geometry_preserves_current_unrelated_fields_and_unchanged_samples_have_no_history() {
    let (mut engine, composition, instance) = instance_fixture();
    let mut current = instance.definition.clone();
    current.opacity = 0.7;
    current.transform.pivot = [-0.1, 1.5];
    current.transform.scale = [4.0, -5.0];
    current.enabled = false;
    edit(
        &mut engine,
        composition,
        CompositionEdit::ConfigureInstance {
            instance: instance.reference.instance,
            definition: current.clone(),
        },
    );
    let history = engine.undo_len();
    edit(
        &mut engine,
        composition,
        request(&instance, InstanceGeometryMode::Translate, [0.0; 2], [0.0; 2]),
    );
    assert_eq!(engine.undo_len(), history);
    for x in [10.0, 20.0] {
        edit(
            &mut engine,
            composition,
            request(&instance, InstanceGeometryMode::Translate, [0.0; 2], [x, 0.0]),
        );
        current.transform.position[0] = 100.0 + x;
        assert_eq!(inspect(&mut engine, composition).instances[0].definition, current);
    }
    assert_eq!(engine.undo_len(), history + 2);
}
