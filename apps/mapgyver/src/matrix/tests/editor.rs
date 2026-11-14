use super::*;
use golden_core::app::ProjectLifecycle;
use golden_core::node::{Node, NodeUuid};

fn query(selected: NodeUuid) -> MatrixEditorQuery {
    MatrixEditorQuery {
        selected: Some(selected),
        document: None,
        locked: None,
        retained: None,
        viewport: MatrixViewport {
            row_start: 0,
            row_count: 8,
            axis_start: 0,
            axis_count: 8,
        },
        selection: None,
        node: None,
    }
}

#[test]
fn matrix_owner_selection_clears_retained_axes_and_off_page_focus_without_changing_content() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let items = layers(&mut engine, matrix.media, None, 6);
    let first = read(&mut engine, query(layer(items[5]).0));
    let history = engine.ui_history_state();
    for owner in [matrix.media.0, matrix.channel.0] {
        let mut request = retained(&first, owner);
        request.locked = Some(matrix.media);
        request.viewport.row_count = 4;
        let page = read(&mut engine, request);
        assert_eq!(page.media, Some(matrix.media));
        assert!(page.node.is_none() && page.selection.is_none());
        let snapshot = page.snapshot.unwrap();
        assert_eq!(snapshot.row_total, 6);
        assert_eq!(snapshot.rows.len(), 4);
        assert!(snapshot.selected.is_none());
        assert_eq!(page.parameters.len(), 4);
    }
    assert_eq!(engine.ui_history_state(), history);
}

#[test]
fn layer_bindings_are_bounded_and_include_the_independent_off_page_selection() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let rows = layers(&mut engine, matrix.media, None, 140);
    let selected = layer(*rows.last().unwrap());
    let mut request = query(selected.0);
    request.viewport.row_count = 4;
    let history = engine.ui_history_state();
    let page = read(&mut engine, request);
    assert_eq!(page.at, engine.time);
    assert_eq!(page.parameters.len(), 5);
    assert!(page.parameters.iter().any(|binding| binding.item == selected.0));
    for binding in page.parameters {
        for (uuid, declaration) in [
            (binding.position, "parameters/position"),
            (binding.size, "parameters/canvas_size"),
        ] {
            let parameter = engine.nodes.get(engine.node_id_by_uuid(uuid).unwrap()).unwrap();
            assert_eq!(parameter.node_data().meta.decl_id.0, declaration);
            let folder = engine.nodes.get(parameter.node_data().parent.unwrap()).unwrap();
            assert_eq!(
                engine
                    .nodes
                    .get(folder.node_data().parent.unwrap())
                    .unwrap()
                    .node_data()
                    .meta
                    .uuid,
                binding.item
            );
        }
    }
    assert_eq!(engine.ui_history_state(), history);
}
fn read(engine: &mut AppEngine, query: MatrixEditorQuery) -> MatrixEditorPage {
    let payload = serde_json::to_value(MediaEdit::InspectMatrixEditor { query }).unwrap();
    let direct = AppNode::query_app_operation(engine, MEDIA_NAMESPACE, MEDIA_OPERATION, &payload)
        .unwrap()
        .unwrap();
    let MediaEditResult::MatrixEditor { page } = apply(engine, MediaEdit::InspectMatrixEditor { query }) else {
        panic!()
    };
    assert_eq!(
        direct,
        serde_json::to_value(MediaEditResult::MatrixEditor { page: page.clone() }).unwrap()
    );
    page
}
fn retained(page: &MatrixEditorPage, selected: NodeUuid) -> MatrixEditorQuery {
    MatrixEditorQuery {
        document: Some(page.document),
        retained: page.media,
        selection: page.selection,
        node: page.node,
        ..query(selected)
    }
}

#[test]
fn independent_targets_follow_descendants_retain_unrelated_and_merge_axes_without_launching() {
    let mut engine = fixture();
    let a = create(&mut engine, MediaKind::Matrix);
    let b = create(&mut engine, MediaKind::Matrix);
    let unrelated = create(&mut engine, MediaKind::SolidColor);
    let row = layers(&mut engine, a.media, None, 1)[0];
    let column = columns(&mut engine, a.media, 1)[0];
    let MediaEditResult::MatrixMediaCreated { cell, media } = edit(
        &mut engine,
        a.media,
        MatrixEdit::CreateCellMedia {
            layer: layer(row),
            column,
            media_kind: MediaKind::SolidColor,
        },
    ) else {
        panic!()
    };
    let history = engine.ui_history_state();
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let first = read(&mut engine, query(layer(row).0));
    assert_eq!(first.media, Some(a.media));
    assert_eq!(first.selection.unwrap().item, Some(row));
    let selected_column = read(&mut engine, retained(&first, column.0));
    assert_eq!(
        selected_column.selection,
        Some(MatrixSelection {
            item: Some(row),
            column: Some(column)
        })
    );
    for selected in [cell.0, media.0] {
        let page = read(&mut engine, query(selected));
        assert_eq!(page.media, Some(a.media));
        assert_eq!(page.node, Some(cell.0));
        assert_eq!(page.selection, selected_column.selection);
    }
    let id = engine.node_id_by_uuid(media.0).unwrap();
    let child = engine.nodes.get(id).unwrap().node_data().first_child.unwrap();
    let uuid = engine.nodes.get(child).unwrap().node_data().meta.uuid;
    assert_eq!(read(&mut engine, query(uuid)).node, Some(cell.0));
    assert_eq!(
        read(&mut engine, retained(&first, unrelated.media.0)).media,
        Some(a.media)
    );
    assert_eq!(read(&mut engine, retained(&first, b.media.0)).media, Some(b.media));
    let locked = MatrixEditorQuery {
        locked: first.media,
        ..retained(&first, b.media.0)
    };
    assert_eq!(read(&mut engine, locked).media, Some(a.media));
    assert_eq!(engine.ui_history_state(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    assert!(
        read(&mut engine, query(a.media.0))
            .snapshot
            .unwrap()
            .playback
            .layers
            .iter()
            .all(|layer| layer.column.is_none())
    );
}

#[test]
fn missing_locked_targets_recover_on_undo_and_load_but_never_cross_document_scope() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let initial = read(&mut engine, query(matrix.media.0));
    let mut request = MatrixEditorQuery {
        locked: initial.media,
        ..retained(&initial, matrix.media.0)
    };
    let id = engine.node_id_by_uuid(matrix.media.0).unwrap();
    assert!(apply_project_intent(&mut engine, UiEditIntent::RemoveNode { node: id }).success);
    let missing = read(&mut engine, request);
    assert_eq!(missing.media, initial.media);
    assert!(missing.snapshot.is_none());
    assert!(engine.undo().unwrap());
    assert!(read(&mut engine, request).snapshot.is_some());
    let mut loaded = from_sparse_project_json::<AppNode>(&to_sparse_project_json_pretty(&engine).unwrap()).unwrap();
    loaded.apply_edits().unwrap();
    assert!(read(&mut loaded, request).snapshot.is_some());
    let mut other = fixture();
    let new = create(&mut other, MediaKind::Matrix);
    request.selected = Some(new.media.0);
    assert!(!read(&mut other, request).document_matches);
    request.locked = None;
    assert_eq!(read(&mut other, request).media, Some(new.media));
    request.selected = None;
    assert!(!read(&mut other, request).document_matches);
    request.document = None;
    assert!(!invoke(&mut engine, MediaEdit::InspectMatrixEditor { query: request }).success);
}

#[test]
fn nearest_nested_matrix_owns_selection_and_sparse_pages_preserve_only_same_target_viewports() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let rows = layers(&mut engine, matrix.media, None, 1000);
    let columns = columns(&mut engine, matrix.media, 1000);
    let MediaEditResult::MatrixMediaCreated { media: nested, .. } = edit(
        &mut engine,
        matrix.media,
        MatrixEdit::CreateCellMedia {
            layer: layer(rows[0]),
            column: columns[0],
            media_kind: MediaKind::Matrix,
        },
    ) else {
        panic!()
    };
    let first = read(&mut engine, query(matrix.media.0));
    let mut request = retained(&first, matrix.media.0);
    request.viewport.row_start = 990;
    request.viewport.axis_start = 990;
    let page = read(&mut engine, request).snapshot.unwrap();
    assert_eq!(page.row_start, 990);
    assert!(page.rows.len() <= 8 && page.columns.len() <= 8);
    assert!(page.cells.is_empty());
    request.selected = Some(nested.0);
    let page = read(&mut engine, request);
    assert_eq!(page.media, Some(nested));
    assert_eq!(page.snapshot.unwrap().row_start, 0);
    let nested_row = inspect(&mut engine, nested).items[0].id;
    assert_eq!(read(&mut engine, query(layer(nested_row).0)).media, Some(nested));
}
