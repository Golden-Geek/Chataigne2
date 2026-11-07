use super::{
    layer_settings::{override_enabled, parameter, write},
    *,
};
use golden_core::{node::Node, parameter::ParamValue};

fn request(engine: &mut AppEngine, media: MediaId, row: LayerId, column: ColumnId) -> TransitionDefinition {
    edit(
        engine,
        media,
        MatrixEdit::LaunchCell {
            layer: row,
            column,
            mode: LaunchMode::Configured,
        },
    );
    let MediaEditResult::MatrixPlayback { snapshot } = super::super::playback::inspect(engine, media).unwrap() else {
        panic!()
    };
    snapshot
        .layers
        .iter()
        .find(|layer| layer.layer == row)
        .unwrap()
        .transition
}

#[test]
fn transition_metadata_preserves_defaults_cached_precedence_dormant_values_and_zero() {
    assert_eq!(TransitionCurve::default(), TransitionCurve::Smooth);
    assert_eq!(
        TransitionDefinition::default(),
        TransitionDefinition {
            duration_ms: 250,
            curve: TransitionCurve::Smooth
        }
    );
    assert!(serde_json::from_str::<TransitionCurve>("\"SmoothStep\"").is_err());
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    layers(&mut engine, media, None, 1);
    columns(&mut engine, media, 1);
    let initial = inspect(&mut engine, media);
    let row = layer(initial.items[0].id);
    let column = initial.columns[0].id;
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
    for owner in [column.0, cell.0] {
        let id = parameter(&engine, owner, "duration_ms");
        let meta = &engine.nodes.get(id).unwrap().node_data().meta;
        assert!(meta.can_be_disabled);
        assert!(!meta.enabled);
        assert_eq!(meta.label, "Transition Time");
        assert!(write(&mut engine, owner, "duration_ms", ParamValue::Float(99.0)).success);
    }
    assert!(
        !to_sparse_project_json_pretty(&engine)
            .unwrap()
            .contains("transition_override")
    );
    assert_eq!(
        request(&mut engine, media, row, column),
        TransitionDefinition::default()
    );
    assert!(write(&mut engine, row.0, "duration_ms", ParamValue::Float(700.0)).success);
    assert_eq!(request(&mut engine, media, row, column).duration_ms, 700);
    // Metadata changes invalidate the already-bound authored index, even with no value change.
    assert!(override_enabled(&mut engine, column.0, true).success);
    assert_eq!(request(&mut engine, media, row, column).duration_ms, 99);
    assert!(write(&mut engine, cell.0, "duration_ms", ParamValue::Float(0.0)).success);
    assert!(override_enabled(&mut engine, cell.0, true).success);
    assert_eq!(request(&mut engine, media, row, column).duration_ms, 0);
    assert!(write(&mut engine, cell.0, "curve", ParamValue::Enum("Linear".into())).success);
    assert_eq!(request(&mut engine, media, row, column).curve, TransitionCurve::Linear);
    assert!(override_enabled(&mut engine, cell.0, false).success);
    assert_eq!(
        request(&mut engine, media, row, column),
        TransitionDefinition {
            duration_ms: 99,
            curve: TransitionCurve::Smooth
        }
    );
    assert!(override_enabled(&mut engine, column.0, false).success);
    assert_eq!(
        request(&mut engine, media, row, column),
        TransitionDefinition {
            duration_ms: 700,
            curve: TransitionCurve::Smooth
        }
    );
    assert!(override_enabled(&mut engine, cell.0, true).success);
    assert_eq!(
        request(&mut engine, media, row, column),
        TransitionDefinition {
            duration_ms: 0,
            curve: TransitionCurve::Linear
        }
    );
}

#[test]
fn transition_metadata_configure_copy_history_and_project_preserve_one_authority() {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    layers(&mut engine, media, None, 1);
    columns(&mut engine, media, 1);
    let initial = inspect(&mut engine, media);
    let row = layer(initial.items[0].id);
    let column = initial.columns[0].id;
    let definition = ColumnDefinition {
        transition: Some(TransitionDefinition {
            duration_ms: 333,
            curve: TransitionCurve::Linear,
        }),
    };
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    edit(
        &mut engine,
        media,
        MatrixEdit::ConfigureColumn {
            column,
            definition: definition.clone(),
        },
    );
    assert_eq!(engine.undo_len(), history + 1);
    assert_eq!(inspect(&mut engine, media).columns[0].definition, definition);
    assert!(engine.undo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    assert!(engine.redo().unwrap());
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
    assert!(write(&mut engine, cell.0, "duration_ms", ParamValue::Float(0.0)).success);
    assert!(override_enabled(&mut engine, cell.0, true).success);
    edit(&mut engine, media, MatrixEdit::DuplicateColumn { column });
    let copied = inspect(&mut engine, media);
    assert_eq!(
        copied
            .columns
            .iter()
            .filter(|column| column.definition == definition)
            .count(),
        2
    );
    assert_eq!(
        copied
            .cells
            .iter()
            .filter(|cell| cell.definition.transition
                == Some(TransitionDefinition {
                    duration_ms: 0,
                    curve: TransitionCurve::Smooth
                }))
            .count(),
        2
    );
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    assert_eq!(inspect(&mut loaded, media), copied);
    for (current, obsolete) in [
        ("mapgyver.matrix.column.v3", "mapgyver.matrix.column.v2"),
        ("mapgyver.matrix.cell.v4", "mapgyver.matrix.cell.v3"),
    ] {
        assert!(from_sparse_project_json::<AppNode>(&saved.replace(current, obsolete)).is_err());
    }
    edit(
        &mut engine,
        media,
        MatrixEdit::ConfigureColumn {
            column,
            definition: ColumnDefinition::default(),
        },
    );
    assert_eq!(inspect(&mut engine, media).columns[0].definition.transition, None);
    let duration = parameter(&engine, column.0, "duration_ms");
    assert_eq!(
        engine
            .nodes
            .get(duration)
            .unwrap()
            .engine_param_snapshot()
            .unwrap()
            .value,
        ParamValue::Float(333.0)
    );
    assert!(engine.undo().unwrap());
    assert_eq!(inspect(&mut engine, media).columns[0].definition, definition);
}
