use super::*;
use golden_core::node::Node;

fn content(engine: &AppEngine, page: MatrixCellLayoutPage) -> serde_json::Value {
    assert_eq!(page.at, engine.time);
    for binding in &page.parameters {
        for (uuid, declaration) in [
            (binding.position, "parameters/position"),
            (
                binding.size,
                if binding.item == page.layer.0 {
                    "parameters/canvas_size"
                } else {
                    "parameters/size"
                },
            ),
        ] {
            let parameter = engine.nodes.get(engine.node_id_by_uuid(uuid).unwrap()).unwrap();
            assert_eq!(parameter.node_data().meta.decl_id.0, declaration);
            assert!(parameter.engine_param_snapshot().is_some());
            let folder = engine.nodes.get(parameter.node_data().parent.unwrap()).unwrap();
            let owner = engine.nodes.get(folder.node_data().parent.unwrap()).unwrap();
            assert_eq!(owner.node_data().meta.uuid, binding.item);
        }
    }
    let mut value = serde_json::to_value(page).unwrap();
    value.as_object_mut().unwrap().remove("at");
    // Actual live ownership is checked above; sparse reload regenerates declaration identities.
    for binding in value["parameters"].as_array_mut().unwrap() {
        binding.as_object_mut().unwrap().remove("position");
        binding.as_object_mut().unwrap().remove("size");
    }
    value
}

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
    assert_eq!(page.parameters.len(), 6);
    assert!(page.parameters.iter().any(|binding| binding.item == selected.0));
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
        content(&loaded, crate::matrix::inspect_cells(&loaded, query).unwrap()),
        content(&engine, crate::matrix::inspect_cells(&engine, query).unwrap())
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
