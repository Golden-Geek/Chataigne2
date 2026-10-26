use super::*;

mod native;

fn identity(engine: &crate::app::AppEngine, node: NodeId) -> FormulaOwnerDto {
    FormulaOwnerDto {
        node,
        uuid: engine.nodes.get(node).unwrap().node_data().meta.uuid,
    }
}
fn edit(
    engine: &mut crate::app::AppEngine,
    formula: &FormulaOwnerDto,
    edit: FormulaGraphEdit,
) -> Result<FormulaGraphEditResult, Box<UiAck>> {
    invoke(
        engine,
        FORMULA_GRAPH_EDIT_OPERATION,
        FormulaGraphEditRequest {
            formula: formula.clone(),
            edit,
        },
    )
}
fn moves(engine: &crate::app::AppEngine, nodes: &[NodeId]) -> FormulaGraphEdit {
    FormulaGraphEdit::Move {
        moves: nodes
            .iter()
            .map(|node| FormulaGraphMove {
                anode: identity(engine, *node),
                position: [8., 3.],
            })
            .collect(),
    }
}

#[test]
fn formula_graph_movement_is_one_atomic_undo_group() {
    let (mut engine, formula) = fixture();
    let a = anode(&mut engine, &formula, "constant");
    let b = anode(&mut engine, &formula, "math");
    let history = engine.undo_len();
    let request = moves(&engine, &[a, b]);
    let reply = edit(&mut engine, &formula, request).unwrap();
    assert_eq!(reply.nodes.len(), 2);
    assert_eq!(engine.undo_len(), history + 1);
    let snapshot = engine.process_tree_snapshot();
    assert_eq!(child_vec2(&snapshot, a, "position"), Some([8., 3.]));
    assert_eq!(child_vec2(&snapshot, b, "position"), Some([8., 3.]));
    assert!(engine.undo().unwrap());
    assert_eq!(
        child_vec2(&engine.process_tree_snapshot(), a, "position"),
        Some([0., 0.])
    );
    assert!(engine.redo().unwrap());
    assert_eq!(
        child_vec2(&engine.process_tree_snapshot(), b, "position"),
        Some([8., 3.])
    );
}

#[test]
fn formula_graph_rejects_readonly_later_target_without_a_move_prefix() {
    let (mut engine, formula) = fixture();
    let a = anode(&mut engine, &formula, "constant");
    let b = anode(&mut engine, &formula, "math");
    let position = engine
        .process_tree_snapshot()
        .find_child_by_decl_id(b, "position")
        .unwrap();
    engine
        .nodes
        .get_mut(position)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<Parameter>()
        .unwrap()
        .read_only = true;
    let history = engine.undo_len();
    let request = moves(&engine, &[a, b]);
    assert!(edit(&mut engine, &formula, request).is_err());
    assert_eq!(engine.undo_len(), history);
    assert_eq!(
        child_vec2(&engine.process_tree_snapshot(), a, "position"),
        Some([0., 0.])
    );
}

#[test]
fn formula_graph_rejects_stale_foreign_duplicate_and_readonly_owners() {
    let (mut engine, formula) = fixture();
    let a = anode(&mut engine, &formula, "constant");
    let id = identity(&engine, a);
    let history = engine.undo_len();
    for target in [
        FormulaOwnerDto {
            uuid: NodeUuid::nil(),
            ..id.clone()
        },
        formula.clone(),
    ] {
        assert!(edit(
            &mut engine,
            &formula,
            FormulaGraphEdit::Move {
                moves: vec![FormulaGraphMove {
                    anode: target,
                    position: [1., 2.]
                }]
            }
        )
        .is_err());
    }
    let request = moves(&engine, &[a, a]);
    assert!(edit(&mut engine, &formula, request).is_err());
    let stale = FormulaOwnerDto {
        uuid: NodeUuid::nil(),
        ..formula.clone()
    };
    assert!(edit(
        &mut engine,
        &stale,
        FormulaGraphEdit::Collapse {
            anode: id.clone(),
            collapsed: true
        }
    )
    .is_err());
    engine
        .nodes
        .get_mut(formula.node)
        .unwrap()
        .node_data_mut()
        .meta
        .tags
        .push(FORMULA_EXTERNAL_READ_ONLY_TAG.into());
    assert!(edit(
        &mut engine,
        &formula,
        FormulaGraphEdit::Rename {
            anode: id,
            label: "Blocked".into()
        }
    )
    .is_err());
    assert_eq!(engine.undo_len(), history);
}

#[test]
fn formula_graph_sizing_and_metadata_are_backend_owned_and_reversible() {
    let (mut engine, formula) = fixture();
    let a = anode(&mut engine, &formula, "constant");
    let id = identity(&engine, a);
    assert!(edit(
        &mut engine,
        &formula,
        FormulaGraphEdit::Resize {
            anode: id.clone(),
            size: Some([-1., 2.])
        }
    )
    .is_err());
    edit(
        &mut engine,
        &formula,
        FormulaGraphEdit::Resize {
            anode: id.clone(),
            size: Some([24., 12.]),
        },
    )
    .unwrap();
    assert_eq!(child_vec2(&engine.process_tree_snapshot(), a, "size"), Some([24., 12.]));
    edit(
        &mut engine,
        &formula,
        FormulaGraphEdit::Rename {
            anode: id.clone(),
            label: "  Renamed  ".into(),
        },
    )
    .unwrap();
    assert_eq!(engine.nodes.get(a).unwrap().node_data().meta.label, "Renamed");
    edit(
        &mut engine,
        &formula,
        FormulaGraphEdit::Collapse {
            anode: id.clone(),
            collapsed: true,
        },
    )
    .unwrap();
    assert!(engine.nodes.get(a).unwrap().node_data().meta.presentation.collapsed);
    edit(
        &mut engine,
        &formula,
        FormulaGraphEdit::Enable {
            anode: id,
            enabled: false,
        },
    )
    .unwrap();
    assert!(!engine.nodes.get(a).unwrap().node_data().meta.enabled);
    assert!(engine.undo().unwrap());
    assert!(engine.nodes.get(a).unwrap().node_data().meta.enabled);
}

#[test]
fn formula_graph_delete_removes_incident_edges_and_restores_exact_identities() {
    let (mut engine, formula, a, b) = connected_fixture();
    let a_id = identity(&engine, a);
    let edge = engine
        .process_tree_snapshot()
        .child_ids(formula.node)
        .into_iter()
        .find(|id| engine.nodes.get(*id).unwrap().get_type() == CONNECTION_NODE_TYPE)
        .unwrap();
    let edge_id = identity(&engine, edge);
    let history = engine.undo_len();
    assert!(edit(
        &mut engine,
        &formula,
        FormulaGraphEdit::Delete {
            nodes: vec![
                a_id.clone(),
                FormulaOwnerDto {
                    uuid: NodeUuid::nil(),
                    node: b
                }
            ]
        }
    )
    .is_err());
    assert!(engine.nodes.get(a).is_some());
    assert_eq!(engine.undo_len(), history);
    edit(
        &mut engine,
        &formula,
        FormulaGraphEdit::Delete {
            nodes: vec![a_id.clone()],
        },
    )
    .unwrap();
    assert!(engine.nodes.get(a).is_none());
    assert!(engine.nodes.get(edge).is_none());
    assert!(engine.nodes.get(b).is_some());
    assert_eq!(engine.undo_len(), history + 1);
    assert!(engine.undo().unwrap());
    assert_eq!(identity(&engine, a).uuid, a_id.uuid);
    assert_eq!(identity(&engine, edge).uuid, edge_id.uuid);
}
