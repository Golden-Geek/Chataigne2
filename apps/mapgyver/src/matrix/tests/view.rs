use super::*;
use std::sync::Arc;

fn viewport(engine: &mut AppEngine, media: MediaId, row: u32, column: u32) -> MatrixViewportSnapshot {
    let MediaEditResult::MatrixViewport { snapshot } = edit(
        engine,
        media,
        MatrixEdit::InspectViewport {
            viewport: MatrixViewport {
                row_start: row,
                row_count: 16,
                column_start: column,
                column_count: 8,
            },
        },
    ) else {
        panic!()
    };
    snapshot
}

#[test]
fn viewport_reads_are_bounded_sparse_cached_and_do_not_launch_or_change_authored_history() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let matrix = create(&mut engine, MediaKind::Matrix);
    let items = layers(&mut engine, matrix.media, None, 1000);
    let columns = columns(&mut engine, matrix.media, 1000);
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::SetCells {
            assignments: vec![CellAssignment {
                layer: layer(items[999]),
                column: columns[999],
                source: Some(source),
            }],
        },
    );
    let history = engine.undo_len();
    let authored = to_sparse_project_json_pretty(&engine).unwrap();
    let first = viewport(&mut engine, matrix.media, 0, 0);
    assert_eq!((first.row_total, first.column_total), (1000, 1000));
    assert_eq!((first.rows.len(), first.columns.len(), first.cells.len()), (16, 8, 0));
    let root = engine.node_id_by_uuid(matrix.channel.0).unwrap();
    let AppNode::MediaChannel(node) = engine.nodes.get(root).unwrap() else {
        panic!()
    };
    let cached = node.matrix_playback().unwrap().view().unwrap();
    let last = viewport(&mut engine, matrix.media, 990, 996);
    assert_eq!((last.rows.len(), last.columns.len(), last.cells.len()), (10, 4, 1));
    assert_eq!(last.cells[0].cell.definition.source, Some(source));
    assert!(!last.cells[0].source_missing);
    assert_eq!(last.playback.desired_column, None);
    let AppNode::MediaChannel(node) = engine.nodes.get(root).unwrap() else {
        panic!()
    };
    assert!(Arc::ptr_eq(&cached, &node.matrix_playback().unwrap().view().unwrap()));
    assert!(serde_json::to_vec(&last).unwrap().len() < 20_000);
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), authored);
    assert!(viewport(&mut engine, matrix.media, u32::MAX, u32::MAX).rows.is_empty());
    let MediaEditResult::Choices { page } = apply(
        &mut engine,
        MediaEdit::InspectChoices {
            query: ChannelChoiceQuery {
                scope: ChannelChoiceScope::MatrixFinal,
                search: String::new(),
                start: 0,
                count: 64,
            },
        },
    ) else {
        panic!()
    };
    assert_eq!(page.total, 1);
    assert_eq!(page.choices[0].reference, matrix);
}

#[test]
fn viewport_refreshes_structure_payload_labels_history_and_missing_source_without_persisting_indices() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let matrix = create(&mut engine, MediaKind::Matrix);
    let items = layers(&mut engine, matrix.media, None, 2);
    let columns = columns(&mut engine, matrix.media, 2);
    viewport(&mut engine, matrix.media, 0, 0);
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::RenameItem {
            item: items[0],
            label: "Renamed".into(),
        },
    );
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::SetCells {
            assignments: vec![CellAssignment {
                layer: layer(items[0]),
                column: columns[0],
                source: Some(source),
            }],
        },
    );
    let current = viewport(&mut engine, matrix.media, 0, 0);
    assert_eq!(current.rows[0].item.label, "Renamed");
    assert_eq!(current.cells[0].cell.definition.source, Some(source));
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::MoveItem {
            item: items[0],
            parent: None,
            after: Some(items[1]),
        },
    );
    assert_eq!(viewport(&mut engine, matrix.media, 0, 0).rows[1].item.id, items[0]);
    assert!(engine.undo().unwrap());
    assert_eq!(viewport(&mut engine, matrix.media, 0, 0).rows[0].item.id, items[0]);
    apply(&mut engine, MediaEdit::RemoveMedia { media: source.media });
    let missing = viewport(&mut engine, matrix.media, 0, 0);
    assert!(missing.cells[0].source_missing);
    assert!(missing.cells[0].source_label.is_none());
    assert!(engine.undo().unwrap());
    assert!(!viewport(&mut engine, matrix.media, 0, 0).cells[0].source_missing);
    let mut restored = from_sparse_project_json::<AppNode>(&to_sparse_project_json_pretty(&engine).unwrap()).unwrap();
    assert_eq!(
        viewport(&mut restored, matrix.media, 0, 0).cells[0].cell.id,
        current.cells[0].cell.id
    );
}

#[test]
fn oversized_viewports_and_choice_queries_reject_without_changes() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let history = engine.undo_len();
    for (rows, cols) in [(0, 1), (1, 0), (129, 1), (1, 65), (128, 64)] {
        assert!(
            !invoke(
                &mut engine,
                MediaEdit::Matrix {
                    media: matrix.media,
                    edit: MatrixEdit::InspectViewport {
                        viewport: MatrixViewport {
                            row_start: 0,
                            row_count: rows,
                            column_start: 0,
                            column_count: cols
                        }
                    }
                }
            )
            .success
        );
    }
    for count in [0, 129] {
        assert!(
            !invoke(
                &mut engine,
                MediaEdit::InspectChoices {
                    query: ChannelChoiceQuery {
                        scope: ChannelChoiceScope::All,
                        search: String::new(),
                        start: 0,
                        count
                    }
                }
            )
            .success
        );
    }
    assert_eq!(engine.undo_len(), history);
}
