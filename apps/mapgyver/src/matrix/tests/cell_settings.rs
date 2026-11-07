use super::cell_playback::{at, capture, time};
use super::*;
use golden_core::{
    node::Node,
    parameter::{ParamValue, ParameterEventBehaviour},
};
use golden_video::Extent2D;

fn parameter(engine: &AppEngine, owner: CellId, field: &str) -> golden_core::node::NodeId {
    let owner = engine.node_id_by_uuid(owner.0).unwrap();
    let folder = engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().parent == Some(owner) && crate::parameters::is_parameter_folder(*node))
        .unwrap()
        .0;
    engine
        .nodes
        .iter()
        .find(|(_, node)| {
            node.node_data().parent == Some(folder) && node.node_data().meta.decl_id.0 == format!("parameters/{field}")
        })
        .unwrap()
        .0
}
fn write(engine: &mut AppEngine, owner: CellId, field: &str, value: ParamValue) -> golden_core::ui_sync::UiAck {
    let node = parameter(engine, owner, field);
    apply_project_intent(
        engine,
        UiEditIntent::SetParam {
            node,
            value,
            behaviour: ParameterEventBehaviour::Append,
        },
    )
}

#[test]
fn optional_cell_size_is_a_typed_parameter_and_metadata_changes_invalidate_without_replacing_the_source() {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    let row = layer(layers(&mut engine, media, None, 1)[0]);
    let column = columns(&mut engine, media, 1)[0];
    let source = create(&mut engine, MediaKind::SolidColor);
    edit(
        &mut engine,
        media,
        MatrixEdit::SetCells {
            assignments: vec![CellAssignment {
                layer: row,
                column,
                source: Some(source),
            }],
        },
    );
    let cell = inspect(&mut engine, media).cells[0].id;
    assert_eq!(inspect(&mut engine, media).cells[0].definition.settings.size, None);
    let id = engine.node_id_by_uuid(cell.0).unwrap();
    let version = match engine.nodes.get(id).unwrap() {
        AppNode::MatrixCell(cell) => cell.source_version(),
        _ => panic!(),
    };
    assert!(write(&mut engine, cell, "size", ParamValue::Vec2(12.0, 8.0)).success);
    assert_eq!(inspect(&mut engine, media).cells[0].definition.settings.size, None);
    edit(
        &mut engine,
        media,
        MatrixEdit::LaunchColumn {
            column,
            mode: LaunchMode::Cut,
        },
    );
    let now = time();
    let output = inspect(&mut engine, media).items[0].channel;
    let before = crate::media::capture_channels_at(&engine, &[output], 16, now).unwrap();
    let node = parameter(&engine, cell, "size");
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::PatchMeta {
            node,
            patch: golden_core::ui_sync::NodeMetaPatch {
                enabled: Some(true),
                ..Default::default()
            },
        },
    );
    assert!(ack.success, "{ack:?}");
    assert!(!before.is_current());
    assert_eq!(
        capture(&engine, media, row, now).inputs[0].placement.unwrap().size,
        Some(Extent2D::new(12, 8).unwrap())
    );
    let AppNode::MatrixCell(cell_node) = engine.nodes.get(id).unwrap() else {
        panic!()
    };
    assert_eq!(cell_node.source_version(), version);
    assert!(engine.undo().unwrap());
    assert_eq!(inspect(&mut engine, media).cells[0].definition.settings.size, None);
    assert!(engine.redo().unwrap());
    assert_eq!(
        inspect(&mut engine, media).cells[0].definition.settings.size,
        Some(Extent2D::new(12, 8).unwrap())
    );
    edit(
        &mut engine,
        media,
        MatrixEdit::ConfigureCell {
            layer: row,
            column,
            settings: CellSettings::default(),
        },
    );
    assert_eq!(inspect(&mut engine, media).cells[0].definition.settings.size, None);
}

#[test]
fn cell_settings_materialize_only_the_address_and_preserve_atomic_values_history_copy_project_and_schema() {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    let row = layer(layers(&mut engine, media, None, 1)[0]);
    let column = columns(&mut engine, media, 2)[0];
    let settings = CellSettings {
        position: [2.5, -3.0],
        size: Some(Extent2D::new(41, 23).unwrap()),
        fit_mode: MediaFitMode::Fill,
        opacity: 0.25,
        enabled: false,
    };
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    edit(
        &mut engine,
        media,
        MatrixEdit::ConfigureCell {
            layer: row,
            column,
            settings,
        },
    );
    assert_eq!(engine.undo_len(), history + 1);
    let snapshot = inspect(&mut engine, media);
    assert_eq!(snapshot.cells.len(), 1);
    assert_eq!(snapshot.cells[0].definition.settings, settings);
    assert!(snapshot.cells[0].media.is_none());
    let cell = snapshot.cells[0].id;
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    assert_eq!(
        serde_json::to_value(inspect(&mut loaded, media)).unwrap(),
        serde_json::to_value(&snapshot).unwrap()
    );
    assert!(engine.undo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    assert!(engine.redo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    assert!(
        from_sparse_project_json::<AppNode>(&saved.replace("mapgyver.matrix.cell.v4", "mapgyver.matrix.cell.v2"))
            .is_err()
    );
    let mut file = engine.to_project_file_with(Node::project_encode_data).unwrap();
    let mut pending = vec![&mut file.root];
    while let Some(record) = pending.pop() {
        if record.node_type == crate::matrix::cell_node::CELL_TYPE {
            record.data = Some(serde_json::json!({"opacity": 0.1}));
            break;
        }
        pending.extend(record.children.iter_mut());
    }
    assert!(crate::media::validate_project_file(&file).is_err());
    assert!(from_sparse_project_json::<AppNode>(&serde_json::to_string(&file).unwrap()).is_err());
    edit(&mut engine, media, MatrixEdit::DuplicateColumn { column });
    let copied = inspect(&mut engine, media);
    assert_eq!(copied.cells.len(), 2);
    assert!(copied.cells.iter().all(|cell| cell.definition.settings == settings));
    assert_ne!(copied.cells[0].id, copied.cells[1].id);
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    for (field, value) in [
        ("position", ParamValue::Vec2(1e40, 0.0)),
        ("opacity", ParamValue::Float(-0.1)),
        ("opacity", ParamValue::Float(1.1)),
        ("size", ParamValue::Vec2(1.5, 2.0)),
        ("size", ParamValue::Vec2(0.0, 2.0)),
        ("size", ParamValue::Vec2(16385.0, 2.0)),
        ("fit_mode", ParamValue::Enum("Unknown".into())),
    ] {
        assert!(!write(&mut engine, cell, field, value).success);
        assert_eq!(engine.undo_len(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    }
    assert!(write(&mut engine, cell, "position", ParamValue::Vec2(-4.0, 5.0)).success);
    assert!(write(&mut engine, cell, "opacity", ParamValue::Float(0.5)).success);
    assert!(write(&mut engine, cell, "enabled", ParamValue::Bool(true)).success);
    assert_eq!(
        inspect(&mut engine, media)
            .cells
            .iter()
            .find(|entry| entry.id == cell)
            .unwrap()
            .definition
            .settings,
        CellSettings {
            position: [-4.0, 5.0],
            opacity: 0.5,
            enabled: true,
            ..settings
        }
    );
    // An atomic invalid edit cannot materialize another otherwise sparse address.
    let snapshot = inspect(&mut engine, media);
    let untouched = snapshot
        .columns
        .iter()
        .find(|column| !snapshot.cells.iter().any(|cell| cell.definition.column == column.id))
        .unwrap()
        .id;
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let ack = invoke(
        &mut engine,
        MediaEdit::Matrix {
            media,
            edit: MatrixEdit::ConfigureCell {
                layer: row,
                column: untouched,
                settings: CellSettings {
                    opacity: -1.0,
                    ..Default::default()
                },
            },
        },
    );
    assert!(!ack.success);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
}

#[test]
fn cell_geometry_opacity_edit_live_while_disabled_destination_holds_and_deleted_outgoing_settings_remain_bound() {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    let row = layer(layers(&mut engine, media, None, 1)[0]);
    let columns = columns(&mut engine, media, 2);
    let sources = [
        create(&mut engine, MediaKind::SolidColor),
        create(&mut engine, MediaKind::SolidColor),
    ];
    for (column, source) in columns.iter().zip(sources) {
        edit(
            &mut engine,
            media,
            MatrixEdit::SetCells {
                assignments: vec![CellAssignment {
                    layer: row,
                    column: *column,
                    source: Some(source),
                }],
            },
        );
    }
    let first = inspect(&mut engine, media)
        .cells
        .iter()
        .find(|cell| cell.definition.column == columns[0])
        .unwrap()
        .id;
    let original = CellSettings {
        position: [3.0, 4.0],
        opacity: 0.25,
        ..Default::default()
    };
    edit(
        &mut engine,
        media,
        MatrixEdit::ConfigureCell {
            layer: row,
            column: columns[0],
            settings: original,
        },
    );
    edit(
        &mut engine,
        media,
        MatrixEdit::LaunchColumn {
            column: columns[0],
            mode: LaunchMode::Cut,
        },
    );
    let now = time();
    let current = capture(&engine, media, row, now);
    assert_eq!(current.inputs.len(), 1);
    assert_eq!(current.inputs[0].placement.unwrap().position, original.position);
    assert_eq!(current.inputs[0].placement.unwrap().size, None);
    assert_eq!(current.inputs[0].opacity, original.opacity);
    assert!(write(&mut engine, first, "position", ParamValue::Vec2(6.0, -2.0)).success);
    assert!(write(&mut engine, first, "opacity", ParamValue::Float(0.5)).success);
    let current = capture(&engine, media, row, now);
    assert_eq!(current.inputs[0].placement.unwrap().position, [6.0, -2.0]);
    assert_eq!(current.inputs[0].opacity, 0.5);
    let held_source = current.inputs[0].source;
    edit(
        &mut engine,
        media,
        MatrixEdit::ConfigureCell {
            layer: row,
            column: columns[1],
            settings: CellSettings {
                enabled: false,
                ..Default::default()
            },
        },
    );
    let before = super::super::playback::inspect(&engine, media).unwrap();
    edit(
        &mut engine,
        media,
        MatrixEdit::LaunchCell {
            layer: row,
            column: columns[1],
            mode: LaunchMode::Cut,
        },
    );
    let after = super::super::playback::inspect(&engine, media).unwrap();
    assert_eq!(
        serde_json::to_value(before).unwrap(),
        serde_json::to_value(after).unwrap()
    );
    edit(
        &mut engine,
        media,
        MatrixEdit::LaunchColumn {
            column: columns[1],
            mode: LaunchMode::Cut,
        },
    );
    let held = capture(&engine, media, row, now);
    assert_eq!(held.inputs[0].source, held_source);
    assert_eq!(held.inputs[0].placement.unwrap().position, [6.0, -2.0]);
    edit(
        &mut engine,
        media,
        MatrixEdit::ConfigureCell {
            layer: row,
            column: columns[1],
            settings: CellSettings {
                position: [-1.0, 2.0],
                opacity: 0.75,
                ..Default::default()
            },
        },
    );
    edit(
        &mut engine,
        media,
        MatrixEdit::LaunchColumn {
            column: columns[1],
            mode: LaunchMode::Configured,
        },
    );
    let outgoing = capture(&engine, media, row, now);
    assert_eq!(outgoing.inputs.len(), 2);
    edit(&mut engine, media, MatrixEdit::RemoveColumn { column: columns[0] });
    let middle = capture(&engine, media, row, at(now, 125));
    let old = middle.inputs.iter().find(|input| input.source == held_source).unwrap();
    assert_eq!(old.placement.unwrap().position, [6.0, -2.0]);
    assert_eq!(old.opacity, 0.5);
    let final_frame = capture(&engine, media, row, at(now, 250));
    assert_eq!(final_frame.inputs.len(), 1);
    assert_eq!(final_frame.inputs[0].placement.unwrap().position, [-1.0, 2.0]);
    assert_eq!(final_frame.inputs[0].opacity, 0.75);
}
