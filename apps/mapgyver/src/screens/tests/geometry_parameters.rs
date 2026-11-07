use super::parameters::{parameters, set};
use super::*;
use golden_core::{app::to_sparse_project_json_pretty, node::Node, parameter::ParamValue};
use std::collections::HashMap;

#[test]
fn geometry_intents_preserve_unrelated_authored_f64_parameters_and_exact_history() {
    let mut engine = fixture();
    let (_, reference) = create(&mut engine);
    let root = engine.node_id_by_uuid(reference.surface.0).unwrap();
    let fields = parameters(&engine, root);
    for (name, value) in [
        ("opacity", ParamValue::Float(0.123456789012345)),
        ("crop_origin", ParamValue::Vec2(0.123456789012345, 0.234567890123456)),
        ("crop_size", ParamValue::Vec2(0.654321098765432, 0.543210987654321)),
        ("bottom_right", ParamValue::Vec2(0.987654321098765, 0.876543210987654)),
    ] {
        assert!(set(&mut engine, fields[&format!("parameters/{name}")], value).success);
    }
    for edit in [
        ScreenEdit::EditSurfaceGeometry {
            reference,
            edit: SurfaceGeometryEdit::QuadCorner {
                corner: 0,
                point: Point { x: 0.1, y: 0.2 },
            },
        },
        ScreenEdit::EditSurfaceGeometry {
            reference,
            edit: SurfaceGeometryEdit::CropOrigin {
                point: Point { x: 0.1, y: 0.2 },
            },
        },
        ScreenEdit::FitSurface {
            reference,
            mode: SurfaceFitMode::Fill,
        },
    ] {
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        let original: HashMap<_, _> = fields
            .iter()
            .map(|(name, id)| {
                (
                    name.clone(),
                    engine.nodes.get(*id).unwrap().engine_param_snapshot().unwrap().value,
                )
            })
            .collect();
        let changed: &[&str] = match &edit {
            ScreenEdit::EditSurfaceGeometry {
                edit: SurfaceGeometryEdit::QuadCorner { .. },
                ..
            } => &["parameters/top_left"],
            ScreenEdit::EditSurfaceGeometry { .. } => &["parameters/crop_origin"],
            _ => &[
                "parameters/top_left",
                "parameters/top_right",
                "parameters/bottom_right",
                "parameters/bottom_left",
            ],
        };
        let history = engine.undo_len();
        apply(&mut engine, edit);
        for (name, id) in &fields {
            if !changed.contains(&name.as_str()) {
                assert_eq!(
                    engine.nodes.get(*id).unwrap().engine_param_snapshot().unwrap().value,
                    original[name],
                    "{name}"
                );
            }
        }
        assert_eq!(engine.undo_len(), history + 1);
        let after = to_sparse_project_json_pretty(&engine).unwrap();
        assert!(engine.undo().unwrap());
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
        assert!(engine.redo().unwrap());
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), after);
    }
}
