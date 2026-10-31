use super::*;
use crate::media::protocol::{MEDIA_NAMESPACE, MEDIA_OPERATION, MediaEdit, MediaEditResult, MediaKind};

fn authored() -> (AppEngine, SurfaceReference, SurfaceDefinition) {
    let mut engine = fixture();
    let (_, reference) = create(&mut engine);
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::InvokeAppOperation {
            namespace: MEDIA_NAMESPACE.into(),
            operation: MEDIA_OPERATION.into(),
            payload: serde_json::to_value(MediaEdit::CreateMedia {
                media_kind: MediaKind::SolidColor,
                label: None,
            })
            .unwrap(),
        },
    );
    assert!(ack.success, "{ack:?}");
    let MediaEditResult::Created { reference: source } = serde_json::from_value(ack.result.unwrap().value).unwrap()
    else {
        panic!()
    };
    let mut definition = SurfaceDefinition {
        source: Some(source),
        opacity: 0.4,
        calibration: Calibration::Grid,
        crop: UnitRect {
            x: 0.2,
            y: 0.2,
            width: 0.6,
            height: 0.6,
        },
        ..Default::default()
    };
    definition.quad.corners[0] = Point { x: 0.1, y: 0.1 };
    apply(
        &mut engine,
        ScreenEdit::ConfigureSurface {
            reference,
            definition: definition.clone(),
        },
    );
    (engine, reference, definition)
}

fn edit(engine: &mut AppEngine, reference: SurfaceReference, edit: SurfaceGeometryEdit) -> SurfaceDefinition {
    let ScreenEditResult::SurfaceChanged { snapshot } =
        apply(engine, ScreenEdit::EditSurfaceGeometry { reference, edit })
    else {
        panic!()
    };
    assert_eq!(snapshot.definition, definition(engine, reference));
    snapshot.definition
}

#[test]
fn every_crop_corner_keeps_the_opposite_corner_fixed() {
    for (corner, point, expected) in [
        (0, Point { x: 0.3, y: 0.3 }, [0.3, 0.3, 0.5, 0.5]),
        (1, Point { x: 0.7, y: 0.3 }, [0.2, 0.3, 0.5, 0.5]),
        (2, Point { x: 0.7, y: 0.7 }, [0.2, 0.2, 0.5, 0.5]),
        (3, Point { x: 0.3, y: 0.7 }, [0.3, 0.2, 0.5, 0.5]),
    ] {
        let (mut engine, reference, original) = authored();
        let result = edit(
            &mut engine,
            reference,
            SurfaceGeometryEdit::CropCorner { corner, point },
        );
        let crop = result.crop;
        for (actual, expected) in [crop.x, crop.y, crop.width, crop.height].into_iter().zip(expected) {
            assert!((actual - expected).abs() < 1e-6);
        }
        assert_eq!(result, SurfaceDefinition { crop, ..original });
    }
}

#[test]
fn crop_samples_are_atomic_and_one_undo_restores_mapping_source_and_calibration() {
    let (mut engine, reference, original) = authored();
    let history = engine.undo_len();
    super::gestures::session(&mut engine, true);
    for index in 0..100 {
        edit(
            &mut engine,
            reference,
            SurfaceGeometryEdit::CropCorner {
                corner: 2,
                point: Point {
                    x: 0.7,
                    y: 0.7 + index as f32 / 1000.0,
                },
            },
        );
    }
    let resized = definition(&engine, reference);
    let moved = edit(
        &mut engine,
        reference,
        SurfaceGeometryEdit::CropOrigin {
            point: Point { x: 0.1, y: 0.1 },
        },
    );
    assert_eq!(moved.crop.width, resized.crop.width);
    assert_eq!(moved.crop.height, resized.crop.height);
    assert_eq!(
        moved,
        SurfaceDefinition {
            crop: moved.crop,
            ..original.clone()
        }
    );
    for edit in [
        SurfaceGeometryEdit::CropCorner {
            corner: 4,
            point: Point { x: 0.3, y: 0.3 },
        },
        SurfaceGeometryEdit::CropCorner {
            corner: 0,
            point: Point { x: -0.1, y: 0.1 },
        },
        SurfaceGeometryEdit::CropCorner {
            corner: 2,
            point: Point { x: 0.1, y: 0.1 },
        },
        SurfaceGeometryEdit::CropCorner {
            corner: 2,
            point: Point { x: 1.1, y: 0.8 },
        },
        SurfaceGeometryEdit::CropOrigin {
            point: Point { x: 0.9, y: 0.1 },
        },
    ] {
        assert!(!invoke(&mut engine, ScreenEdit::EditSurfaceGeometry { reference, edit }).success);
        assert_eq!(definition(&engine, reference), moved);
    }
    super::gestures::session(&mut engine, false);
    assert_eq!(engine.undo_len(), history + 1);
    assert!(engine.undo().unwrap());
    assert_eq!(definition(&engine, reference), original);
    assert!(engine.redo().unwrap());
    assert_eq!(definition(&engine, reference), moved);
    let json = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = golden_core::app::from_sparse_project_json::<crate::app::AppNode>(&json).unwrap();
    loaded.apply_edits().unwrap();
    assert_eq!(definition(&loaded, reference), moved);
}

#[test]
fn geometry_edits_keep_missing_source_identity_and_noops_have_no_history() {
    let (mut engine, reference, original) = authored();
    let source = original.source.unwrap();
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::InvokeAppOperation {
            namespace: MEDIA_NAMESPACE.into(),
            operation: MEDIA_OPERATION.into(),
            payload: serde_json::to_value(MediaEdit::RemoveMedia { media: source.media }).unwrap(),
        },
    );
    assert!(ack.success);
    let history = engine.undo_len();
    edit(
        &mut engine,
        reference,
        SurfaceGeometryEdit::CropOrigin {
            point: Point {
                x: original.crop.x,
                y: original.crop.y,
            },
        },
    );
    assert_eq!(engine.undo_len(), history);
    let result = edit(
        &mut engine,
        reference,
        SurfaceGeometryEdit::CropOrigin {
            point: Point { x: 0.1, y: 0.1 },
        },
    );
    assert_eq!(result.source, Some(source));
    assert!(crate::media::channel(&engine, source).is_err());
    assert_eq!(result.quad, original.quad);
    super::super::validate_project(&engine).unwrap();
}
