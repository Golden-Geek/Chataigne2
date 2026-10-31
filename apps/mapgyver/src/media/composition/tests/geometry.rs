use super::*;
use golden_core::{application::apply_project_intent, ui_sync::UiEditIntent};

fn instance_fixture() -> (AppEngine, ChannelReference, InstanceSnapshot) {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let composition = create(&mut engine, MediaKind::Composition);
    add(&mut engine, composition, vec![source]);
    let mut instance = inspect(&mut engine, composition).instances.remove(0);
    instance.definition.transform = Transform2D {
        position: [100.0, 100.0],
        scale: [-2.0, 3.0],
        rotation: 0.0,
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
fn rotation_and_mirrored_nonuniform_scale_use_canvas_pivot_and_local_axes() {
    let (mut engine, composition, instance) = instance_fixture();
    edit(
        &mut engine,
        composition,
        request(&instance, InstanceGeometryMode::Rotate, [110.0, 100.0], [100.0, 110.0]),
    );
    let rotated = inspect(&mut engine, composition).instances.remove(0);
    let mut expected = instance.definition.clone();
    expected.transform.rotation = std::f32::consts::FRAC_PI_2;
    assert_eq!(rotated.definition, expected);
    edit(
        &mut engine,
        composition,
        request(&rotated, InstanceGeometryMode::Scale, [80.0, 110.0], [40.0, 95.0]),
    );
    let scaled = inspect(&mut engine, composition).instances.remove(0);
    assert!((scaled.definition.transform.scale[0] - 1.0).abs() < 1e-5);
    assert!((scaled.definition.transform.scale[1] - 9.0).abs() < 1e-5);
    expected.transform.scale = scaled.definition.transform.scale;
    assert_eq!(scaled.definition, expected);
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
            request(&instance, InstanceGeometryMode::Scale, [110.0, 100.0], [120.0; 2]),
        ),
        (
            composition,
            request(&instance, InstanceGeometryMode::Scale, [110.0; 2], [100.0; 2]),
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
