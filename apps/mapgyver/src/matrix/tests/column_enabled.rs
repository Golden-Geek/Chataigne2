use super::{
    cell_playback::{at, capture, time},
    *,
};
use golden_core::node::{Node, NodeUuid};

pub(super) fn enabled(engine: &mut AppEngine, uuid: NodeUuid, value: bool) {
    let node = engine.node_id_by_uuid(uuid).unwrap();
    let ack = apply_project_intent(
        engine,
        UiEditIntent::PatchMeta {
            node,
            patch: golden_core::ui_sync::NodeMetaPatch {
                enabled: Some(value),
                ..Default::default()
            },
        },
    );
    assert!(ack.success, "{ack:?}");
}

fn assign(engine: &mut AppEngine, media: MediaId, row: LayerId, column: ColumnId, source: ChannelReference) {
    edit(
        engine,
        media,
        MatrixEdit::SetCells {
            assignments: vec![CellAssignment {
                layer: row,
                column,
                source: Some(source),
            }],
        },
    );
}

#[test]
fn column_visibility_mutes_existing_bindings_without_resetting_playback_and_survives_history_copy_reload() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
    let column = columns(&mut engine, matrix.media, 1)[0];
    let source = create(&mut engine, MediaKind::SolidColor);
    assign(&mut engine, matrix.media, row, column, source);
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::LaunchColumn {
            column,
            mode: LaunchMode::Cut,
        },
    );
    let now = time();
    let original = capture(&engine, matrix.media, row, now).inputs[0].source;
    let before = inspect(&mut engine, matrix.media);
    let playback = serde_json::to_value(super::super::playback::inspect(&engine, matrix.media).unwrap()).unwrap();
    enabled(&mut engine, column.0, false);
    assert!(capture(&engine, matrix.media, row, now).inputs.is_empty());
    assert_eq!(
        serde_json::to_value(super::super::playback::inspect(&engine, matrix.media).unwrap()).unwrap(),
        playback
    );
    assert_eq!(
        serde_json::to_value(inspect(&mut engine, matrix.media).cells).unwrap(),
        serde_json::to_value(&before.cells).unwrap()
    );
    assert!(engine.undo().unwrap());
    assert_eq!(capture(&engine, matrix.media, row, now).inputs[0].source, original);
    assert!(engine.redo().unwrap());
    assert!(capture(&engine, matrix.media, row, now).inputs.is_empty());
    edit(&mut engine, matrix.media, MatrixEdit::DuplicateColumn { column });
    let copied = inspect(&mut engine, matrix.media)
        .columns
        .into_iter()
        .find(|entry| entry.id != column)
        .unwrap()
        .id;
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    loaded.apply_edits().unwrap();
    for uuid in [column.0, copied.0] {
        assert!(
            !loaded
                .nodes
                .get(loaded.node_id_by_uuid(uuid).unwrap())
                .unwrap()
                .node_data()
                .meta
                .enabled
        );
    }
    enabled(&mut engine, column.0, true);
    assert_eq!(capture(&engine, matrix.media, row, now).inputs[0].source, original);
}

#[test]
fn disabled_column_group_mutes_columns_but_keeps_cells_editable() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
    let MediaEditResult::MatrixColumnGroupCreated { group } = edit(
        &mut engine,
        matrix.media,
        MatrixEdit::AddColumnGroup {
            parent: None,
            label: None,
        },
    ) else {
        panic!()
    };
    let MediaEditResult::MatrixCreated { columns, .. } = edit(
        &mut engine,
        matrix.media,
        MatrixEdit::AddColumns {
            parent: Some(group),
            count: 1,
        },
    ) else {
        panic!()
    };
    let column = columns[0];
    let source = create(&mut engine, MediaKind::SolidColor);
    assign(&mut engine, matrix.media, row, column, source);
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::LaunchColumn {
            column,
            mode: LaunchMode::Cut,
        },
    );
    let now = time();
    assert_eq!(capture(&engine, matrix.media, row, now).inputs.len(), 1);
    enabled(&mut engine, group.0, false);
    assert!(capture(&engine, matrix.media, row, now).inputs.is_empty());
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::ConfigureCell {
            layer: row,
            column,
            settings: CellSettings {
                opacity: 0.4,
                ..Default::default()
            },
        },
    );
    assert!(capture(&engine, matrix.media, row, now).inputs.is_empty());
    assert!(
        engine
            .nodes
            .get(engine.node_id_by_uuid(column.0).unwrap())
            .unwrap()
            .node_data()
            .meta
            .enabled
    );
    enabled(&mut engine, group.0, true);
    assert_eq!(capture(&engine, matrix.media, row, now).inputs[0].opacity, 0.4);
}

#[test]
fn muted_outgoing_column_does_not_resurrect_replaced_or_deleted_cell_content() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
    let columns = columns(&mut engine, matrix.media, 2);
    let sources = [
        create(&mut engine, MediaKind::SolidColor),
        create(&mut engine, MediaKind::SolidColor),
        create(&mut engine, MediaKind::SolidColor),
    ];
    assign(&mut engine, matrix.media, row, columns[0], sources[0]);
    assign(&mut engine, matrix.media, row, columns[1], sources[1]);
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::LaunchColumn {
            column: columns[0],
            mode: LaunchMode::Cut,
        },
    );
    let now = time();
    let pinned = capture(&engine, matrix.media, row, now).inputs[0].source;
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::LaunchColumn {
            column: columns[1],
            mode: LaunchMode::Configured,
        },
    );
    assert_eq!(capture(&engine, matrix.media, row, now).inputs.len(), 2);
    assign(&mut engine, matrix.media, row, columns[0], sources[2]);
    enabled(&mut engine, columns[0].0, false);
    let middle = at(now, 125);
    let muted = capture(&engine, matrix.media, row, middle);
    assert_eq!(muted.inputs.len(), 1);
    assert_ne!(muted.inputs[0].source, pinned);
    enabled(&mut engine, columns[0].0, true);
    let resumed = capture(&engine, matrix.media, row, middle);
    assert_eq!(resumed.inputs.len(), 2);
    assert!(resumed.inputs.iter().any(|input| input.source == pinned));
    enabled(&mut engine, columns[0].0, false);
    assert_eq!(capture(&engine, matrix.media, row, middle).inputs.len(), 1);
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::RemoveColumn { column: columns[0] },
    );
    assert_eq!(capture(&engine, matrix.media, row, middle).inputs.len(), 1);
}
