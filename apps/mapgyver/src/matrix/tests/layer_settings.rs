use super::cell_playback::{at, capture, time};
use super::*;
use golden_core::{
    node::{Node, NodeId},
    parameter::{ParamValue, ParameterEventBehaviour},
};
use golden_video::Extent2D;

fn parameter(engine: &AppEngine, owner: golden_core::node::NodeUuid, field: &str) -> NodeId {
    let owner = engine.node_id_by_uuid(owner).unwrap();
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
fn write(
    engine: &mut AppEngine,
    owner: golden_core::node::NodeUuid,
    field: &str,
    value: ParamValue,
) -> golden_core::ui_sync::UiAck {
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
fn layer_spatial_settings_are_atomic_typed_independent_of_final_canvas_and_preserved_by_copy_history_project() {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    apply(
        &mut engine,
        MediaEdit::ConfigureMediaResolution {
            media,
            size: SizePolicy::Fixed {
                extent: Extent2D::new(16, 8).unwrap(),
            },
        },
    );
    let item = layers(&mut engine, media, None, 1)[0];
    let row = layer(item);
    assert_eq!(
        inspect(&mut engine, media).items[0].layer.unwrap().size,
        Extent2D::new(16, 8).unwrap()
    );
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    let settings = LayerSettings {
        position: [2.5, -3.0],
        size: Extent2D::new(8, 4).unwrap(),
        duration_ms: 900,
    };
    edit(&mut engine, media, MatrixEdit::ConfigureLayer { layer: row, settings });
    assert_eq!(engine.undo_len(), history + 1);
    assert_eq!(inspect(&mut engine, media).items[0].layer, Some(settings));
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    assert_eq!(inspect(&mut loaded, media).items[0].layer, Some(settings));
    assert!(engine.undo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    assert!(engine.redo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    edit(&mut engine, media, MatrixEdit::DuplicateItem { item });
    assert!(
        inspect(&mut engine, media)
            .items
            .iter()
            .all(|item| item.layer == Some(settings))
    );
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    for (field, value) in [
        ("canvas_size", ParamValue::Vec2(4.5, 2.0)),
        ("canvas_size", ParamValue::Vec2(0.0, 2.0)),
        ("canvas_size", ParamValue::Vec2(16385.0, 2.0)),
        ("position", ParamValue::Vec2(1e40, 0.0)),
        ("duration_ms", ParamValue::Float(0.5)),
    ] {
        assert!(!write(&mut engine, row.0, field, value).success);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
        assert_eq!(engine.undo_len(), history);
    }
    assert!(write(&mut engine, row.0, "canvas_size", ParamValue::Vec2(12.0, 6.0)).success);
    assert_eq!(
        inspect(&mut engine, media)
            .items
            .iter()
            .find(|item| item.id == MatrixItemId::Layer { id: row })
            .unwrap()
            .layer
            .unwrap()
            .size,
        Extent2D::new(12, 6).unwrap()
    );
}

#[test]
fn cell_then_column_then_layer_duration_controls_independent_and_atomic_column_blends() {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    let rows = layers(&mut engine, media, None, 2)
        .into_iter()
        .map(layer)
        .collect::<Vec<_>>();
    let columns = columns(&mut engine, media, 2);
    let sources = [
        create(&mut engine, MediaKind::SolidColor),
        create(&mut engine, MediaKind::SolidColor),
    ];
    for (row, duration_ms) in [(rows[0], 100), (rows[1], 900)] {
        edit(
            &mut engine,
            media,
            MatrixEdit::ConfigureLayer {
                layer: row,
                settings: LayerSettings {
                    duration_ms,
                    ..Default::default()
                },
            },
        );
        for (column, source) in columns.iter().zip(sources) {
            edit(
                &mut engine,
                media,
                MatrixEdit::DropSource {
                    layer: row,
                    column: *column,
                    source: source.media.0,
                    copy: false,
                },
            );
        }
    }
    let now = time();
    edit(
        &mut engine,
        media,
        MatrixEdit::LaunchColumn {
            column: columns[0],
            mode: LaunchMode::Cut,
        },
    );
    for row in &rows {
        capture(&engine, media, *row, now);
    }
    edit(
        &mut engine,
        media,
        MatrixEdit::LaunchColumn {
            column: columns[1],
            mode: LaunchMode::Configured,
        },
    );
    let first = capture(&engine, media, rows[0], now);
    let second = capture(&engine, media, rows[1], now);
    for (captured, millis) in [(first, 50), (second, 450)] {
        assert_eq!(captured.inputs.len(), 2);
        assert!(
            captured
                .inputs
                .iter()
                .all(|input| (input.weight.unwrap().sample(at(now, millis)).get() - 0.5).abs() < 0.0001)
        );
    }
    assert_eq!(capture(&engine, media, rows[0], at(now, 100)).inputs.len(), 1);
    assert_eq!(capture(&engine, media, rows[1], at(now, 100)).inputs.len(), 2);
    edit(
        &mut engine,
        media,
        MatrixEdit::ConfigureColumn {
            column: columns[0],
            definition: ColumnDefinition {
                transition: Some(TransitionDefinition {
                    duration_ms: 500,
                    curve: TransitionCurve::SmoothStep,
                }),
            },
        },
    );
    let cell = crate::matrix::document::Document::load(&engine, media).unwrap().cells[&(rows[0], columns[0])];
    let owner = engine.nodes.get(cell).unwrap().node_data().meta.uuid;
    assert!(write(&mut engine, owner, "transition_override", ParamValue::Bool(true)).success);
    assert!(write(&mut engine, owner, "duration_ms", ParamValue::Float(17.0)).success);
    let MediaEditResult::MatrixLaunched { snapshot } = edit(
        &mut engine,
        media,
        MatrixEdit::LaunchCell {
            layer: rows[0],
            column: columns[0],
            mode: LaunchMode::Configured,
        },
    ) else {
        panic!()
    };
    assert_eq!(
        snapshot
            .layers
            .iter()
            .find(|request| request.layer == rows[0])
            .unwrap()
            .transition
            .duration_ms,
        17
    );
    let MediaEditResult::MatrixLaunched { snapshot } = edit(
        &mut engine,
        media,
        MatrixEdit::LaunchCell {
            layer: rows[1],
            column: columns[0],
            mode: LaunchMode::Configured,
        },
    ) else {
        panic!()
    };
    assert_eq!(
        snapshot
            .layers
            .iter()
            .find(|request| request.layer == rows[1])
            .unwrap()
            .transition
            .duration_ms,
        500
    );
    let MediaEditResult::MatrixLaunched { snapshot } = edit(
        &mut engine,
        media,
        MatrixEdit::LaunchCell {
            layer: rows[1],
            column: columns[1],
            mode: LaunchMode::Configured,
        },
    ) else {
        panic!()
    };
    assert_eq!(
        snapshot
            .layers
            .iter()
            .find(|request| request.layer == rows[1])
            .unwrap()
            .transition
            .duration_ms,
        900
    );
}
