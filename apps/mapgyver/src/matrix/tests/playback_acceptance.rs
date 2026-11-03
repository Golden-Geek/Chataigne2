use super::{
    cell_playback::{at, capture, time},
    layer_settings::write,
    *,
};
use golden_core::parameter::ParamValue;

#[test]
fn actual_three_layer_columns_apply_cell_column_layer_precedence_atomically_from_interrupted_shared_time() {
    let mut engine = fixture();
    let MediaEditResult::Created { reference } = apply(
        &mut engine,
        MediaEdit::CreateMedia {
            media_kind: MediaKind::Matrix,
            label: None,
        },
    ) else {
        panic!()
    };
    let initial = inspect(&mut engine, reference.media);
    let rows = initial.items.iter().map(|item| layer(item.id)).collect::<Vec<_>>();
    let columns = initial.columns.iter().map(|column| column.id).collect::<Vec<_>>();
    let sources = [
        create(&mut engine, MediaKind::SolidColor),
        create(&mut engine, MediaKind::SolidColor),
    ];
    let mut assignments = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        edit(
            &mut engine,
            reference.media,
            MatrixEdit::ConfigureLayer {
                layer: *row,
                settings: LayerSettings {
                    duration_ms: [100, 900, 700][index],
                    ..Default::default()
                },
            },
        );
        for (column, source) in columns.iter().zip(sources) {
            assignments.push(CellAssignment {
                layer: *row,
                column: *column,
                source: Some(source),
            });
        }
    }
    edit(&mut engine, reference.media, MatrixEdit::SetCells { assignments });
    edit(
        &mut engine,
        reference.media,
        MatrixEdit::ConfigureColumn {
            column: columns[1],
            definition: ColumnDefinition {
                transition: Some(TransitionDefinition {
                    duration_ms: 500,
                    curve: TransitionCurve::SmoothStep,
                }),
            },
        },
    );
    let cell = inspect(&mut engine, reference.media)
        .cells
        .iter()
        .find(|cell| cell.layer == rows[0] && cell.definition.column == columns[1])
        .unwrap()
        .id;
    for (field, value) in [
        ("transition_override", ParamValue::Bool(true)),
        ("duration_ms", ParamValue::Float(17.0)),
    ] {
        assert!(write(&mut engine, cell.0, field, value).success);
    }
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    edit(
        &mut engine,
        reference.media,
        MatrixEdit::LaunchColumn {
            column: columns[0],
            mode: LaunchMode::Cut,
        },
    );
    let now = time();
    let old = rows
        .iter()
        .map(|row| capture(&engine, reference.media, *row, now).inputs[0].source)
        .collect::<Vec<_>>();
    edit(
        &mut engine,
        reference.media,
        MatrixEdit::LaunchColumn {
            column: columns[1],
            mode: LaunchMode::Configured,
        },
    );
    let faded = rows
        .iter()
        .map(|row| capture(&engine, reference.media, *row, now))
        .collect::<Vec<_>>();
    for (index, captured) in faded.iter().enumerate() {
        assert_eq!(captured.inputs.len(), 2);
        for input in &captured.inputs {
            let (millis, incoming) = if index == 0 { (8, 8.0 / 17.0) } else { (125, 0.15625) };
            let expected = if input.source == old[index] {
                1.0 - incoming
            } else {
                incoming
            };
            assert!((input.weight.unwrap().sample(at(now, millis)).get() - expected).abs() < 0.0001);
        }
    }
    // Interrupt the shared Column fade after 200 ms. Every Layer must start its own
    // fallback duration from the actual mix at that same anchor, rather than restarting it.
    let interrupted_at = at(now, 200);
    for row in &rows {
        capture(&engine, reference.media, *row, interrupted_at);
    }
    edit(
        &mut engine,
        reference.media,
        MatrixEdit::LaunchColumn {
            column: columns[0],
            mode: LaunchMode::Configured,
        },
    );
    let interrupted = rows
        .iter()
        .map(|row| capture(&engine, reference.media, *row, interrupted_at))
        .collect::<Vec<_>>();
    for (index, captured) in interrupted.iter().enumerate() {
        let midpoint = at(now, 200 + [50, 450, 350][index]);
        for input in &captured.inputs {
            let old_weight = if index == 0 { 0.0 } else { 0.648 };
            let expected = if input.source == old[index] {
                (old_weight + 1.0) * 0.5
            } else {
                (1.0 - old_weight) * 0.5
            };
            assert!((input.weight.unwrap().sample(midpoint).get() - expected).abs() < 0.0001);
        }
    }
    assert_eq!(capture(&engine, reference.media, rows[0], at(now, 300)).inputs.len(), 1);
    assert_eq!(capture(&engine, reference.media, rows[1], at(now, 300)).inputs.len(), 2);
    assert_eq!(capture(&engine, reference.media, rows[2], at(now, 900)).inputs.len(), 1);
    assert_eq!(capture(&engine, reference.media, rows[1], at(now, 900)).inputs.len(), 2);
    assert_eq!(
        capture(&engine, reference.media, rows[1], at(now, 1100)).inputs.len(),
        1
    );
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
}
