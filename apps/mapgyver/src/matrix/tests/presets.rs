use super::*;
use golden_core::node::{Node, NodeUuid};

fn preset(engine: &mut AppEngine, media: MediaId) -> NodeUuid {
    let MediaEditResult::MatrixPresetCreated { preset } = edit(
        engine,
        media,
        MatrixEdit::Preset {
            edit: MatrixPresetEdit::Capture {
                label: Some("Show".into()),
            },
        },
    ) else {
        panic!()
    };
    preset
}
fn action(engine: &mut AppEngine, media: MediaId, operation: MatrixPresetEdit) {
    edit(engine, media, MatrixEdit::Preset { edit: operation });
}
fn enabled(engine: &AppEngine, uuid: NodeUuid) -> bool {
    engine
        .nodes
        .get(engine.node_id_by_uuid(uuid).unwrap())
        .unwrap()
        .node_data()
        .meta
        .enabled
}
fn disable(engine: &mut AppEngine, uuid: NodeUuid) {
    let node = engine.node_id_by_uuid(uuid).unwrap();
    let ack = apply_project_intent(
        engine,
        UiEditIntent::PatchMeta {
            node,
            patch: golden_core::ui_sync::NodeMetaPatch {
                enabled: Some(false),
                ..Default::default()
            },
        },
    );
    assert!(ack.success, "{ack:?}");
}

#[test]
fn capture_recall_restores_order_and_enabled_atomically_without_changing_cells_or_playback() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix).media;
    let rows = layers(&mut engine, matrix, None, 2);
    let cols = columns(&mut engine, matrix, 2);
    let source = create(&mut engine, MediaKind::SolidColor);
    edit(
        &mut engine,
        matrix,
        MatrixEdit::SetCells {
            assignments: vec![CellAssignment {
                layer: layer(rows[0]),
                column: cols[0],
                source: Some(source),
            }],
        },
    );
    edit(
        &mut engine,
        matrix,
        MatrixEdit::LaunchColumn {
            column: cols[0],
            mode: LaunchMode::Cut,
        },
    );
    let original = inspect(&mut engine, matrix);
    let preset = preset(&mut engine, matrix);
    edit(
        &mut engine,
        matrix,
        MatrixEdit::MoveItem {
            item: rows[1],
            parent: None,
            after: None,
        },
    );
    edit(
        &mut engine,
        matrix,
        MatrixEdit::MoveColumnItem {
            item: ColumnItemId::Column { id: cols[1] },
            parent: None,
            after: None,
        },
    );
    disable(&mut engine, layer(rows[0]).0);
    disable(&mut engine, cols[0].0);
    let changed = inspect(&mut engine, matrix);
    let playback = serde_json::to_value(crate::matrix::playback::inspect(&engine, matrix).unwrap()).unwrap();
    let history = engine.undo_len();
    action(&mut engine, matrix, MatrixPresetEdit::Recall { preset });
    assert_eq!(engine.undo_len(), history + 1);
    assert_eq!(inspect(&mut engine, matrix), original);
    assert!(enabled(&engine, layer(rows[0]).0));
    assert!(enabled(&engine, cols[0].0));
    assert_eq!(
        serde_json::to_value(crate::matrix::playback::inspect(&engine, matrix).unwrap()).unwrap(),
        playback
    );
    assert!(engine.undo().unwrap());
    assert_eq!(inspect(&mut engine, matrix), changed);
    assert!(!enabled(&engine, cols[0].0));
    assert!(engine.redo().unwrap());
    assert_eq!(inspect(&mut engine, matrix), original);
    let encoded = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&encoded).unwrap();
    loaded.apply_edits().unwrap();
    assert_eq!(
        crate::matrix::presets::summaries(&loaded, matrix).unwrap()[0].id,
        preset
    );
    action(&mut loaded, matrix, MatrixPresetEdit::Recall { preset });
    assert_eq!(inspect(&mut loaded, matrix), original);
}

#[test]
fn update_rename_delete_and_foreign_matrix_rejection_preserve_grouped_history() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix).media;
    let row = layers(&mut engine, matrix, None, 1)[0];
    let preset = preset(&mut engine, matrix);
    disable(&mut engine, layer(row).0);
    let history = engine.undo_len();
    action(&mut engine, matrix, MatrixPresetEdit::Update { preset });
    assert_eq!(engine.undo_len(), history + 1);
    action(&mut engine, matrix, MatrixPresetEdit::Recall { preset });
    assert!(!enabled(&engine, layer(row).0));
    action(
        &mut engine,
        matrix,
        MatrixPresetEdit::Rename {
            preset,
            label: "Blackout".into(),
        },
    );
    assert_eq!(
        crate::matrix::presets::summaries(&engine, matrix).unwrap()[0].label,
        "Blackout"
    );
    let other = create(&mut engine, MediaKind::Matrix).media;
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let ack = invoke(
        &mut engine,
        MediaEdit::Matrix {
            media: other,
            edit: MatrixEdit::Preset {
                edit: MatrixPresetEdit::Recall { preset },
            },
        },
    );
    assert!(!ack.success);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    action(&mut engine, matrix, MatrixPresetEdit::Delete { preset });
    assert!(crate::matrix::presets::summaries(&engine, matrix).unwrap().is_empty());
    assert!(engine.undo().unwrap());
    assert_eq!(
        crate::matrix::presets::summaries(&engine, matrix).unwrap()[0].id,
        preset
    );
}

#[test]
fn recall_keeps_new_members_and_groups_in_place_and_skips_deleted_members() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix).media;
    let rows = layers(&mut engine, matrix, None, 3);
    let preset = preset(&mut engine, matrix);
    let added = layers(&mut engine, matrix, None, 1)[0];
    edit(
        &mut engine,
        matrix,
        MatrixEdit::MoveItem {
            item: added,
            parent: None,
            after: Some(rows[0]),
        },
    );
    edit(
        &mut engine,
        matrix,
        MatrixEdit::MoveItem {
            item: rows[2],
            parent: None,
            after: None,
        },
    );
    edit(
        &mut engine,
        matrix,
        MatrixEdit::MoveItem {
            item: added,
            parent: None,
            after: Some(rows[2]),
        },
    );
    edit(&mut engine, matrix, MatrixEdit::RemoveItem { item: rows[1] });
    let MediaEditResult::MatrixCreated { items: groups, .. } = edit(
        &mut engine,
        matrix,
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
    action(&mut engine, matrix, MatrixPresetEdit::Recall { preset });
    let ids: Vec<_> = inspect(&mut engine, matrix).items.iter().map(|item| item.id).collect();
    assert_eq!(ids, vec![rows[0], added, rows[2], groups[0]]);
    edit(
        &mut engine,
        matrix,
        MatrixEdit::MoveItem {
            item: rows[0],
            parent: Some(group),
            after: None,
        },
    );
    action(&mut engine, matrix, MatrixPresetEdit::Recall { preset });
    let current = inspect(&mut engine, matrix);
    assert_eq!(
        current.items.iter().find(|item| item.id == rows[0]).unwrap().parent,
        Some(group)
    );
    assert_eq!(current.items.iter().find(|item| item.id == added).unwrap().parent, None);
}

#[test]
fn matrix_copy_remaps_preset_members_to_the_copied_layers_and_columns() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix).media;
    let row = layers(&mut engine, matrix, None, 1)[0];
    let column = columns(&mut engine, matrix, 1)[0];
    preset(&mut engine, matrix);
    let source = engine.node_id_by_uuid(matrix.0).unwrap();
    let parent = engine.nodes.get(source).unwrap().node_data().parent.unwrap();
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::DuplicateNode {
            source,
            new_parent: parent,
            new_prev_sibling: Some(source),
            initial_params: vec![],
        },
    );
    assert!(ack.success, "{ack:?}");
    let copy = engine
        .nodes
        .iter()
        .find_map(|(_, node)| match node {
            AppNode::Media(node)
                if node.node_data().parent == Some(parent) && node.node_data().meta.uuid != matrix.0 =>
            {
                Some(MediaId(node.node_data().meta.uuid))
            }
            _ => None,
        })
        .unwrap();
    let copied = inspect(&mut engine, copy);
    let copied_preset = crate::matrix::presets::summaries(&engine, copy).unwrap()[0].id;
    let copied_row = layer(copied.items[0].id).0;
    let copied_column = copied.columns[0].id.0;
    disable(&mut engine, copied_row);
    disable(&mut engine, copied_column);
    action(&mut engine, copy, MatrixPresetEdit::Recall { preset: copied_preset });
    assert!(enabled(&engine, copied_row));
    assert!(enabled(&engine, copied_column));
    assert!(enabled(&engine, layer(row).0));
    assert!(enabled(&engine, column.0));
}

#[test]
fn member_pages_are_bounded_immutable_and_reject_invalid_or_foreign_cursors() {
    use golden_core::app::ProjectLifecycle;
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    layers(&mut engine, media, None, 70);
    columns(&mut engine, media, 70);
    let preset = preset(&mut engine, media);
    let operation = |after, count| MediaEdit::Matrix {
        media,
        edit: MatrixEdit::Preset {
            edit: MatrixPresetEdit::InspectMembers { preset, after, count },
        },
    };
    let history = engine.ui_history_state();
    let result = AppNode::query_app_operation(
        &engine,
        MEDIA_NAMESPACE,
        MEDIA_OPERATION,
        &serde_json::to_value(operation(None, 128)).unwrap(),
    )
    .unwrap()
    .unwrap();
    let MediaEditResult::MatrixPresetMembers { page: first } = serde_json::from_value(result).unwrap() else {
        panic!()
    };
    assert_eq!(first.members.len(), 128);
    let MediaEditResult::MatrixPresetMembers { page: second } = apply(&mut engine, operation(first.next, 128)) else {
        panic!()
    };
    assert_eq!(second.members.len(), 12);
    assert!(second.next.is_none());
    assert!(
        !first
            .members
            .iter()
            .any(|member| second.members.iter().any(|other| other.id == member.id))
    );
    assert_eq!(engine.ui_history_state(), history);
    for count in [0, 129, u16::MAX] {
        assert!(!invoke(&mut engine, operation(None, count)).success);
    }
    let MediaEditResult::MatrixPresetCreated { preset: other } = edit(
        &mut engine,
        media,
        MatrixEdit::Preset {
            edit: MatrixPresetEdit::Capture { label: None },
        },
    ) else {
        panic!()
    };
    let ack = invoke(
        &mut engine,
        MediaEdit::Matrix {
            media,
            edit: MatrixEdit::Preset {
                edit: MatrixPresetEdit::InspectMembers {
                    preset: other,
                    after: first.next,
                    count: 32,
                },
            },
        },
    );
    assert!(!ack.success);
}

#[test]
fn recall_changes_active_and_outgoing_visibility_without_replacing_pinned_media_or_playback() {
    use super::cell_playback::{at, capture, time};
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix).media;
    let row = layer(layers(&mut engine, matrix, None, 1)[0]);
    let cols = columns(&mut engine, matrix, 2);
    for column in &cols {
        let source = create(&mut engine, MediaKind::SolidColor);
        edit(
            &mut engine,
            matrix,
            MatrixEdit::SetCells {
                assignments: vec![CellAssignment {
                    layer: row,
                    column: *column,
                    source: Some(source),
                }],
            },
        );
    }
    let visible = preset(&mut engine, matrix);
    disable(&mut engine, cols[0].0);
    let muted = preset(&mut engine, matrix);
    action(&mut engine, matrix, MatrixPresetEdit::Recall { preset: visible });
    edit(
        &mut engine,
        matrix,
        MatrixEdit::LaunchColumn {
            column: cols[0],
            mode: LaunchMode::Cut,
        },
    );
    let now = time();
    let pinned = capture(&engine, matrix, row, now).inputs[0].source;
    edit(
        &mut engine,
        matrix,
        MatrixEdit::LaunchColumn {
            column: cols[1],
            mode: LaunchMode::Configured,
        },
    );
    let middle = at(now, 125);
    let before = capture(&engine, matrix, row, middle);
    assert_eq!(before.inputs.len(), 2);
    let cells = inspect(&mut engine, matrix).cells;
    let playback = serde_json::to_value(crate::matrix::playback::inspect(&engine, matrix).unwrap()).unwrap();
    action(&mut engine, matrix, MatrixPresetEdit::Recall { preset: muted });
    let current = capture(&engine, matrix, row, middle);
    assert_eq!(current.inputs.len(), 1);
    assert_ne!(current.inputs[0].source, pinned);
    action(&mut engine, matrix, MatrixPresetEdit::Recall { preset: visible });
    assert_eq!(capture(&engine, matrix, row, middle).inputs.len(), 2);
    assert!(
        capture(&engine, matrix, row, middle)
            .inputs
            .iter()
            .any(|input| input.source == pinned)
    );
    assert_eq!(inspect(&mut engine, matrix).cells, cells);
    assert_eq!(
        serde_json::to_value(crate::matrix::playback::inspect(&engine, matrix).unwrap()).unwrap(),
        playback
    );
    disable(&mut engine, matrix.0);
    action(
        &mut engine,
        matrix,
        MatrixPresetEdit::Rename {
            preset: muted,
            label: "Edited while disabled".into(),
        },
    );
    action(&mut engine, matrix, MatrixPresetEdit::Update { preset: muted });
    assert_eq!(
        crate::matrix::presets::summaries(&engine, matrix).unwrap()[1].label,
        "Edited while disabled"
    );
    assert_eq!(inspect(&mut engine, matrix).cells, cells);
}

#[test]
fn million_address_sparse_matrix_capture_and_recall_scale_with_axes_without_materializing_cells() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix).media;
    let rows = layers(&mut engine, matrix, None, 1000);
    let cols = columns(&mut engine, matrix, 1000);
    let before = engine.nodes.len();
    let preset = preset(&mut engine, matrix);
    assert!(engine.nodes.len() - before < 8 * (rows.len() + cols.len()));
    edit(
        &mut engine,
        matrix,
        MatrixEdit::MoveItem {
            item: rows[999],
            parent: None,
            after: None,
        },
    );
    edit(
        &mut engine,
        matrix,
        MatrixEdit::ReorderColumn {
            column: cols[999],
            after: None,
        },
    );
    let history = engine.undo_len();
    action(&mut engine, matrix, MatrixPresetEdit::Recall { preset });
    assert_eq!(engine.undo_len(), history + 1);
    let current = inspect(&mut engine, matrix);
    assert!(current.cells.is_empty());
    assert_eq!(current.items.iter().map(|item| item.id).collect::<Vec<_>>(), rows);
    assert_eq!(current.columns.iter().map(|column| column.id).collect::<Vec<_>>(), cols);
}

#[test]
fn preset_limit_rejection_and_generic_structure_guard_preserve_the_existing_configuration() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix).media;
    let first = preset(&mut engine, matrix);
    for _ in 1..crate::matrix::presets::MAX_PRESETS {
        preset(&mut engine, matrix);
    }
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.ui_history_state();
    let ack = invoke(
        &mut engine,
        MediaEdit::Matrix {
            media: matrix,
            edit: MatrixEdit::Preset {
                edit: MatrixPresetEdit::Capture { label: None },
            },
        },
    );
    assert!(!ack.success);
    let source = engine.node_id_by_uuid(first).unwrap();
    let parent = engine.nodes.get(source).unwrap().node_data().parent.unwrap();
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::DuplicateNode {
            source,
            new_parent: parent,
            new_prev_sibling: Some(source),
            initial_params: vec![],
        },
    );
    assert!(
        !ack.success,
        "generic duplication must not bypass Matrix structural admission"
    );
    assert_eq!(engine.ui_history_state(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    assert_eq!(crate::matrix::presets::summaries(&engine, matrix).unwrap().len(), 128);
}
