use super::*;
use crate::app::AppNode;
use golden_core::app::{from_sparse_project_json, to_sparse_project_json_pretty};

#[test]
fn quad_translation_preserves_shape_and_other_parameters_with_one_history_step_and_roundtrip() {
    let mut engine = fixture();
    let (_, reference) = create(&mut engine);
    let source = super::drop_sources::source(&mut engine);
    let normal = Quad::default().corners;
    for quad in [
        Quad {
            corners: [
                Point { x: 0.1, y: 0.2 },
                Point { x: 0.8, y: 0.1 },
                Point { x: 0.9, y: 0.9 },
                Point { x: 0.2, y: 0.8 },
            ],
        },
        Quad {
            corners: [normal[0], normal[2], normal[1], normal[3]],
        },
    ] {
        let original = SurfaceDefinition {
            quad,
            crop: UnitRect {
                x: 0.1,
                y: 0.2,
                width: 0.7,
                height: 0.6,
            },
            source: Some(source),
            opacity: 0.4,
            enabled: false,
            calibration: Calibration::Grid,
        };
        apply(
            &mut engine,
            ScreenEdit::ConfigureSurface {
                reference,
                definition: original.clone(),
            },
        );
        let history = engine.undo_len();
        super::gestures::session(&mut engine, true);
        for point in [
            Point { x: 0.75, y: 0.75 },
            Point { x: 0.25, y: 0.375 },
            Point { x: -0.5, y: 1.5 },
        ] {
            let origin = Point { x: 0.5, y: 0.5 };
            let ScreenEditResult::SurfaceChanged { snapshot } = apply(
                &mut engine,
                ScreenEdit::EditSurfaceGeometry {
                    reference,
                    edit: SurfaceGeometryEdit::QuadTranslation {
                        quad: original.quad,
                        origin,
                        point,
                    },
                },
            ) else {
                panic!()
            };
            let mut expected = original.clone();
            expected.quad.corners = original.quad.corners.map(|corner| Point {
                x: corner.x + point.x - origin.x,
                y: corner.y + point.y - origin.y,
            });
            for (a, b) in snapshot.definition.quad.corners.iter().zip(expected.quad.corners) {
                assert!((a.x - b.x).abs() < 1e-6 && (a.y - b.y).abs() < 1e-6);
            }
            expected.quad = snapshot.definition.quad;
            assert_eq!(snapshot.definition, expected);
            assert_eq!(definition(&engine, reference), expected);
        }
        super::gestures::session(&mut engine, false);
        let final_state = definition(&engine, reference);
        assert_eq!(engine.undo_len(), history + 1);
        assert!(engine.undo().unwrap());
        assert_eq!(definition(&engine, reference), original);
        assert!(engine.redo().unwrap());
        assert_eq!(definition(&engine, reference), final_state);
        let saved = to_sparse_project_json_pretty(&engine).unwrap();
        let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
        loaded.apply_edits().unwrap();
        super::super::validate_project(&loaded).unwrap();
        assert_eq!(definition(&loaded, reference), final_state);
    }
}

#[test]
fn translation_overflow_and_nonfinite_input_reject_without_authored_or_history_changes() {
    let mut engine = fixture();
    let (_, reference) = create(&mut engine);
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    for (origin, point) in [
        (Point { x: -f32::MAX, y: 0.0 }, Point { x: f32::MAX, y: 0.0 }),
        (Point { x: 0.0, y: f32::NAN }, Point { x: 0.0, y: 0.0 }),
    ] {
        assert!(
            !invoke(
                &mut engine,
                ScreenEdit::EditSurfaceGeometry {
                    reference,
                    edit: SurfaceGeometryEdit::QuadTranslation {
                        quad: Quad::default(),
                        origin,
                        point,
                    }
                }
            )
            .success
        );
        assert_eq!(engine.undo_len(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    }
}
