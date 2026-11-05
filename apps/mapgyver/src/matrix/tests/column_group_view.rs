use super::*;

fn group(engine: &mut AppEngine, media: MediaId, parent: Option<ColumnGroupId>) -> ColumnGroupId {
    let MediaEditResult::MatrixColumnGroupCreated { group } =
        edit(engine, media, MatrixEdit::AddColumnGroup { parent, label: None })
    else {
        panic!()
    };
    group
}
fn window(
    engine: &mut AppEngine,
    media: MediaId,
    start: u32,
    count: u16,
    selected: Option<ColumnId>,
) -> MatrixViewportSnapshot {
    let MediaEditResult::MatrixViewport { snapshot } = edit(
        engine,
        media,
        MatrixEdit::InspectViewport {
            viewport: MatrixViewport {
                row_start: 0,
                row_count: 8,
                axis_start: start,
                axis_count: count,
            },
            selection: selected.map(|column| MatrixSelection {
                item: None,
                column: Some(column),
            }),
        },
    ) else {
        panic!()
    };
    snapshot
}

#[test]
fn empty_groups_have_slots_without_columns_or_cells_and_offscreen_selection_uses_axis_position() {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    columns(&mut engine, media, 1);
    let empty = group(&mut engine, media, None);
    let outer = group(&mut engine, media, None);
    let MediaEditResult::MatrixCreated { columns: nested, .. } = edit(
        &mut engine,
        media,
        MatrixEdit::AddColumns {
            parent: Some(outer),
            count: 2,
        },
    ) else {
        panic!()
    };
    let inner = group(&mut engine, media, Some(outer));
    let last = columns(&mut engine, media, 1)[0];
    let page = window(&mut engine, media, 1, 4, Some(last));
    assert_eq!((page.column_total, page.axis_total, page.axis_depth), (4, 6, 2));
    assert_eq!(
        page.columns.iter().map(|c| (c.id, c.axis_index)).collect::<Vec<_>>(),
        [(nested[0], 2), (nested[1], 3)]
    );
    assert!(page.cells.is_empty());
    assert_eq!(page.column_groups.len(), 3);
    let group = |id| page.column_groups.iter().find(|g| g.id == id).unwrap();
    assert!(group(empty).empty);
    assert_eq!((group(empty).axis_start, group(empty).axis_count), (1, 1));
    assert_eq!(
        (
            group(outer).axis_start,
            group(outer).axis_count,
            group(outer).column_count
        ),
        (2, 3, 2)
    );
    assert_eq!((group(inner).axis_start, group(inner).depth), (4, 1));
    let selected = page.selected.unwrap();
    assert_eq!(selected.column_index, Some(5));
    assert_eq!(selected.column.unwrap().id, last);
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    assert_eq!(
        window(&mut loaded, media, 1, 4, Some(last)).column_groups,
        page.column_groups
    );
}

#[test]
fn thousands_of_empty_groups_are_paged_without_materializing_columns_or_rebuilding_snapshots() {
    use golden_core::edit::{Edit, NodeTree, UserItemTreeInsertion};
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    let root = super::super::document::Document::load(&engine, media).unwrap().root;
    engine.edits.push(Edit::AddUserItemTrees {
        items: (0..1000)
            .map(|index| {
                UserItemTreeInsertion::new(
                    root,
                    NodeTree::new(super::super::ColumnGroupNode::named(format!("Group {index}"))),
                )
            })
            .collect(),
    });
    engine.apply_edits().unwrap();
    let first = window(&mut engine, media, 0, 8, None);
    assert_eq!((first.column_total, first.axis_total), (0, 1000));
    assert_eq!(first.column_groups.len(), 8);
    let history = engine.undo_len();
    let snapshots = engine.tick_stats().lifecycle_snapshot_builds;
    let last = window(&mut engine, media, 992, 8, None);
    assert_eq!(last.column_groups.len(), 8);
    assert!(last.columns.is_empty() && last.cells.is_empty());
    assert_eq!(last.column_groups[0].label, "Group 992");
    assert_eq!(engine.undo_len(), history);
    assert_eq!(engine.tick_stats().lifecycle_snapshot_builds, snapshots);
}

#[test]
fn contextual_axis_drops_resolve_complete_siblings_and_reject_cycles_atomically() {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    let column = columns(&mut engine, media, 1)[0];
    let outer = group(&mut engine, media, None);
    let inner = group(&mut engine, media, Some(outer));
    let before = inspect(&mut engine, media);
    edit(
        &mut engine,
        media,
        MatrixEdit::DropColumnItem {
            item: ColumnItemId::Column { id: column },
            target: ColumnItemDropTarget::Inside { group: inner },
        },
    );
    assert_eq!(inspect(&mut engine, media).columns[0].parent, Some(inner));
    assert!(engine.undo().unwrap());
    assert_eq!(inspect(&mut engine, media), before);
    let history = engine.undo_len();
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::Matrix {
                media,
                edit: MatrixEdit::DropColumnItem {
                    item: ColumnItemId::Group { id: outer },
                    target: ColumnItemDropTarget::Inside { group: inner }
                }
            }
        )
        .success
    );
    assert_eq!(engine.undo_len(), history);
    assert_eq!(inspect(&mut engine, media), before);
    edit(
        &mut engine,
        media,
        MatrixEdit::DropColumnItem {
            item: ColumnItemId::Column { id: column },
            target: ColumnItemDropTarget::After {
                item: ColumnItemId::Group { id: outer },
            },
        },
    );
    let order = super::super::column_groups::siblings(&inspect(&mut engine, media), None);
    assert_eq!(
        order,
        [ColumnItemId::Group { id: outer }, ColumnItemId::Column { id: column }]
    );
}
