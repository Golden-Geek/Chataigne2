use super::*;

#[test]
fn layer_cell_layout_pages_only_authored_cells_and_keeps_selected_definitions_independent_through_history_and_load() {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    let row = layer(layers(&mut engine, media, None, 1)[0]);
    let columns = columns(&mut engine, media, 130);
    let source = create(&mut engine, MediaKind::SolidColor);
    edit(
        &mut engine,
        media,
        MatrixEdit::SetCells {
            assignments: columns
                .iter()
                .map(|column| CellAssignment {
                    layer: row,
                    column: *column,
                    source: Some(source),
                })
                .collect(),
        },
    );
    let snapshot = inspect(&mut engine, media);
    let selected = snapshot
        .cells
        .iter()
        .find(|cell| cell.definition.column == columns[129])
        .unwrap()
        .id;
    let query = MatrixCellLayoutQuery {
        media,
        layer: row,
        selected: Some(selected),
        node: None,
        start: 0,
        count: 4,
    };
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    let page = crate::matrix::inspect_cells(&engine, query).unwrap();
    assert_eq!(page.total, 130);
    assert_eq!(page.cells.len(), 4);
    assert_eq!(
        page.cells
            .iter()
            .map(|cell| cell.view.cell.definition.column)
            .collect::<Vec<_>>(),
        columns[..4]
    );
    assert_eq!(page.selected.unwrap().view.cell.id, selected);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    assert_eq!(engine.undo_len(), history);
    let mut loaded = from_sparse_project_json::<AppNode>(&before).unwrap();
    assert_eq!(
        serde_json::to_value(crate::matrix::inspect_cells(&loaded, query).unwrap()).unwrap(),
        serde_json::to_value(crate::matrix::inspect_cells(&engine, query).unwrap()).unwrap()
    );
    edit(&mut loaded, media, MatrixEdit::RemoveColumn { column: columns[129] });
    let missing = crate::matrix::inspect_cells(&loaded, query).unwrap();
    assert_eq!(missing.selected_id, Some(selected));
    assert!(missing.selected.is_none());
    assert_eq!(missing.total, 129);
    assert!(loaded.undo().unwrap());
    assert_eq!(
        crate::matrix::inspect_cells(&loaded, query)
            .unwrap()
            .selected
            .unwrap()
            .view
            .cell
            .id,
        selected
    );
    for count in [0, 129] {
        assert!(crate::matrix::inspect_cells(&engine, MatrixCellLayoutQuery { count, ..query }).is_err());
    }
    let foreign = create(&mut engine, MediaKind::Matrix).media;
    let foreign_layer = layer(layers(&mut engine, foreign, None, 1)[0]);
    let wrong = crate::matrix::inspect_cells(
        &engine,
        MatrixCellLayoutQuery {
            layer: foreign_layer,
            ..query
        },
    )
    .unwrap();
    assert!(wrong.row.is_none() && wrong.context.is_none() && wrong.cells.is_empty() && wrong.selected.is_none());
}
