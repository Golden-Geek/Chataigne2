use super::*;

#[test]
fn one_atomic_assignment_replaces_edges_without_false_cycles_and_rejects_new_feedback() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let matrix = create(&mut engine, MediaKind::Matrix);
    let items = layers(&mut engine, matrix.media, None, 2);
    let columns = columns(&mut engine, matrix.media, 1);
    let snapshot = inspect(&mut engine, matrix.media);
    let a = snapshot.items.iter().find(|item| item.id == items[0]).unwrap().channel;
    let b = snapshot.items.iter().find(|item| item.id == items[1]).unwrap().channel;
    let address = |item, source| CellAssignment {
        layer: layer(item),
        column: columns[0],
        source: Some(source),
    };
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::SetCells {
            assignments: vec![address(items[0], b), address(items[1], source)],
        },
    );
    let history = engine.undo_len();
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::SetCells {
            assignments: vec![address(items[0], source), address(items[1], a)],
        },
    );
    assert_eq!(engine.undo_len(), history + 1);
    let before = inspect(&mut engine, matrix.media);
    for assignments in [
        vec![address(items[0], b), address(items[1], a)],
        vec![address(items[0], source), address(items[0], source)],
    ] {
        assert!(
            !invoke(
                &mut engine,
                MediaEdit::Matrix {
                    media: matrix.media,
                    edit: MatrixEdit::SetCells { assignments }
                }
            )
            .success
        );
        assert_eq!(inspect(&mut engine, matrix.media), before);
    }
    use crate::media::composition::protocol::CompositionEdit;
    let composition = create(&mut engine, MediaKind::Composition);
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::SetCells {
            assignments: vec![address(items[0], composition)],
        },
    );
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::Composition {
                reference: composition,
                edit: CompositionEdit::AddInstances { sources: vec![matrix] }
            }
        )
        .success
    );
}

#[test]
fn group_moves_reject_tree_and_export_feedback_without_partial_edits() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let MediaEditResult::MatrixCreated { items: groups, .. } = edit(
        &mut engine,
        matrix.media,
        MatrixEdit::AddGroup {
            parent: None,
            label: None,
        },
    ) else {
        panic!()
    };
    let MatrixItemId::Group { id: group } = groups[0] else {
        panic!()
    };
    layers(&mut engine, matrix.media, Some(group), 1);
    let outside = layers(&mut engine, matrix.media, None, 1);
    let columns = columns(&mut engine, matrix.media, 1);
    let snapshot = inspect(&mut engine, matrix.media);
    let group_channel = snapshot.items.iter().find(|item| item.id == groups[0]).unwrap().channel;
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::SetCells {
            assignments: vec![CellAssignment {
                layer: layer(outside[0]),
                column: columns[0],
                source: Some(group_channel),
            }],
        },
    );
    let before = inspect(&mut engine, matrix.media);
    let history = engine.undo_len();
    for item in [outside[0], groups[0]] {
        assert!(
            !invoke(
                &mut engine,
                MediaEdit::Matrix {
                    media: matrix.media,
                    edit: MatrixEdit::MoveItem {
                        item,
                        parent: Some(group),
                        after: None
                    }
                }
            )
            .success
        );
        assert_eq!(inspect(&mut engine, matrix.media), before);
        assert_eq!(engine.undo_len(), history);
    }
}

#[test]
fn ownership_batch_and_export_invariants_reject_before_mutation() {
    let mut engine = fixture();
    let a = create(&mut engine, MediaKind::Matrix);
    let b = create(&mut engine, MediaKind::Matrix);
    let items = layers(&mut engine, a.media, None, 1);
    let cols = columns(&mut engine, a.media, 1);
    let foreign = columns(&mut engine, b.media, 1);
    let before = inspect(&mut engine, a.media);
    let history = engine.undo_len();
    for operation in [
        MatrixEdit::AddLayers { parent: None, count: 0 },
        MatrixEdit::AddColumns { count: 4097 },
        MatrixEdit::SetCells {
            assignments: vec![CellAssignment {
                layer: layer(items[0]),
                column: foreign[0],
                source: None,
            }],
        },
        MatrixEdit::MoveItem {
            item: items[0],
            parent: None,
            after: Some(items[0]),
        },
        MatrixEdit::ReorderColumn {
            column: cols[0],
            after: Some(cols[0]),
        },
    ] {
        assert!(
            !invoke(
                &mut engine,
                MediaEdit::Matrix {
                    media: a.media,
                    edit: operation
                }
            )
            .success
        );
        assert_eq!(inspect(&mut engine, a.media), before);
        assert_eq!(engine.undo_len(), history);
    }
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::Matrix {
                media: b.media,
                edit: MatrixEdit::RemoveItem { item: items[0] }
            }
        )
        .success
    );
    for operation in [
        MediaEdit::AddChannel {
            media: a.media,
            label: None,
        },
        MediaEdit::RemoveChannel { reference: a },
        MediaEdit::RemoveChannel {
            reference: before.items[0].channel,
        },
    ] {
        assert!(!invoke(&mut engine, operation).success);
    }
    let mut definition = crate::media::channel(&engine, a).unwrap().definition;
    definition.source = ChannelSource::Matrix {
        output: MatrixOutput::Layer { layer: layer(items[0]) },
    };
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::ConfigureChannel {
                reference: a,
                definition
            }
        )
        .success
    );
    assert_eq!(inspect(&mut engine, a.media), before);
}

#[test]
fn group_depth_admission_is_iterative_and_roundtrips_at_the_boundary() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let mut parent = None;
    for _ in 0..32 {
        let MediaEditResult::MatrixCreated { items, .. } =
            edit(&mut engine, matrix.media, MatrixEdit::AddGroup { parent, label: None })
        else {
            panic!()
        };
        let MatrixItemId::Group { id } = items[0] else { panic!() };
        parent = Some(id);
    }
    let before = inspect(&mut engine, matrix.media);
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::Matrix {
                media: matrix.media,
                edit: MatrixEdit::AddGroup { parent, label: None }
            }
        )
        .success
    );
    assert_eq!(inspect(&mut engine, matrix.media), before);
    let encoded = to_sparse_project_json_pretty(&engine).unwrap();
    let mut restored = from_sparse_project_json::<AppNode>(&encoded).unwrap();
    assert_eq!(inspect(&mut restored, matrix.media), before);
}

#[test]
fn matrix_matrix_feedback_and_all_leaf_source_types_share_one_validation_graph() {
    let mut engine = fixture();
    let a = create(&mut engine, MediaKind::Matrix);
    let b = create(&mut engine, MediaKind::Matrix);
    let a_items = layers(&mut engine, a.media, None, 1);
    let b_items = layers(&mut engine, b.media, None, 1);
    let a_cols = columns(&mut engine, a.media, 1);
    let b_cols = columns(&mut engine, b.media, 1);
    let assign_a = |source| MatrixEdit::SetCells {
        assignments: vec![CellAssignment {
            layer: layer(a_items[0]),
            column: a_cols[0],
            source: Some(source),
        }],
    };
    for kind in [
        MediaKind::SolidColor,
        MediaKind::Picture,
        MediaKind::SharedTexture,
        MediaKind::Composition,
    ] {
        let source = create(&mut engine, kind);
        edit(&mut engine, a.media, assign_a(source));
    }
    edit(&mut engine, a.media, assign_a(b));
    let before = inspect(&mut engine, b.media);
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::Matrix {
                media: b.media,
                edit: MatrixEdit::SetCells {
                    assignments: vec![CellAssignment {
                        layer: layer(b_items[0]),
                        column: b_cols[0],
                        source: Some(a)
                    }]
                }
            }
        )
        .success
    );
    assert_eq!(inspect(&mut engine, b.media), before);
}

#[test]
fn project_candidate_preparation_rejects_loaded_feedback_and_orphaned_exports() {
    use golden_core::{app::ProjectLifecycle, node::Node};
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let matrix = create(&mut engine, MediaKind::Matrix);
    let items = layers(&mut engine, matrix.media, None, 1);
    let cols = columns(&mut engine, matrix.media, 1);
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::SetCells {
            assignments: vec![CellAssignment {
                layer: layer(items[0]),
                column: cols[0],
                source: Some(source),
            }],
        },
    );
    let snapshot = inspect(&mut engine, matrix.media);
    let id = engine.node_id_by_uuid(snapshot.cells[0].id.0).unwrap();
    let Some(AppNode::MatrixElement(node)) = engine.nodes.get_mut(id) else {
        panic!()
    };
    node.set_definition(ElementDefinition::Cell {
        definition: CellDefinition {
            column: cols[0],
            source: Some(matrix),
        },
    })
    .unwrap();
    let encoded = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&encoded).unwrap();
    assert!(AppNode::prepare_project_candidate(&mut loaded).is_err());
    let Some(AppNode::MatrixElement(node)) = engine.nodes.get_mut(id) else {
        panic!()
    };
    node.set_definition(ElementDefinition::Cell {
        definition: CellDefinition {
            column: cols[0],
            source: Some(source),
        },
    })
    .unwrap();
    let export = snapshot.items[0].channel;
    let export_id = engine.node_id_by_uuid(export.channel.0).unwrap();
    let Some(AppNode::MediaChannel(node)) = engine.nodes.get_mut(export_id) else {
        panic!()
    };
    let mut definition = node.definition().clone();
    definition.source = ChannelSource::Matrix {
        output: MatrixOutput::Group {
            group: GroupId(node.node_data().meta.uuid),
        },
    };
    node.set_definition(definition);
    let encoded = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&encoded).unwrap();
    assert!(AppNode::prepare_project_candidate(&mut loaded).is_err());
}

#[test]
fn generic_structural_intents_cannot_bypass_matrix_export_ownership() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let items = layers(&mut engine, matrix.media, None, 1);
    let before = inspect(&mut engine, matrix.media);
    let node = engine.node_id_by_uuid(layer(items[0]).0).unwrap();
    let root = engine.node_id_by_uuid(matrix.channel.0).unwrap();
    let source = create(&mut engine, MediaKind::SolidColor);
    let source_node = engine.node_id_by_uuid(source.media.0).unwrap();
    let history = engine.undo_len();
    for intent in [
        UiEditIntent::RemoveNodes {
            nodes: vec![source_node, node],
        },
        UiEditIntent::RemoveNode { node: root },
        UiEditIntent::MoveNode {
            node,
            new_parent: root,
            new_prev_sibling: None,
        },
        UiEditIntent::MoveNode {
            node: source_node,
            new_parent: root,
            new_prev_sibling: None,
        },
        UiEditIntent::CreateUserItem {
            parent: root,
            node_type: crate::matrix::node::LAYER_TYPE.into(),
            label: None,
            initial_params: vec![],
        },
        UiEditIntent::DuplicateNode {
            source: node,
            new_parent: root,
            new_prev_sibling: None,
            initial_params: vec![],
        },
        UiEditIntent::DuplicateNodes {
            nodes: vec![golden_core::ui_sync::UiDuplicateNodeSpec {
                source: node,
                new_parent: root,
                new_prev_sibling: None,
                initial_params: vec![],
            }],
            created_items: vec![],
            dependent_items: vec![],
        },
    ] {
        let ack = apply_project_intent(&mut engine, intent);
        assert!(!ack.success, "generic structure must use the app-owned Matrix intent");
        assert_eq!(ack.error_code.as_deref(), Some("app_intent_rejected"));
        assert_eq!(inspect(&mut engine, matrix.media), before);
        assert!(crate::media::channel(&engine, source).is_ok());
        assert_eq!(engine.undo_len(), history);
    }
    edit(&mut engine, matrix.media, MatrixEdit::RemoveItem { item: items[0] });
    assert!(inspect(&mut engine, matrix.media).items.is_empty());
}
