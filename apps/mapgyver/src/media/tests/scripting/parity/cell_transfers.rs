use super::*;
use crate::matrix::protocol::*;

fn matrix(engine: &mut AppEngine, media: MediaId, edit: MatrixEdit) -> MediaEditResult {
    apply(engine, MediaEdit::Matrix { media, edit })
}

#[test]
fn cell_transfers_share_atomic_history_rejection_and_ownership_for_all_public_script_and_transport_callers() {
    let mut base = fixture();
    let root = create(&mut base, MediaKind::Matrix);
    let other = create(&mut base, MediaKind::Matrix);
    let MediaEditResult::Matrix { snapshot } = matrix(&mut base, root.media, MatrixEdit::Inspect) else {
        panic!()
    };
    let MediaEditResult::Matrix { snapshot: foreign } = matrix(&mut base, other.media, MatrixEdit::Inspect) else {
        panic!()
    };
    let rows = snapshot
        .items
        .iter()
        .map(|item| match item.id {
            MatrixItemId::Layer { id } => id,
            _ => panic!(),
        })
        .collect::<Vec<_>>();
    let columns = snapshot.columns.iter().map(|column| column.id).collect::<Vec<_>>();
    let MatrixItemId::Layer { id: foreign_row } = foreign.items[0].id else {
        panic!()
    };
    let MediaEditResult::MatrixMediaCreated { cell: source, .. } = matrix(
        &mut base,
        root.media,
        MatrixEdit::CreateCellMedia {
            layer: rows[0],
            column: columns[0],
            media_kind: MediaKind::SolidColor,
        },
    ) else {
        panic!()
    };
    let MediaEditResult::MatrixMediaCreated { cell: peer, .. } = matrix(
        &mut base,
        root.media,
        MatrixEdit::CreateCellMedia {
            layer: rows[1],
            column: columns[1],
            media_kind: MediaKind::SolidColor,
        },
    ) else {
        panic!()
    };
    matrix(
        &mut base,
        root.media,
        MatrixEdit::ConfigureCellApplication {
            layer: rows[0],
            column: columns[0],
            definition: crate::applications::protocol::ApplicationDefinition {
                executable: Some("C:\\missing\\caller-producer.exe".into()),
                ..Default::default()
            },
            pre: 0,
            post: 0,
        },
    );
    let initial = crate::matrix::inspect(&base, root.media).unwrap();
    let original = initial.cells.iter().find(|cell| cell.id == source).unwrap();
    let saved = to_sparse_project_json_pretty(&base).unwrap();
    for caller in CALLERS {
        for (target, row, column, action, accepted) in [
            (root.media, rows[1], columns[1], CellTransferAction::Drop, true),
            (root.media, rows[2], columns[2], CellTransferAction::Drop, true),
            (root.media, rows[2], columns[2], CellTransferAction::Move, true),
            (root.media, rows[0], columns[0], CellTransferAction::Move, true),
            (root.media, rows[0], columns[0], CellTransferAction::Drop, true),
            (root.media, rows[1], columns[1], CellTransferAction::Move, false),
            (root.media, rows[2], columns[2], CellTransferAction::Copy, true),
            (root.media, rows[2], columns[2], CellTransferAction::Reference, true),
            (
                other.media,
                foreign_row,
                foreign.columns[0].id,
                CellTransferAction::Copy,
                true,
            ),
            (
                other.media,
                foreign_row,
                foreign.columns[0].id,
                CellTransferAction::Reference,
                true,
            ),
            (root.media, rows[1], columns[1], CellTransferAction::Copy, false),
            (root.media, rows[1], columns[1], CellTransferAction::Reference, false),
            (
                other.media,
                foreign_row,
                foreign.columns[0].id,
                CellTransferAction::Drop,
                false,
            ),
            (
                other.media,
                foreign_row,
                foreign.columns[0].id,
                CellTransferAction::Move,
                false,
            ),
        ] {
            let mut engine = loaded(&saved);
            let result = run(
                &mut engine,
                caller,
                MediaEdit::Matrix {
                    media: target,
                    edit: MatrixEdit::TransferCell {
                        cell: source,
                        layer: row,
                        column,
                        action,
                    },
                },
            );
            assert_eq!(result.is_some(), accepted, "{caller:?} {action:?}");
            if !accepted {
                continue;
            }
            let result: MediaEditResult = serde_json::from_value(result.unwrap()).unwrap();
            let current = crate::matrix::inspect(&engine, target).unwrap();
            if matches!(action, CellTransferAction::Copy | CellTransferAction::Reference) {
                let MediaEditResult::MatrixCellPrepared { cell, .. } = result else {
                    panic!()
                };
                let created = current.cells.iter().find(|item| item.id == cell).unwrap();
                assert_eq!(created.layer, row);
                assert_eq!(created.definition.column, column);
                assert_eq!(created.definition.settings, original.definition.settings);
                assert_ne!(created.media, original.media);
                assert_eq!(
                    created.definition.source.unwrap().channel == original.definition.source.unwrap().channel,
                    action == CellTransferAction::Reference
                );
                assert_eq!(
                    created.definition.application.0,
                    if action == CellTransferAction::Reference {
                        source.0
                    } else {
                        cell.0
                    }
                );
            } else {
                assert!(matches!(result, MediaEditResult::Changed));
                let moved = current.cells.iter().find(|cell| cell.id == source).unwrap();
                assert_eq!((moved.layer, moved.definition.column), (row, column));
                assert_eq!(moved.media, original.media);
                assert_eq!(
                    moved.definition,
                    CellDefinition {
                        column,
                        ..original.definition.clone()
                    }
                );
                if row == rows[1] && column == columns[1] {
                    let swapped = current.cells.iter().find(|cell| cell.id == peer).unwrap();
                    assert_eq!((swapped.layer, swapped.definition.column), (rows[0], columns[0]));
                }
            }
            crate::media::validate_project(&engine).unwrap();
            let restored = loaded(&to_sparse_project_json_pretty(&engine).unwrap());
            assert_eq!(crate::matrix::inspect(&restored, target).unwrap(), current);
        }
    }
}
