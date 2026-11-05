use super::*;
use crate::applications::protocol::{ApplicationId, ApplicationWindow};

fn group(engine: &mut AppEngine, media: MediaId, parent: Option<ColumnGroupId>) -> ColumnGroupId {
    let MediaEditResult::MatrixColumnGroupCreated { group } =
        edit(engine, media, MatrixEdit::AddColumnGroup { parent, label: None })
    else {
        panic!()
    };
    group
}

fn nested_columns(engine: &mut AppEngine, media: MediaId, parent: ColumnGroupId, count: u16) -> Vec<ColumnId> {
    let MediaEditResult::MatrixCreated { columns, .. } = edit(
        engine,
        media,
        MatrixEdit::AddColumns {
            parent: Some(parent),
            count,
        },
    ) else {
        panic!()
    };
    columns
}

#[test]
fn nested_axis_has_one_authored_order_and_survives_ungroup_history_and_reload() {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    let first = columns(&mut engine, media, 1)[0];
    let outer = group(&mut engine, media, None);
    let direct = nested_columns(&mut engine, media, outer, 1)[0];
    let inner = group(&mut engine, media, Some(outer));
    let nested = nested_columns(&mut engine, media, inner, 2);
    let last = columns(&mut engine, media, 1)[0];
    let snapshot = inspect(&mut engine, media);
    assert_eq!(
        snapshot.columns.iter().map(|c| c.id).collect::<Vec<_>>(),
        [first, direct, nested[0], nested[1], last]
    );
    assert_eq!(snapshot.column_groups[0].column_start, 1);
    assert_eq!(snapshot.column_groups[0].column_count, 3);
    assert_eq!(snapshot.column_groups[1].parent, Some(outer));
    assert_eq!(snapshot.column_groups[1].column_count, 2);
    assert_eq!(snapshot.columns[2].parent, Some(inner));
    edit(
        &mut engine,
        media,
        MatrixEdit::LaunchColumn {
            column: nested[1],
            mode: LaunchMode::Cut,
        },
    );
    let root = super::super::playback::root(&engine, media).unwrap();
    let Some(AppNode::MediaChannel(channel)) = engine.nodes.get(root) else {
        panic!()
    };
    let control = channel.matrix_playback().unwrap().clone();
    assert!(super::super::projection::index(&engine, media, &control).is_ok());
    edit(
        &mut engine,
        media,
        MatrixEdit::InspectViewport {
            viewport: MatrixViewport {
                row_start: 0,
                row_count: 1,
                column_start: 0,
                column_count: 5,
            },
            selection: None,
        },
    );
    let history = engine.undo_len();
    edit(&mut engine, media, MatrixEdit::UngroupColumns { group: outer });
    assert_eq!(engine.undo_len(), history + 1);
    let ungrouped = inspect(&mut engine, media);
    assert_eq!(ungrouped.column_groups.len(), 1);
    assert_eq!(ungrouped.column_groups[0].parent, None);
    assert_eq!(
        ungrouped.columns.iter().map(|c| c.id).collect::<Vec<_>>(),
        snapshot.columns.iter().map(|c| c.id).collect::<Vec<_>>()
    );
    let encoded = to_sparse_project_json_pretty(&engine).unwrap();
    let mut restored = from_sparse_project_json::<AppNode>(&encoded).unwrap();
    assert_eq!(inspect(&mut restored, media), ungrouped);
    assert!(engine.undo().unwrap());
    assert_eq!(inspect(&mut engine, media), snapshot);
    assert!(engine.redo().unwrap());
    assert_eq!(inspect(&mut engine, media), ungrouped);
}

#[test]
fn empty_groups_keep_sibling_order_when_moving_and_ungrouping() {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    let left = columns(&mut engine, media, 1)[0];
    let empty = group(&mut engine, media, None);
    let right = columns(&mut engine, media, 1)[0];
    let outer = group(&mut engine, media, None);
    edit(
        &mut engine,
        media,
        MatrixEdit::MoveColumnItem {
            item: ColumnItemId::Group { id: empty },
            parent: Some(outer),
            after: None,
        },
    );
    edit(
        &mut engine,
        media,
        MatrixEdit::MoveColumnItem {
            item: ColumnItemId::Column { id: left },
            parent: Some(outer),
            after: Some(ColumnItemId::Group { id: empty }),
        },
    );
    let snapshot = inspect(&mut engine, media);
    assert_eq!(snapshot.columns.iter().map(|c| c.id).collect::<Vec<_>>(), [right, left]);
    assert_eq!(snapshot.column_groups[0].id, outer);
    assert_eq!(snapshot.column_groups[1].column_count, 0);
    assert_eq!(snapshot.column_groups[1].ordinal, 0);
    edit(&mut engine, media, MatrixEdit::UngroupColumns { group: outer });
    let ungrouped = inspect(&mut engine, media);
    assert_eq!(
        super::super::column_groups::siblings(&ungrouped, None),
        [
            ColumnItemId::Column { id: right },
            ColumnItemId::Group { id: empty },
            ColumnItemId::Column { id: left },
        ]
    );
}

#[test]
fn maximum_axis_depth_roundtrips_and_rejects_deeper_groups_without_mutation() {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    let mut parent = None;
    for _ in 0..super::super::document::MAX_DEPTH {
        parent = Some(group(&mut engine, media, parent));
    }
    let column = nested_columns(&mut engine, media, parent.unwrap(), 1)[0];
    edit(
        &mut engine,
        media,
        MatrixEdit::LaunchColumn {
            column,
            mode: LaunchMode::Cut,
        },
    );
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::Matrix {
                media,
                edit: MatrixEdit::AddColumnGroup { parent, label: None },
            }
        )
        .success
    );
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    let mut restored = from_sparse_project_json::<AppNode>(&before).unwrap();
    assert_eq!(inspect(&mut restored, media), inspect(&mut engine, media));
}

#[test]
fn moves_reparent_columns_but_reject_cycles_and_foreign_groups_atomically() {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    let column = columns(&mut engine, media, 1)[0];
    let outer = group(&mut engine, media, None);
    let inner = group(&mut engine, media, Some(outer));
    edit(
        &mut engine,
        media,
        MatrixEdit::MoveColumnItem {
            item: ColumnItemId::Column { id: column },
            parent: Some(inner),
            after: None,
        },
    );
    let before = inspect(&mut engine, media);
    assert_eq!(before.columns[0].parent, Some(inner));
    let history = engine.undo_len();
    let ack = invoke(
        &mut engine,
        MediaEdit::Matrix {
            media,
            edit: MatrixEdit::MoveColumnItem {
                item: ColumnItemId::Group { id: outer },
                parent: Some(inner),
                after: None,
            },
        },
    );
    assert!(!ack.success);
    assert_eq!(engine.undo_len(), history);
    assert_eq!(inspect(&mut engine, media), before);
    let other = create(&mut engine, MediaKind::Matrix).media;
    let foreign = group(&mut engine, other, None);
    let ack = invoke(
        &mut engine,
        MediaEdit::Matrix {
            media,
            edit: MatrixEdit::MoveColumnItem {
                item: ColumnItemId::Column { id: column },
                parent: Some(foreign),
                after: None,
            },
        },
    );
    assert!(!ack.success);
    assert_eq!(inspect(&mut engine, media), before);
}

#[test]
fn group_copy_remaps_sparse_cells_and_delete_undo_restores_exact_identity() {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    let source = create(&mut engine, MediaKind::SolidColor);
    let row = layer(layers(&mut engine, media, None, 1)[0]);
    let outer = group(&mut engine, media, None);
    let inner = group(&mut engine, media, Some(outer));
    let columns = nested_columns(&mut engine, media, inner, 2);
    edit(
        &mut engine,
        media,
        MatrixEdit::SetCells {
            assignments: vec![CellAssignment {
                layer: row,
                column: columns[1],
                source: Some(source),
            }],
        },
    );
    let original = inspect(&mut engine, media);
    let history = engine.undo_len();
    let MediaEditResult::MatrixColumnGroupCreated { group: copy } =
        edit(&mut engine, media, MatrixEdit::DuplicateColumnGroup { group: outer })
    else {
        panic!()
    };
    assert_eq!(engine.undo_len(), history + 1);
    let copied = inspect(&mut engine, media);
    assert_eq!(copied.column_groups.len(), 4);
    assert_eq!(copied.columns.len(), 4);
    assert_eq!(copied.cells.len(), 2);
    let new_columns: Vec<_> = copied.columns.iter().filter(|c| !columns.contains(&c.id)).collect();
    assert_eq!(new_columns.len(), 2);
    let new_cell = copied.cells.iter().find(|c| c.id != original.cells[0].id).unwrap();
    assert_eq!(new_cell.definition.column, new_columns[1].id);
    assert_ne!(new_cell.definition.source, original.cells[0].definition.source);
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    assert_eq!(inspect(&mut loaded, media), copied);
    edit(&mut engine, media, MatrixEdit::RemoveColumnGroup { group: copy });
    assert_eq!(inspect(&mut engine, media), original);
    assert!(engine.undo().unwrap());
    assert_eq!(inspect(&mut engine, media), copied);
}

#[test]
fn large_group_stays_sparse_and_owns_typed_normal_application_settings() {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    let group = group(&mut engine, media, None);
    let history = engine.undo_len();
    let snapshots = engine.tick_stats().lifecycle_snapshot_builds;
    nested_columns(&mut engine, media, group, 1024);
    assert_eq!(engine.undo_len(), history + 1);
    assert!(engine.tick_stats().lifecycle_snapshot_builds - snapshots <= 8);
    let snapshot = inspect(&mut engine, media);
    assert_eq!(snapshot.columns.len(), 1024);
    assert!(snapshot.cells.is_empty());
    assert_eq!(snapshot.column_groups[0].column_count, 1024);
    let application = ApplicationId(group.0);
    let mut definition = crate::applications::inspect(&engine, application).unwrap().definition;
    assert_eq!(definition.window, ApplicationWindow::Normal);
    definition.window = ApplicationWindow::Hidden;
    apply(
        &mut engine,
        MediaEdit::Applications {
            edit: crate::applications::protocol::ApplicationEdit::Configure {
                application,
                definition: definition.clone(),
            },
        },
    );
    let encoded = to_sparse_project_json_pretty(&engine).unwrap();
    let restored = from_sparse_project_json::<AppNode>(&encoded).unwrap();
    assert_eq!(
        crate::applications::inspect(&restored, application).unwrap().definition,
        definition
    );
    let snapshots = engine.tick_stats().lifecycle_snapshot_builds;
    let history = engine.undo_len();
    edit(&mut engine, media, MatrixEdit::UngroupColumns { group });
    assert_eq!(engine.undo_len(), history + 1);
    assert!(engine.tick_stats().lifecycle_snapshot_builds - snapshots <= 8);
    let ungrouped = inspect(&mut engine, media);
    assert!(ungrouped.column_groups.is_empty());
    assert_eq!(ungrouped.columns.len(), 1024);
    assert!(ungrouped.cells.is_empty());
    assert!(engine.undo().unwrap());
    assert_eq!(inspect(&mut engine, media), snapshot);
    assert_eq!(
        crate::applications::inspect(&engine, application).unwrap().definition,
        definition
    );
}
