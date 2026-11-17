use super::*;
use golden_core::node::{Node, NodeMetaPatch, PresentationHint};

#[test]
fn matrix_custom_colors_are_editable_and_survive_history_nested_copy_and_reload() {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    let MediaEditResult::MatrixCreated { items, .. } = edit(
        &mut engine,
        media,
        MatrixEdit::AddGroup {
            parent: None,
            label: None,
        },
    ) else {
        panic!()
    };
    let MatrixItemId::Group { id: group } = items[0] else {
        panic!()
    };
    let row = layer(layers(&mut engine, media, Some(group), 1)[0]);
    let MediaEditResult::MatrixColumnGroupCreated { group: column_group } = edit(
        &mut engine,
        media,
        MatrixEdit::AddColumnGroup {
            parent: None,
            label: None,
        },
    ) else {
        panic!()
    };
    let MediaEditResult::MatrixCreated { columns, .. } = edit(
        &mut engine,
        media,
        MatrixEdit::AddColumns {
            parent: Some(column_group),
            count: 1,
        },
    ) else {
        panic!()
    };
    let column = columns[0];
    let MediaEditResult::MatrixMediaCreated { cell, .. } = edit(
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
    let presentation = PresentationHint {
        color: Some(
            serde_json::from_value(serde_json::json!({
                "r": 0.9, "g": 0.15, "b": 0.25, "a": 1.0,
            }))
            .unwrap(),
        ),
        ..Default::default()
    };
    let uuids = [group.0, row.0, column_group.0, column.0, cell.0];
    for uuid in uuids {
        let id = engine.node_id_by_uuid(uuid).unwrap();
        let before = engine.nodes.get(id).unwrap().node_data().meta.presentation.clone();
        assert!(
            engine
                .nodes
                .get(id)
                .unwrap()
                .node_data()
                .meta
                .user_permissions
                .can_edit_color
        );
        let history = engine.undo_len();
        let ack = apply_project_intent(
            &mut engine,
            UiEditIntent::PatchMeta {
                node: id,
                patch: NodeMetaPatch {
                    presentation: Some(presentation.clone()),
                    ..Default::default()
                }
                .into(),
            },
        );
        assert!(ack.success, "{ack:?}");
        assert_eq!(engine.undo_len(), history + 1);
        assert_eq!(
            engine.nodes.get(id).unwrap().node_data().meta.presentation,
            presentation
        );
        assert!(engine.undo().unwrap());
        assert_eq!(engine.nodes.get(id).unwrap().node_data().meta.presentation, before);
        assert!(engine.redo().unwrap());
    }
    edit(
        &mut engine,
        media,
        MatrixEdit::DuplicateItem {
            item: MatrixItemId::Group { id: group },
        },
    );
    edit(
        &mut engine,
        media,
        MatrixEdit::DuplicateColumnGroup { group: column_group },
    );
    let snapshot = inspect(&mut engine, media);
    let ids: Vec<_> = snapshot
        .items
        .iter()
        .map(|item| match item.id {
            MatrixItemId::Layer { id } => id.0,
            MatrixItemId::Group { id } => id.0,
        })
        .chain(snapshot.columns.iter().map(|item| item.id.0))
        .chain(snapshot.column_groups.iter().map(|item| item.id.0))
        .chain(snapshot.cells.iter().map(|item| item.id.0))
        .collect();
    assert!(ids.len() > uuids.len(), "copies must be independently materialized");
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    loaded.apply_edits().unwrap();
    for uuid in ids {
        for document in [&engine, &loaded] {
            let node = document.nodes.get(document.node_id_by_uuid(uuid).unwrap()).unwrap();
            assert_eq!(node.node_data().meta.presentation.color, presentation.color);
            assert!(node.node_data().meta.user_permissions.can_edit_color);
        }
    }
}
