use super::*;
use crate::app::AppNode;
use golden_core::{
    app::{ProjectNode, from_sparse_project_json, to_sparse_project_json_pretty},
    node::Node,
    parameter::{ParamValue, ParameterControlMode, ParameterControlSpec, ParameterControlState},
};

fn member(reference: SurfaceReference, corner: u8) -> ScreenCorner {
    ScreenCorner { reference, corner }
}

fn param(engine: &AppEngine, corner: ScreenCorner) -> golden_core::node::NodeId {
    super::super::handle_links::parameter(engine, corner).unwrap()
}

fn link(engine: &mut AppEngine, source: ScreenCorner, target: ScreenCorner) {
    apply(engine, ScreenEdit::LinkScreenCorners { source, target });
}

fn position(engine: &AppEngine, corner: ScreenCorner) -> Point {
    definition(engine, corner.reference).quad.corners[usize::from(corner.corner)]
}

#[test]
fn groups_propagate_geometry_and_generic_parameter_edits_from_every_member_in_one_transaction() {
    let mut engine = fixture();
    let corners: Vec<_> = (0..4)
        .map(|index| {
            let (_, surface) = create(&mut engine);
            member(surface, index)
        })
        .collect();
    for &source in &corners[..3] {
        link(&mut engine, source, corners[3]);
    }
    assert_eq!(engine.parameter_binding_group(param(&engine, corners[0])).len(), 4);
    for (index, &source) in corners.iter().enumerate() {
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        let history = engine.undo_len();
        let point = Point {
            x: 0.125 * (index as f32 + 1.0),
            y: 0.75,
        };
        if index % 2 == 0 {
            apply(
                &mut engine,
                ScreenEdit::EditSurfaceGeometry {
                    reference: source.reference,
                    edit: SurfaceGeometryEdit::QuadCorner {
                        corner: source.corner,
                        point,
                    },
                },
            );
        } else {
            let param = param(&engine, source);
            assert!(
                super::parameters::set(
                    &mut engine,
                    param,
                    ParamValue::Vec2(f64::from(point.x), f64::from(point.y))
                )
                .success
            );
        }
        for &corner in &corners {
            assert_eq!(position(&engine, corner), point);
        }
        assert_eq!(engine.undo_len(), history + 1);
        let after = to_sparse_project_json_pretty(&engine).unwrap();
        assert!(engine.undo().unwrap());
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
        assert!(engine.redo().unwrap());
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), after);
    }
}

#[test]
fn merge_unlink_and_reload_keep_one_history_step_and_real_binding_controls() {
    let mut engine = fixture();
    let corners: Vec<_> = (0..4).map(|_| member(create(&mut engine).1, 0)).collect();
    link(&mut engine, corners[0], corners[1]);
    link(&mut engine, corners[2], corners[3]);
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    link(&mut engine, corners[0], corners[2]);
    assert_eq!(engine.undo_len(), history + 1);
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    assert!(engine.undo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    assert!(engine.redo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    assert_eq!(loaded.parameter_binding_group(param(&loaded, corners[0])).len(), 4);
    let loaded_before = to_sparse_project_json_pretty(&loaded).unwrap();
    let history = loaded.undo_len();
    apply(&mut loaded, ScreenEdit::UnlinkScreenCorners { member: corners[1] });
    assert_eq!(loaded.undo_len(), history + 1);
    for &corner in &corners {
        let param = param(&loaded, corner);
        assert_eq!(loaded.parameter_binding_group(param), vec![param]);
        assert_eq!(
            loaded
                .nodes
                .get(param)
                .unwrap()
                .engine_param_control_state()
                .unwrap()
                .mode,
            ParameterControlMode::Manual
        );
    }
    assert!(loaded.undo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&loaded).unwrap(), loaded_before);
}

#[test]
fn same_screen_invalid_corner_and_conflicting_controls_reject_without_authored_changes() {
    let mut engine = fixture();
    let (_, a) = create(&mut engine);
    let (_, b) = create(&mut engine);
    let controlled = param(&engine, member(b, 0));
    engine.edits.push(golden_core::edit::Edit::SetParamControlState {
        node: controlled,
        state: ParameterControlState::new(ParameterControlMode::Expression, ParameterControlSpec::Expression),
    });
    engine.apply_edits().unwrap();
    for target in [member(a, 1), member(b, 4), member(b, 0)] {
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        let history = engine.undo_len();
        assert!(
            !invoke(
                &mut engine,
                ScreenEdit::LinkScreenCorners {
                    source: member(a, 0),
                    target
                }
            )
            .success
        );
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
        assert_eq!(engine.undo_len(), history);
    }
}

#[test]
fn deleted_target_retains_uuid_binding_and_undo_restores_group_without_relinking_by_label() {
    let mut engine = fixture();
    let (_, a) = create(&mut engine);
    let (_, b) = create(&mut engine);
    link(&mut engine, member(a, 0), member(b, 0));
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    apply(&mut engine, ScreenEdit::RemoveScreen { screen: b.screen });
    let source = param(&engine, member(a, 0));
    assert_eq!(engine.parameter_binding_group(source), vec![source]);
    assert_eq!(
        engine
            .nodes
            .get(source)
            .unwrap()
            .engine_param_control_state()
            .unwrap()
            .mode,
        ParameterControlMode::Binding
    );
    assert!(engine.undo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    assert_eq!(engine.parameter_binding_group(param(&engine, member(a, 0))).len(), 2);
}

#[test]
fn copied_surface_retains_external_binding_and_group_can_still_be_unlinked() {
    let mut engine = fixture();
    let (_, a) = create(&mut engine);
    let (_, b) = create(&mut engine);
    link(&mut engine, member(a, 0), member(b, 0));
    let root = engine.node_id_by_uuid(a.surface.0).unwrap();
    let parent = engine.node_id_by_uuid(a.screen.0).unwrap();
    let copied = engine
        .duplicate_subtree_with(
            root,
            parent,
            Some(root),
            None,
            Node::project_encode_data,
            AppNode::project_decode_node,
        )
        .unwrap();
    let copied = member(
        SurfaceReference {
            screen: a.screen,
            surface: SurfaceId(engine.nodes.get(copied).unwrap().node_data().meta.uuid),
        },
        0,
    );
    assert_eq!(engine.parameter_binding_group(param(&engine, copied)).len(), 3);
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    apply(&mut engine, ScreenEdit::UnlinkScreenCorners { member: copied });
    for corner in [member(a, 0), member(b, 0), copied] {
        let param = param(&engine, corner);
        assert_eq!(engine.parameter_binding_group(param), vec![param]);
    }
    assert!(engine.undo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
}

#[test]
fn recreating_a_screen_with_the_same_label_does_not_retarget_a_missing_binding() {
    let mut engine = fixture();
    let (_, a) = create(&mut engine);
    let (_, b) = create(&mut engine);
    let original_label = engine
        .nodes
        .get(engine.node_id_by_uuid(b.screen.0).unwrap())
        .unwrap()
        .node_data()
        .meta
        .label
        .clone();
    link(&mut engine, member(a, 0), member(b, 0));
    apply(&mut engine, ScreenEdit::RemoveScreen { screen: b.screen });
    let (_, replacement) = create(&mut engine);
    assert_ne!(replacement.screen, b.screen);
    assert_eq!(
        engine
            .nodes
            .get(engine.node_id_by_uuid(replacement.screen.0).unwrap())
            .unwrap()
            .node_data()
            .meta
            .label,
        original_label
    );
    let source = param(&engine, member(a, 0));
    assert_eq!(engine.parameter_binding_group(source), vec![source]);
    assert!(engine.parameter_binding_target(source).is_none());
    apply(&mut engine, ScreenEdit::UnlinkScreenCorners { member: member(a, 0) });
    assert_eq!(
        engine
            .nodes
            .get(source)
            .unwrap()
            .engine_param_control_state()
            .unwrap()
            .mode,
        ParameterControlMode::Manual
    );
}
