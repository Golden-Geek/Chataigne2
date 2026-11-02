use super::*;
use crate::app::AppNode;
use golden_core::app::{from_sparse_project_json, to_sparse_project_json_pretty};
use golden_video::plan::ProjectiveMapping;

#[test]
fn a_held_warp_crosses_invalid_shapes_to_upside_down_with_one_undo_step() {
    let mut engine = fixture();
    let (_, reference) = create(&mut engine);
    let original = definition(&engine, reference);
    let history = engine.undo_len();
    super::gestures::session(&mut engine, true);
    let rotated = [
        Point { x: 1.0, y: 1.0 },
        Point { x: 0.0, y: 1.0 },
        Point { x: 0.0, y: 0.0 },
        Point { x: 1.0, y: 0.0 },
    ];
    for (corner, point) in rotated.into_iter().enumerate() {
        let ScreenEditResult::SurfaceChanged { snapshot } = apply(
            &mut engine,
            ScreenEdit::EditSurfaceGeometry {
                reference,
                edit: SurfaceGeometryEdit::QuadCorner {
                    corner: corner as u8,
                    point,
                },
            },
        ) else {
            panic!()
        };
        assert_eq!(snapshot.definition.quad.corners[corner], point);
        assert_eq!(snapshot.mapping_valid, corner == 3);
        assert_eq!(definition(&engine, reference), snapshot.definition);
        // An invalid intermediate shape is a valid saved project, not a transient UI copy.
        let saved = to_sparse_project_json_pretty(&engine).unwrap();
        let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
        loaded.apply_edits().unwrap();
        super::super::validate_project(&loaded).unwrap();
        assert_eq!(definition(&loaded, reference), snapshot.definition);
    }
    super::gestures::session(&mut engine, false);
    let final_state = definition(&engine, reference);
    assert_eq!(final_state.quad.corners, rotated);
    let mapping = ProjectiveMapping::new(final_state.crop, final_state.quad).unwrap();
    assert_eq!(
        mapping.source_coordinate(Point { x: 0.75, y: 0.75 }),
        Some([0.25, 0.25])
    );
    assert_eq!(engine.undo_len(), history + 1);
    assert!(engine.undo().unwrap());
    assert_eq!(definition(&engine, reference), original);
    assert!(engine.redo().unwrap());
    assert_eq!(definition(&engine, reference), final_state);
}

#[test]
fn crossed_concave_collapsed_and_unstable_quads_are_authored_but_not_renderable() {
    let mut engine = fixture();
    let (_, reference) = create(&mut engine);
    let normal = Quad::default().corners;
    for corners in [
        [normal[0], normal[2], normal[1], normal[3]],
        [normal[0], normal[1], Point { x: 0.2, y: 0.2 }, normal[3]],
        [normal[0]; 4],
        normal.map(|point| Point {
            x: point.x * 1e-6,
            y: point.y * 1e-6,
        }),
    ] {
        let authored = SurfaceDefinition {
            quad: Quad { corners },
            ..Default::default()
        };
        apply(
            &mut engine,
            ScreenEdit::ConfigureSurface {
                reference,
                definition: authored.clone(),
            },
        );
        let ScreenEditResult::Surface { snapshot } = apply(&mut engine, ScreenEdit::InspectSurface { reference })
        else {
            panic!()
        };
        assert_eq!(snapshot.definition, authored);
        assert!(!snapshot.mapping_valid);
        super::super::validate_project(&engine).unwrap();
    }
}
