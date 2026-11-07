use super::*;
use crate::matrix::protocol::*;

fn nodes(engine: &AppEngine, uuid: golden_core::node::NodeUuid) -> usize {
    let mut pending = vec![engine.node_id_by_uuid(uuid).unwrap()];
    let mut count = 0;
    while let Some(id) = pending.pop() {
        count += 1;
        let mut next = engine.nodes.get(id).unwrap().node_data().first_child;
        while let Some(id) = next {
            pending.push(id);
            next = engine.nodes.get(id).unwrap().node_data().next_sibling;
        }
    }
    count
}

#[test]
fn sparse_million_address_transfers_bound_authoring_to_real_cell_subtrees_for_every_caller() {
    let mut base = fixture();
    let root = create(&mut base, MediaKind::Matrix);
    let edit = |engine: &mut AppEngine, edit| {
        apply(
            engine,
            MediaEdit::Matrix {
                media: root.media,
                edit,
            },
        )
    };
    edit(
        &mut base,
        MatrixEdit::AddLayers {
            parent: None,
            count: 997,
        },
    );
    edit(
        &mut base,
        MatrixEdit::AddColumns {
            parent: None,
            count: 997,
        },
    );
    let initial = crate::matrix::inspect(&base, root.media).unwrap();
    assert_eq!((initial.items.len(), initial.columns.len()), (1000, 1000));
    let row = |index: usize| match initial.items[index].id {
        MatrixItemId::Layer { id } => id,
        _ => panic!(),
    };
    let source_row = row(0);
    let far_row = row(999);
    let first = initial.columns[0].id;
    let last = initial.columns[999].id;
    let MediaEditResult::MatrixMediaCreated { cell: source, .. } = edit(
        &mut base,
        MatrixEdit::CreateCellMedia {
            layer: source_row,
            column: first,
            media_kind: MediaKind::SolidColor,
        },
    ) else {
        panic!()
    };
    edit(
        &mut base,
        MatrixEdit::CreateCellMedia {
            layer: far_row,
            column: last,
            media_kind: MediaKind::SolidColor,
        },
    );
    let original = crate::matrix::inspect(&base, root.media).unwrap();
    let source_cell = original.cells.iter().find(|cell| cell.id == source).unwrap();
    let original_nodes = nodes(&base, root.media.0);
    let source_nodes = nodes(&base, source.0);
    let producer_nodes = nodes(&base, source_cell.media.unwrap().0);
    let saved = to_sparse_project_json_pretty(&base).unwrap();
    for caller in CALLERS {
        for action in [
            CellTransferAction::Drop,
            CellTransferAction::Move,
            CellTransferAction::Copy,
            CellTransferAction::Reference,
        ] {
            let mut engine = loaded(&saved);
            let column = if action == CellTransferAction::Drop {
                last
            } else {
                initial.columns[998].id
            };
            let (result, elapsed) = run_measured(
                &mut engine,
                caller,
                MediaEdit::Matrix {
                    media: root.media,
                    edit: MatrixEdit::TransferCell {
                        cell: source,
                        layer: far_row,
                        column,
                        action,
                    },
                },
            );
            let result: MediaEditResult = serde_json::from_value(result.unwrap()).unwrap();
            let after = crate::matrix::inspect(&engine, root.media).unwrap();
            assert_eq!(after.items, original.items);
            assert_eq!(after.columns, original.columns);
            assert_eq!(after.column_groups, original.column_groups);
            let after_nodes = nodes(&engine, root.media.0);
            if let MediaEditResult::MatrixCellPrepared { cell, .. } = result {
                assert_eq!(
                    after.cells.len(),
                    3,
                    "copy/reference must not materialize the Cartesian grid"
                );
                for old in &original.cells {
                    assert_eq!(after.cells.iter().find(|item| item.id == old.id).unwrap(), old);
                }
                let created = after.cells.iter().find(|item| item.id == cell).unwrap();
                assert_eq!((created.layer, created.definition.column), (far_row, column));
                assert_eq!(created.definition.settings, source_cell.definition.settings);
                let expected_growth = match action {
                    CellTransferAction::Copy => source_nodes,
                    CellTransferAction::Reference => {
                        source_nodes - producer_nodes + nodes(&engine, created.media.unwrap().0)
                    }
                    _ => panic!(),
                };
                assert_eq!(
                    after_nodes - original_nodes,
                    expected_growth,
                    "only the new Cell and its owned content may be added"
                );
            } else {
                assert!(matches!(result, MediaEditResult::Changed));
                assert_eq!(after.cells.len(), 2);
                assert_eq!(after_nodes, original_nodes, "move/swap must not rebuild subtrees");
                let moved = after.cells.iter().find(|cell| cell.id == source).unwrap();
                assert_eq!((moved.layer, moved.definition.column), (far_row, column));
                assert_eq!(moved.media, source_cell.media);
                assert_eq!(moved.definition.settings, source_cell.definition.settings);
                let peer = after.cells.iter().find(|cell| cell.id != source).unwrap();
                assert_eq!(
                    (peer.layer, peer.definition.column),
                    if action == CellTransferAction::Drop {
                        (source_row, first)
                    } else {
                        (far_row, last)
                    }
                );
            }
            eprintln!(
                "Sparse Cell transfer caller={caller:?} action={action:?} grid=1000x1000 cells={} matrix_nodes_before={original_nodes} matrix_nodes_after={after_nodes} operation_ms={:.3}",
                after.cells.len(),
                elapsed.as_secs_f64() * 1000.0
            );
        }
    }
}
