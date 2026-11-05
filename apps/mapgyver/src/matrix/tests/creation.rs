use super::*;

fn contextual(
    engine: &mut AppEngine,
    media: MediaId,
    item: MatrixAxisItemKind,
    context: Option<golden_core::model::NodeUuid>,
) -> MediaEditResult {
    edit(engine, media, MatrixEdit::CreateAxisItem { item, context })
}

#[test]
fn contextual_creation_uses_committed_hierarchy_for_groups_columns_layers_and_owned_media() {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    let MediaEditResult::MatrixColumnGroupCreated { group } =
        contextual(&mut engine, media, MatrixAxisItemKind::ColumnGroup, None)
    else {
        panic!()
    };
    let MediaEditResult::MatrixCreated { columns, .. } =
        contextual(&mut engine, media, MatrixAxisItemKind::Column, Some(group.0))
    else {
        panic!()
    };
    let column = columns[0];
    let MediaEditResult::MatrixCreated { items, .. } =
        contextual(&mut engine, media, MatrixAxisItemKind::LayerGroup, None)
    else {
        panic!()
    };
    let MatrixItemId::Group { id: row_group } = items[0] else {
        panic!()
    };
    let MediaEditResult::MatrixCreated { items, .. } =
        contextual(&mut engine, media, MatrixAxisItemKind::Layer, Some(row_group.0))
    else {
        panic!()
    };
    let row = layer(items[0]);
    let MediaEditResult::MatrixMediaCreated { media: child, .. } = edit(
        &mut engine,
        media,
        MatrixEdit::CreateCellMedia {
            layer: row,
            column,
            media_kind: MediaKind::SolidColor,
        },
    ) else {
        panic!()
    };
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    let MediaEditResult::MatrixCreated { columns, .. } =
        contextual(&mut engine, media, MatrixAxisItemKind::Column, Some(child.0))
    else {
        panic!()
    };
    assert_eq!(engine.undo_len(), history + 1);
    let snapshot = inspect(&mut engine, media);
    assert_eq!(
        snapshot
            .columns
            .iter()
            .find(|entry| entry.id == columns[0])
            .unwrap()
            .parent,
        Some(group)
    );
    assert!(engine.undo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    let MediaEditResult::MatrixCreated { items, .. } =
        contextual(&mut engine, media, MatrixAxisItemKind::Layer, Some(child.0))
    else {
        panic!()
    };
    let snapshot = inspect(&mut engine, media);
    assert_eq!(
        snapshot.items.iter().find(|entry| entry.id == items[0]).unwrap().parent,
        Some(row_group)
    );
    let MediaEditResult::MatrixColumnGroupCreated { group: nested } =
        contextual(&mut engine, media, MatrixAxisItemKind::ColumnGroup, Some(column.0))
    else {
        panic!()
    };
    assert_eq!(
        inspect(&mut engine, media)
            .column_groups
            .iter()
            .find(|entry| entry.id == nested)
            .unwrap()
            .parent,
        Some(group)
    );
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    assert_eq!(inspect(&mut loaded, media), inspect(&mut engine, media));
}

#[test]
fn unrelated_live_selection_uses_root_but_a_removed_context_rejects_without_mutation() {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    let unrelated = create(&mut engine, MediaKind::SolidColor);
    let MediaEditResult::MatrixCreated { columns, .. } =
        contextual(&mut engine, media, MatrixAxisItemKind::Column, Some(unrelated.media.0))
    else {
        panic!()
    };
    assert_eq!(inspect(&mut engine, media).columns[0].id, columns[0]);
    assert_eq!(inspect(&mut engine, media).columns[0].parent, None);
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    let ack = invoke(
        &mut engine,
        MediaEdit::Matrix {
            media,
            edit: MatrixEdit::CreateAxisItem {
                item: MatrixAxisItemKind::Layer,
                context: Some(golden_core::model::NodeUuid(uuid::Uuid::new_v4())),
            },
        },
    );
    assert!(!ack.success);
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
}
